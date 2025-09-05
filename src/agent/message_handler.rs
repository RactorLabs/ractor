use super::api::{Message, MessageRole, RaworcClient, MESSAGE_ROLE_USER};
use super::ollama::{ChatMessage, OllamaClient};
use super::tools::{run_bash, text_edit, TextEditAction};
use super::error::Result;
use super::guardrails::Guardrails;
use chrono::{DateTime, Utc};
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
    /// Only messages created after the operator task was created should be processed.
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

        // Tool-enabled loop (no streaming): supports bash and text_editor actions.
        // Protocol: If the model needs a tool, it must respond with a single-line JSON object:
        //   {"tool":"bash","input":{"cmd":"..."}}
        //   {"tool":"text_editor","input":{"action":"create","path":"...","content":"..."}}
        // Otherwise, it responds with the final text answer.

        // Fetch full conversation
        let all_messages = self.fetch_all_agent_messages().await?;
        let mut conversation = self.prepare_conversation_history(&all_messages, &message.id);

        // Build enhanced system prompt (adds strict tool protocol)
        let mut system_prompt = self.build_system_prompt().await;
        system_prompt.push_str(
            "\n\nTool protocol (STRICT):\n- If you need to run a shell command, respond ONLY with JSON: {\"tool\":\"bash\",\"input\":{\"cmd\":\"...\"}}\n- If you need to edit files, respond ONLY with JSON: {\"tool\":\"text_editor\",\"input\":{\"action\":\"view|create|str_replace|insert\", ...}}\n- Do NOT include any additional text when calling tools.\n- If no tool is needed, respond with your final answer as plain text.\n- Paths must be relative to /agent.\n- str_replace must match exactly once.\n- insert uses 1-based line numbers.\n"
        );
        system_prompt.push_str("\nForbidden: Do NOT use any other tool names (e.g., python). Use only 'bash' or 'text_editor' via the JSON with keys {tool,input}.\n");
        system_prompt.push_str("Strict output rules when using tools:\n");
        system_prompt.push_str("- Respond with ONE raw JSON object only (no prose, no code fences).\n");
        system_prompt.push_str("- JSON shape: {\\\"tool\\\":\\\"bash|text_editor\\\",\\\"input\\\":{...}} exactly.\n");
        system_prompt.push_str("- Do NOT include keys: function_call, tool_calls, function, name, arguments, tool_name.\n");
        system_prompt.push_str("- Do NOT wrap JSON in ```json ... ``` or any other formatting.\n");
        system_prompt.push_str("- If no tool is required, produce ONLY a plain text answer (no JSON at all).\n");

        // Loop for tool usage with no hard cap on steps
        let mut steps = 0;
        // Keep a thread-local conversation for this message
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

            // Try to parse as a tool call JSON
            if let Some((tool, input)) = parse_tool_call(&model_text) {
                // Log parsed tool call (with truncated preview to avoid leaking data)
                let mut preview = if let Some(s) = input.as_str() {
                    s.to_string()
                } else {
                    serde_json::to_string(&input).unwrap_or_else(|_| "<unprintable>".to_string())
                };
                if preview.len() > 300 { preview.truncate(300); preview.push_str("…"); }
                info!("Tool call: {} input: {}", tool, preview);

                // Send tool execution notification to user
                let tool_description = match tool.as_str() {
                    "bash" => {
                        if let Some(s) = input.get("cmd").and_then(|v| v.as_str()) {
                            s.to_string()
                        } else if let Some(s) = input.get("command").and_then(|v| v.as_str()) {
                            s.to_string()
                        } else if let Some(s) = input.as_str() {
                            s.to_string()
                        } else if let Some(args) = input.get("args") {
                            if let Some(arr) = args.as_array() {
                                let parts: Vec<String> = arr.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect();
                                parts.join(" ")
                            } else if let Some(s) = args.as_str() {
                                s.to_string()
                            } else {
                                "unknown command".to_string()
                            }
                        } else {
                            "unknown command".to_string()
                        }
                    },
                    "text_editor" => {
                        let action = input.get("action")
                            .or_else(|| input.get("command"))
                            .or_else(|| input.get("operation"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("edit");
                        let path = input.get("path")
                            .or_else(|| input.get("file_path"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        format!("{} {}", action, path)
                    },
                    _ => format!("Executing {} tool", tool),
                };

                self.send_tool_message(&tool_description, &tool).await?;

                // Execute tool
                let tool_result = match tool.as_str() {
                    "bash" => {
                        let cmd = if let Some(s) = input.get("cmd").and_then(|v| v.as_str()) {
                            s.to_string()
                        } else if let Some(s) = input.get("command").and_then(|v| v.as_str()) {
                            s.to_string()
                        } else if let Some(s) = input.as_str() {
                            s.to_string()
                        } else if let Some(args) = input.get("args") {
                            if let Some(arr) = args.as_array() {
                                let parts: Vec<String> = arr.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect();
                                parts.join(" ")
                            } else if let Some(s) = args.as_str() {
                                s.to_string()
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };
                        match run_bash(&cmd).await {
                            Ok(o) => format!("[bash ok]\n{}", o),
                            Err(e) => format!("[bash error] {}", e),
                        }
                    }
                    "text_editor" => {
                        let mut normalized = input.clone();
                        // Map alternate field names from previous Claude tool schema
                        if normalized.get("action").is_none() && normalized.get("command").is_some() {
                            let cmd_owned = normalized.get("command").and_then(|v| v.as_str()).map(|s| s.to_string());
                            if let Some(cmd) = cmd_owned.as_deref() {
                                let mapped = match cmd { "view"=>"view", "create"=>"create", "str_replace"=>"str_replace", "insert"=>"insert", other=>other };
                                if let Some(obj) = normalized.as_object_mut() { obj.insert("action".to_string(), serde_json::Value::String(mapped.to_string())); }
                            }
                        }
                        // Map path alias
                        if normalized.get("path").is_none() && normalized.get("file_path").is_some() {
                            if let Some(fp) = normalized.get("file_path").cloned() { if let Some(obj)=normalized.as_object_mut(){ obj.insert("path".to_string(), fp);} }
                        }
                        // Map content aliases
                        if normalized.get("content").is_none() {
                            if let Some(ft)=normalized.get("file_text").cloned(){ if let Some(obj)=normalized.as_object_mut(){ obj.insert("content".to_string(), ft);} }
                        }
                        // Map str_replace aliases
                        if normalized.get("target").is_none() {
                            if let Some(old)=normalized.get("old_str").cloned(){ if let Some(obj)=normalized.as_object_mut(){ obj.insert("target".to_string(), old);} }
                        }
                        if normalized.get("replacement").is_none() {
                            if let Some(new)=normalized.get("new_str").cloned(){ if let Some(obj)=normalized.as_object_mut(){ obj.insert("replacement".to_string(), new);} }
                        }
                        // Map insert line alias
                        if normalized.get("line").is_none() {
                            if let Some(il)=normalized.get("insert_line").cloned(){ if let Some(obj)=normalized.as_object_mut(){ obj.insert("line".to_string(), il);} }
                        }
                        // Map view range alias
                        if normalized.get("start_line").is_none() && normalized.get("end_line").is_none() {
                            if let Some(vr)=normalized.get("view_range").and_then(|v| v.as_array()).cloned(){
                                if vr.len()>=2 { if let Some(obj)=normalized.as_object_mut(){ obj.insert("start_line".to_string(), vr[0].clone()); obj.insert("end_line".to_string(), vr[1].clone()); } }
                            }
                        }
                        if normalized.get("action").is_none() {
                            let op_owned = normalized
                                .get("operation")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            if let Some(op) = op_owned.as_deref() {
                                let mapped = match op {
                                    "write" | "create" => "create",
                                    "read" | "view" => "view",
                                    "replace" | "str_replace" => "str_replace",
                                    "insert" => "insert",
                                    other => other,
                                };
                                if let Some(obj) = normalized.as_object_mut() {
                                    obj.insert("action".to_string(), serde_json::Value::String(mapped.to_string()));
                                }
                            }
                        }
                        match parse_text_edit(&normalized) {
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

                // Summarize result length for logging (content logged below is truncated inside tools)
                info!("Tool result: {} ({} bytes)", tool, tool_result.len());
                // Cookbook alignment: feed tool result as role "tool" with name
                conversation.push(ChatMessage { role: "tool".to_string(), content: tool_result, name: Some(tool.clone()) });

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

        Ok(())
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

    async fn send_tool_message(&self, description: &str, tool_name: &str) -> Result<()> {
        // Log tool execution notification to Docker logs
        println!("TOOL_EXECUTION: {}: {}", tool_name, description);

        // Send tool execution message to user
        let metadata = serde_json::json!({
            "type": "tool_execution",
            "tool_type": tool_name
        });

        self.api_client
            .send_message(description.to_string(), Some(metadata))
            .await?;

        Ok(())
    }
}

// Parse a tool call JSON object from model output
// Expected: {"tool":"bash"|"text_editor","input":{...}}
fn parse_tool_call(s: &str) -> Option<(String, serde_json::Value)> {
    // Try multiple strategies:
    // 1) Our protocol: {"tool": "bash"|"text_editor", "input": {...}}
    // 2) Harmony/OpenAI-like: {"function_call": {"name": "tool.bash", "arguments": (obj|string) }}
    // 3) OpenAI-like array: {"tool_calls": [{"function": {"name": "tool.bash", "arguments": (obj|string)}}]}

    // Try to extract JSON blocks (plain or fenced) and parse
    let candidates = extract_json_candidates(s);
    for cand in candidates {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cand) {
            // 1) Our protocol
            if let (Some(tool), Some(input)) = (v.get("tool"), v.get("input")) {
                if let Some(tool_str) = tool.as_str() {
                    return Some((tool_str.to_string(), input.clone()));
                }
            }

            // 2) Harmony/OpenAI function_call
            if let Some(fc) = v.get("function_call") {
                if let Some(name) = fc.get("name").and_then(|n| n.as_str()) {
                    let mapped = map_tool_name(name);
                    if let Some(mut tool_name) = mapped {
                        let args_v = match fc.get("arguments") {
                            Some(a) if a.is_object() => a.clone(),
                            Some(a) if a.is_string() => {
                                let s = a.as_str().unwrap();
                                serde_json::from_str::<serde_json::Value>(s).unwrap_or(serde_json::json!({"raw": s}))
                            }
                            _ => serde_json::json!({}),
                        };
                        return Some((tool_name, args_v));
                    }
                }
            }

            // 3) tool_calls array
            if let Some(tc) = v.get("tool_calls").and_then(|t| t.as_array()) {
                if let Some(first) = tc.first() {
                    if let Some(func) = first.get("function") {
                        if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                            let mapped = map_tool_name(name);
                            if let Some(tool_name) = mapped {
                                let args_v = match func.get("arguments") {
                                    Some(a) if a.is_object() => a.clone(),
                                    Some(a) if a.is_string() => {
                                        let s = a.as_str().unwrap();
                                        serde_json::from_str::<serde_json::Value>(s).unwrap_or(serde_json::json!({"raw": s}))
                                    }
                                    _ => serde_json::json!({}),
                                };
                                return Some((tool_name, args_v));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn map_tool_name(name: &str) -> Option<String> {
    match name {
        "tool.bash" | "bash" => Some("bash".to_string()),
        "tool.text_editor" | "text_editor" | "editor" => Some("text_editor".to_string()),
        // Some models may emit a generic 'python' function; map it to bash for safety (user can run python via bash)
        "python" => Some("bash".to_string()),
        // Handle container-oriented aliases commonly emitted by models
        "container.exec" => Some("bash".to_string()),
        _ => None,
    }
}

fn extract_json_candidates(s: &str) -> Vec<String> {
    let trimmed = s.trim();
    let mut out = Vec::new();
    // If it already looks like JSON
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        out.push(trimmed.to_string());
    }
    // Look for fenced code blocks ```json ... ```
    let fence = "```";
    let mut idx = 0;
    while let Some(start) = trimmed[idx..].find(fence) {
        let a = idx + start + fence.len();
        // Optional language tag
        let after_lang = if trimmed[a..].starts_with("json") { a + 4 } else { a };
        if let Some(end_rel) = trimmed[after_lang..].find(fence) {
            let b = after_lang + end_rel;
            let block = trimmed[after_lang..b].trim();
            if block.starts_with('{') {
                out.push(block.to_string());
            }
            idx = b + fence.len();
        } else { break; }
    }
    // As a last resort, take the first { ... } span
    if out.is_empty() {
        if let Some(pos) = trimmed.find('{') {
            out.push(trimmed[pos..].to_string());
        }
    }
    out
}

fn parse_text_edit(input: &serde_json::Value) -> anyhow::Result<TextEditAction> {
    // Re-marshal and de to leverage enum tagging
    let action: TextEditAction = serde_json::from_value(input.clone())?;
    Ok(action)
}
