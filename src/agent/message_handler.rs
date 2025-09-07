use super::api::{Message, MessageRole, RaworcClient, MESSAGE_ROLE_USER};
use super::ollama::{ChatMessage, OllamaClient};
use super::tools::{run_bash, text_edit, TextEditAction};
use super::error::Result;
use super::guardrails::Guardrails;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub struct MessageHandler {
    api_client: Arc<RaworcClient>,
    ollama_client: Arc<OllamaClient>,
    guardrails: Arc<Guardrails>,
    processed_user_message_ids: Arc<Mutex<HashSet<String>>>,
    task_created_at: DateTime<Utc>,
}

impl MessageHandler {
    pub fn new(
        api_client: Arc<RaworcClient>,
        ollama_client: Arc<OllamaClient>,
        guardrails: Arc<Guardrails>,
    ) -> Self {
        // Try to read task creation timestamp from environment, fallback to current time
        let task_created_at = std::env::var("RAWORC_TASK_CREATED_AT")
            .ok()
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|| {
                warn!("RAWORC_TASK_CREATED_AT not found, using current time");
                Utc::now()
            });

        info!(
            "MessageHandler initialized with task created at: {}",
            task_created_at
        );

        Self {
            api_client,
            ollama_client,
            guardrails,
            processed_user_message_ids: Arc::new(Mutex::new(HashSet::new())),
            task_created_at,
        }
    }

    /// Initialize message processing based on task creation time.
    /// Only messages created after the controller task was created should be processed.
    pub async fn initialize_processed_tracking(&self) -> Result<()> {
        info!("Initializing timestamp-based message tracking...");
        info!("Task creation time: {}", self.task_created_at);

        let all_messages = self.api_client.get_messages(None, None).await?;

        if all_messages.is_empty() {
            info!("No existing messages - fresh agent");
            return Ok(());
        }

        // Mark all user messages created before task creation time as processed
        let mut user_messages_before_task = HashSet::new();
        let mut messages_after_task_count = 0;

        for message in &all_messages {
            if message.role == MessageRole::User {
                if let Ok(message_time) = DateTime::parse_from_rfc3339(&message.created_at) {
                    let message_time_utc = message_time.with_timezone(&Utc);
                    if message_time_utc < self.task_created_at {
                        user_messages_before_task.insert(message.id.clone());
                        info!(
                            "User message {} created before task - marking as processed",
                            message.id
                        );
                    } else {
                        messages_after_task_count += 1;
                        info!(
                            "User message {} created after task - will process",
                            message.id
                        );
                    }
                } else {
                    warn!(
                        "Failed to parse created_at timestamp for message {}: {}",
                        message.id, message.created_at
                    );
                }
            }
        }

        info!("Found {} total messages", all_messages.len());
        info!(
            "Marked {} user messages before task as processed",
            user_messages_before_task.len()
        );
        info!(
            "Found {} user messages after task that need processing",
            messages_after_task_count
        );

        // Mark pre-task user messages as processed
        let mut processed = self.processed_user_message_ids.lock().await;
        *processed = user_messages_before_task;

        Ok(())
    }

    pub async fn poll_and_process(&self) -> Result<usize> {
        // Get recent messages
        let recent_messages = self.api_client.get_messages(Some(50), None).await?;

        if recent_messages.is_empty() {
            return Ok(0);
        }

        // Find user messages created after task creation that need processing
        let mut unprocessed_user_messages = Vec::new();

        for message in &recent_messages {
            if message.role == MessageRole::User {
                // Only consider messages created after task creation
                if let Ok(message_time) = DateTime::parse_from_rfc3339(&message.created_at) {
                    let message_time_utc = message_time.with_timezone(&Utc);
                    if message_time_utc >= self.task_created_at {
                        // Check if already processed
                        let processed_ids = self.processed_user_message_ids.lock().await;
                        let already_processed = processed_ids.contains(&message.id);
                        drop(processed_ids);

                        if !already_processed {
                            // Check if this message already has an agent response
                            let has_response = recent_messages.iter().any(|m| {
                                m.role == MessageRole::Agent && {
                                    if let Ok(m_time) = DateTime::parse_from_rfc3339(&m.created_at)
                                    {
                                        let m_time_utc = m_time.with_timezone(&Utc);
                                        m_time_utc > message_time_utc
                                    } else {
                                        false
                                    }
                                }
                            });

                            if !has_response {
                                unprocessed_user_messages.push(message.clone());
                            }
                        }
                    }
                } else {
                    warn!(
                        "Failed to parse created_at timestamp for message {}: {}",
                        message.id, message.created_at
                    );
                }
            }
        }

        if unprocessed_user_messages.is_empty() {
            return Ok(0);
        }

        // Sort by creation time to process in order
        unprocessed_user_messages.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        // Update agent state to BUSY (pauses timeout)
        if let Err(e) = self.api_client.update_agent_to_busy().await {
            warn!("Failed to update agent state to BUSY: {}", e);
        }

        // Process each message
        for message in &unprocessed_user_messages {
            if let Err(e) = self.process_message(message).await {
                error!("Failed to process message {}: {}", message.id, e);

                // Generate error response
                let error_response = format!(
                    "Sorry, I encountered an error processing your message: {}",
                    e
                );
                if let Err(send_err) = self
                    .api_client
                    .send_message(
                        error_response,
                        Some(serde_json::json!({
                            "type": "error_response",
                            "original_error": e.to_string()
                        })),
                    )
                    .await
                {
                    error!("Failed to send error response: {}", send_err);
                }
            }

            // Mark this user message as processed
            let mut processed_ids = self.processed_user_message_ids.lock().await;
            processed_ids.insert(message.id.clone());
        }

        // Update agent state back to IDLE (starts timeout)
        if let Err(e) = self.api_client.update_agent_to_idle().await {
            warn!("Failed to update agent state to IDLE: {}", e);
        }

        Ok(unprocessed_user_messages.len())
    }

    async fn process_message(&self, message: &Message) -> Result<()> {
        info!("Processing message: {}", message.id);

        // Validate input with guardrails
        self.guardrails.validate_input(&message.content)?;

        // Fast-path: if USER sent a strict top-level tool JSON, execute directly
        if let Some((tool, input)) = parse_user_tool_call(&message.content) {
            // Notify
            let tool_description = match tool.as_str() {
                "bash" => {
                    input
                        .get("command").and_then(|v| v.as_str())
                        .or_else(|| input.get("cmd").and_then(|v| v.as_str()))
                        .unwrap_or("bash command")
                        .to_string()
                }
                "text_editor" => {
                    let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("edit");
                    let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("unknown");
                    format!("{} {}", action, path)
                }
                _ => format!("Executing {} tool", tool),
            };
            self.send_tool_message(&tool_description, &tool, Some(&input)).await?;

            // Execute
            let tool_result = match tool.as_str() {
                "bash" => {
                    let cmd = input
                        .get("command").and_then(|v| v.as_str())
                        .or_else(|| input.get("cmd").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    match run_bash(cmd).await { Ok(o) => format!("[bash ok]\n{}", o), Err(e) => format!("[bash error] {}", e) }
                }
                "text_editor" => {
                    match parse_text_edit(&input) {
                        Ok(op) => match text_edit(op).await { Ok(o) => format!("[text_editor ok]\n{}", o), Err(e) => format!("[text_editor error] {}", e) },
                        Err(e) => format!("[text_editor error] {}", e),
                    }
                }
                other => format!("[error] unknown tool: {}", other),
            };

            // Send result back
            let mut metadata = serde_json::json!({ "type": "tool_result", "tool_type": tool });
            if let Some(obj) = metadata.as_object_mut() { obj.insert("args".to_string(), input.clone()); }
            self.api_client.send_message(tool_result, Some(metadata)).await?;
            return Ok(());
        }

        // Native Ollama tool calling with GPT-OSS models
        // The model will use structured tool calls when tools are needed

        // Fetch full conversation
        let all_messages = self.fetch_all_agent_messages().await?;
        let mut conversation = self.prepare_conversation_history(&all_messages, &message.id);

        // Build system prompt
        let system_prompt = self.build_system_prompt().await;

        // Loop for tool usage with no hard cap on steps
        let mut steps = 0;
        loop {
            steps += 1;
            // Simple retry/backoff for transient failures
            let mut resp = Err(super::error::HostError::Model("uninitialized".to_string()));
            for attempt in 0..=2 {
                let try_resp = self
                    .ollama_client
                    .complete(conversation.clone(), Some(system_prompt.clone()))
                    .await;
                match try_resp {
                    Ok(t) => { resp = Ok(t); break; }
                    Err(e) => {
                        if attempt < 2 { let delay = 300u64 * 3u64.pow(attempt); tokio::time::sleep(std::time::Duration::from_millis(delay)).await; resp = Err(e); continue; } else { resp = Err(e); }
                    }
                }
            }

            let model_text = match resp {
                Ok(t) => t,
                Err(e) => {
                    warn!("Ollama API failed: {}", e);
                    self.finalize_with_fallback(&message.content).await?;
                    return Ok(());
                }
            };

            // Check if response contains structured tool calls (GPT-OSS format)
            if let Some(tool_calls) = parse_structured_tool_calls(&model_text) {
                if let Some(tool_call) = tool_calls.first() {
                    let tool_name = &tool_call.function.name;
                    let args = &tool_call.function.arguments;
                    
                    // Log parsed tool call
                    info!("Structured tool call: {} with args: {:?}", tool_name, args);

                    // Send tool execution notification to user
                    let tool_description = match tool_name.as_str() {
                        "bash" => {
                            if let Some(cmd) = args
                                .get("command").and_then(|v| v.as_str())
                                .or_else(|| args.get("cmd").and_then(|v| v.as_str()))
                            {
                                cmd.to_string()
                            } else {
                                "bash command".to_string()
                            }
                        },
                        "text_editor" => {
                            let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("edit");
                            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("unknown");
                            format!("{} {}", action, path)
                        },
                        _ => format!("Executing {} tool", tool_name),
                    };

                    self.send_tool_message(&tool_description, tool_name, Some(args)).await?;

                    // Execute tool
                    let tool_result = match tool_name.as_str() {
                        "bash" => {
                            let cmd = args
                                .get("command").and_then(|v| v.as_str())
                                .or_else(|| args.get("cmd").and_then(|v| v.as_str()))
                                .unwrap_or("");
                            match run_bash(cmd).await {
                                Ok(o) => format!("[bash ok]\n{}", o),
                                Err(e) => format!("[bash error] {}", e),
                            }
                        }
                        "text_editor" => {
                            match parse_text_edit(args) {
                                Ok(action) => match text_edit(action).await {
                                    Ok(o) => format!("[text_editor ok]\n{}", o),
                                    Err(e) => format!("[text_editor error] {}", e),
                                },
                                Err(e) => format!("[text_editor error] {}", e),
                            }
                        }
                        other => {
                            format!("[error] unknown tool: {}", other)
                        }
                    };

                    // Log tool result
                    info!("Tool result: {} ({} bytes)", tool_name, tool_result.len());
                    
                    // Also send tool result as a message so Operator can render it
                    let mut meta = serde_json::json!({ "type": "tool_result", "tool_type": tool_name });
                    if let Some(obj) = meta.as_object_mut() { obj.insert("args".to_string(), args.clone()); }
                    let _ = self.api_client.send_message(tool_result.clone(), Some(meta)).await;

                    // Add tool result to conversation following Ollama cookbook
                    conversation.push(ChatMessage { 
                        role: "tool".to_string(), 
                        content: tool_result, 
                        name: Some(tool_name.clone()) 
                    });

                    continue;
                }
            } else if let Some((tool_name, args)) = parse_assistant_functions_text(&model_text) {
                // Fallback: parse assistant<|channel|>functions.* style
                info!("Assistant functions text tool call: {} with args: {:?}", tool_name, args);

                // Notify user
                let tool_description = match tool_name.as_str() {
                    "bash" => {
                        if let Some(cmd) = args
                            .get("command").and_then(|v| v.as_str())
                            .or_else(|| args.get("cmd").and_then(|v| v.as_str()))
                        {
                            cmd.to_string()
                        } else { "bash command".to_string() }
                    }
                    "text_editor" => {
                        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("edit");
                        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("unknown");
                        format!("{} {}", action, path)
                    }
                    _ => format!("Executing {} tool", tool_name),
                };
                self.send_tool_message(&tool_description, &tool_name, Some(&args)).await?;

                // Execute
                let tool_result = match tool_name.as_str() {
                    "bash" => {
                        let cmd = args
                            .get("command").and_then(|v| v.as_str())
                            .or_else(|| args.get("cmd").and_then(|v| v.as_str()))
                            .unwrap_or("");
                        match run_bash(cmd).await {
                            Ok(o) => format!("[bash ok]\n{}", o),
                            Err(e) => format!("[bash error] {}", e),
                        }
                    }
                    "text_editor" => {
                        match parse_text_edit(&args) {
                            Ok(action) => match text_edit(action).await {
                                Ok(o) => format!("[text_editor ok]\n{}", o),
                                Err(e) => format!("[text_editor error] {}", e),
                            },
                            Err(e) => format!("[text_editor error] {}", e),
                        }
                    }
                    other => format!("[error] unknown tool: {}", other),
                };

                // Send result to Operator and add to conversation
                let mut meta = serde_json::json!({ "type": "tool_result", "tool_type": tool_name });
                if let Some(obj) = meta.as_object_mut() { obj.insert("args".to_string(), args.clone()); }
                let _ = self.api_client.send_message(tool_result.clone(), Some(meta)).await;

                // Add tool result to conversation and continue
                conversation.push(ChatMessage { 
                    role: "tool".to_string(), 
                    content: tool_result, 
                    name: Some(tool_name.clone()) 
                });
                continue;
            } else {
                // Treat as final answer
                let sanitized = self.guardrails.validate_output(&model_text)?;
                self.api_client
                    .send_message(
                        sanitized,
                        Some(serde_json::json!({
                            "type": "model_response",
                            "model": "gpt-oss"
                        })),
                    )
                    .await?;
                return Ok(());
            }
        }
    }

    async fn finalize_with_fallback(&self, original: &str) -> Result<()> {
        let fallback_response = format!(
            "I'm experiencing technical difficulties with AI processing. Your request was: \"{}\". Please try again later.",
            original
        );
        let sanitized = self.guardrails.validate_output(&fallback_response)?;
        self.api_client
            .send_message(
                sanitized,
                Some(serde_json::json!({
                    "type": "fallback_response",
                    "model": "gpt-oss"
                })),
            )
            .await?;
        Ok(())
    }

    async fn finalize_with_note(&self, note: &str) -> Result<()> {
        let msg = format!("Tool loop terminated: {}", note);
        let sanitized = self.guardrails.validate_output(&msg)?;
        self.api_client
            .send_message(
                sanitized,
                Some(serde_json::json!({
                    "type": "tool_loop_stop",
                    "model": "gpt-oss"
                })),
            )
            .await?;
        Ok(())
    }

    async fn fetch_all_agent_messages(&self) -> Result<Vec<Message>> {
        // Fetch ALL messages in agent without pagination limits
        let all_messages = self.api_client.get_messages(None, None).await?;

        info!(
            "Fetched {} total messages for conversation history",
            all_messages.len()
        );
        Ok(all_messages)
    }

    fn prepare_conversation_history(
        &self,
        messages: &[Message],
        current_id: &str,
    ) -> Vec<ChatMessage> {
        let mut conversation: Vec<ChatMessage> = Vec::new();

        // Include ALL message history (excluding the current message being processed)
        let history: Vec<_> = messages
            .iter()
            .filter(|m| m.id != current_id)
            .filter(|m| m.role == MessageRole::User || m.role == MessageRole::Agent)
            .map(|m| {
                let role = match m.role {
                    MessageRole::User => MESSAGE_ROLE_USER,
                    MessageRole::Agent => "assistant", // Model expects "assistant" not "agent"
                    _ => MESSAGE_ROLE_USER,
                };
                ChatMessage { role: role.to_string(), content: m.content.clone(), name: None }
            })
            .collect();

        conversation.extend(history);

        // Add current message
        if let Some(current) = messages.iter().find(|m| m.id == current_id) {
            conversation.push(ChatMessage { role: MESSAGE_ROLE_USER.to_string(), content: current.content.clone(), name: None });
        }

        info!(
            "Prepared conversation with {} messages of history",
            conversation.len() - 1
        );
        conversation
    }

    async fn build_system_prompt(&self) -> String {
        let mut prompt = String::from(
            r#"You are a helpful AI assistant operating within a RemoteAgent agent with bash command execution capabilities.

Key capabilities:
- You can help users with various tasks and answer questions
- You maintain conversation context within this agent
- You can create, read, and modify files within the agent directory
- You have access to a bash tool that can execute shell commands
- You have access to a text_editor tool for precise file editing operations


Bash Tool Usage:
- Use the bash tool to execute shell commands when needed
- Commands are executed in the /agent/ directory with persistent state
- You can run any typical bash/shell commands: ls, cat, grep, find, python, npm, git, etc.
- File operations, code execution, system administration, package management are all supported
- The bash environment persists between commands within the conversation
- For system package management (apt-get, yum, etc.), use sudo when needed but confirm with user first
- Example: "I need to install a package with sudo apt-get. Is that okay?" before running privileged commands
- All bash executions are automatically logged to /agent/logs/ and Docker logs for debugging

Text Editor Tool Usage:
- Use the text_editor tool for precise file editing operations
- Available commands: view, create, str_replace, insert
- All paths are relative to /agent/ directory
- view: Examine file contents or list directory contents (supports line ranges)
- create: Create new files with specified content
- str_replace: Replace exact text strings in files (must be unique matches)
- insert: Insert text at specific line numbers
- Ideal for code editing, configuration files, and precise text modifications
- All text editor operations are automatically logged to /agent/logs/ and Docker logs for debugging



Working Directory and File Operations:
- Your working directory is /agent/
- When creating files, writing code, or performing file operations, use /agent/ as your base directory
- The agent has persistent storage mounted at /agent/ with the following structure and usage patterns:

  /agent/code/ - Code artifacts and development files:
    - Store all source code files (Python, JavaScript, Rust, etc.)
    - Save scripts, automation tools, and executable files
    - Keep project configuration files (package.json, requirements.txt, Cargo.toml)
    - Place build artifacts and compiled outputs
    - Store development documentation and README files
    - Example: /agent/code/my_script.py, /agent/code/package.json

  /agent/logs/ - Command execution logs and system activity:
    - Automatically stores individual bash command execution logs
    - Each bash command creates a timestamped log file (bash_TIMESTAMP.log)
    - Contains command, exit code, stdout, stderr, and execution details
    - Useful for debugging, auditing, and reviewing command history
    - Not copied during agent remix - logs are unique per agent instance
    - Example: /agent/logs/bash_1641234567.log

  /agent/content/ - HTML display and visualization content:
    - Store HTML files and supporting assets for displaying information to users
    - ALWAYS create or update /agent/content/index.html as the main entry point
    - Use index.html for summary, overview, intro, instructions, or navigation
    - Link to other files using relative URLs (e.g., <a href="report.html">Report</a>)
    - Create interactive visualizations, reports, charts, and data displays
    - Build images, maps, tables, games, apps, and rich interactive content
    - Support all types of visual and interactive content: charts, graphs, dashboards, games, applications, maps, image galleries, data tables, reports, presentations
    - Build dashboard-style interfaces and presentation materials
    - Save CSS, JavaScript, and other web assets that support HTML content
    - Perfect for creating visual outputs that users can view in a browser
    - IMPORTANT: Use /agent/content/ for displaying ANY information to users - results, reports, dashboards, visualizations, documentation, summaries, interactive apps, games, or any content users need to view
    - Create well-formatted HTML files with proper styling and navigation for professional presentation
    - Example structure: index.html (main), report.html, chart.html, dashboard/, games/, maps/

  /agent/secrets/ - Environment variables and configuration:
    - Contains environment variables automatically sourced by the agent
    - Secrets and API keys are loaded from this directory
    - Configuration files for authentication and external services
    - This directory is automatically processed - you typically don't need to manage it directly

Special Files with Automatic Processing:
  /agent/code/instructions.md - Agent instructions (auto-included in system prompt):
    - If this file exists, its contents are automatically appended to your system prompt
    - Use this for persistent agent-specific instructions or context
    - Perfect for project requirements, coding standards, or ongoing task context
    - Contents become part of your instructions for every message in the agent

  /agent/code/setup.sh - Agent initialization script (auto-executed on container start):
    - If this file exists, it's automatically executed when the agent container starts
    - Use this for environment setup, package installation, or initial configuration
    - Runs once at the beginning of each agent (including agent restores)
    - Perfect for installing dependencies, setting up tools, or preparing the environment

- Use /agent/code/ for all files including executables, data, project structure, and working files
- Use /agent/content/ for HTML files and web assets that provide visual displays to users
- /agent/logs/ contains automatic execution logs - not for user files
- All file paths should be relative to /agent/ unless specifically working with system files

Security and Safety:
- The bash tool has built-in security restrictions to prevent dangerous operations
- Commands that could damage the system or access sensitive areas are blocked
- You're operating in an isolated container environment
- Feel free to use the bash tool for legitimate development and analysis tasks
- When using sudo for package installation or system changes, always ask user permission first
- Be transparent about privileged operations: "I need sudo access to install X. Is that okay?"

Guidelines:
- Be helpful, accurate, and concise
- Use the bash tool for system operations, package management, and command execution
- Use the text_editor tool for precise file editing, viewing, and text modifications
- Choose the right tool: bash for operations, text_editor for files
- Respect user privacy and security
- When creating files, organize them appropriately:
  - Save all files including source code, data, scripts, and project files to /agent/code/
  - Save HTML files and visual displays to /agent/content/
  - ALWAYS use /agent/content/ when you need to display information to users in a visual format
  - Create interactive content like games, apps, maps, charts, tables, images, and presentations in /agent/content/
  - Create /agent/code/instructions.md for persistent agent context (auto-loaded)
  - Create /agent/code/setup.sh for environment initialization (auto-executed)
- Content folder workflow (IMPORTANT for visual content):
  - ALWAYS create /agent/content/index.html as the main entry point
  - Use index.html for overview, summary, navigation, or standalone content
  - Link additional files using relative paths: href="report.html", src="data/chart.png"
  - Create supporting files: report.html, dashboard.html, styles.css, etc.
  - Organize subdirectories as needed: images/, data/, scripts/
  - Example: index.html -> links to -> report.html, chart.html, dashboard/
- Assume the current working directory is /agent/
- Show command outputs to users when relevant
- Organize files logically: all working files in /agent/code/, visuals in /agent/content/

Current agent context:
- This is an isolated agent environment with persistent storage
- Messages are persisted in the Raworc system
- You're operating as the Agent (Computer Use Agent) within this container
- Your agent persists between container restarts
- You have full bash access for development, analysis, and automation tasks"#,
        );

        // Read instructions from /agent/code/instructions.md if it exists
        let instructions_path = std::path::Path::new("/agent/code/instructions.md");
        info!(
            "Checking for instructions file at: {}",
            instructions_path.display()
        );
        if instructions_path.exists() {
            info!("Instructions file exists, reading contents...");
            match tokio::fs::read_to_string(instructions_path).await {
                Ok(instructions) => {
                    info!("Read instructions content: '{}'", instructions.trim());
                    prompt.push_str("\n\nSPECIAL INSTRUCTIONS FROM USER:\n");
                    prompt.push_str(&instructions);
                    info!("Loaded instructions from /agent/code/instructions.md");
                }
                Err(e) => {
                    warn!("Failed to read instructions file: {}", e);
                }
            }
        } else {
            info!(
                "No instructions file found at {}",
                instructions_path.display()
            );
        }

        prompt
    }

    async fn send_tool_message(&self, description: &str, tool_name: &str, args: Option<&serde_json::Value>) -> Result<()> {
        // Log tool execution notification to Docker logs
        println!("TOOL_EXECUTION: {}: {}", tool_name, description);

        // Send tool execution message to user
        let mut meta = serde_json::json!({
            "type": "tool_execution",
            "tool_type": tool_name
        });
        if let Some(a) = args { if let Some(obj) = meta.as_object_mut() { obj.insert("args".to_string(), a.clone()); } }
        self.api_client
            .send_message(description.to_string(), Some(meta))
            .await?;

        Ok(())
    }
}

// Parse structured tool calls from GPT-OSS model response
#[derive(Debug, Deserialize)]
struct StructuredToolCall {
    function: StructuredToolFunction,
}

#[derive(Debug, Deserialize)]
struct StructuredToolFunction {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ToolCallsResponse {
    tool_calls: Vec<StructuredToolCall>,
}

fn parse_structured_tool_calls(s: &str) -> Option<Vec<StructuredToolCall>> {
    // Try to parse the response as JSON containing tool_calls
    if let Ok(response) = serde_json::from_str::<ToolCallsResponse>(s) {
        Some(response.tool_calls)
    } else {
        None
    }
}

// Parse text that looks like: "assistant<|channel|>functions.bash" followed by a JSON args block
fn parse_assistant_functions_text(s: &str) -> Option<(String, serde_json::Value)> {
    let lower = s.to_lowercase();
    let tool = if lower.contains("assistant<|channel|>functions.bash") {
        "bash"
    } else if lower.contains("assistant<|channel|>functions.text_editor") {
        "text_editor"
    } else {
        return None;
    };
    let start = s.rfind('{')?;
    let end = s.rfind('}')?;
    if end <= start { return None; }
    let json_str = &s[start..=end];
    let args: serde_json::Value = serde_json::from_str(json_str).ok()?;
    Some((tool.to_string(), args))
}

// Strict user tool JSON: only accepts a top-level {"tool":"bash|text_editor","input":{...}}
fn parse_user_tool_call(s: &str) -> Option<(String, serde_json::Value)> {
    let trimmed = s.trim();
    if !(trimmed.starts_with('{') && trimmed.ends_with('}')) {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let tool = v.get("tool")?.as_str()?.to_string();
    if tool != "bash" && tool != "text_editor" { return None; }
    let input = v.get("input")?.clone();
    Some((tool, input))
}

fn parse_text_edit(input: &serde_json::Value) -> anyhow::Result<TextEditAction> {
    // Normalize common alias keys before deserializing to the enum
    let mut v = input.clone();
    if let Some(obj) = v.as_object_mut() {
        // Accept "file" or "file_path" as alias for "path"
        if !obj.contains_key("path") {
            if let Some(p) = obj.get("file").cloned().or_else(|| obj.get("file_path").cloned()) {
                obj.insert("path".to_string(), p);
            }
        }
    }
    let action: TextEditAction = serde_json::from_value(v)?;
    Ok(action)
}
