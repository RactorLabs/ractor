use super::api::{Message, MessageRole, RaworcClient, MESSAGE_ROLE_USER};
use super::ollama::OllamaClient;
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

        // Use Ollama API directly
        info!("Using Ollama API for message processing");

        // Fetch ALL messages from agent for complete conversation history
        info!("Fetching complete conversation history for Ollama");
        let all_messages = self.fetch_all_agent_messages().await?;

        // Prepare conversation history for Ollama
        let conversation = self.prepare_conversation_history(&all_messages, &message.id);

        // Get model response with fallback
        let system_prompt = self.build_system_prompt().await;
        let response_result = self
            .ollama_client
            .complete(conversation, Some(system_prompt))
            .await;

        let (response_text, response_type) = match response_result {
            Ok(model_response) => {
                // Validate and sanitize output
                let sanitized_response = self.guardrails.validate_output(&model_response)?;
                (sanitized_response, "model_response")
            }
            Err(e) => {
                warn!("Ollama API failed: {}, using fallback response", e);
                let fallback_response = format!(
                    "I'm currently experiencing technical difficulties with my AI processing. Here's what I can tell you:\n\n\
                    Your message was: \"{}\"\n\n\
                    I'm a Raworc Agent (Computer Use Agent) designed to help with various tasks including:\n\
                    - Code generation and analysis\n\
                    - File operations\n\
                    - Agent management\n\n\
                    Please try your request again.",
                    message.content
                );
                (fallback_response, "fallback_response")
            }
        };

        // Send response back via API
        self.api_client
            .send_message(
                response_text,
                Some(serde_json::json!({
                    "type": response_type,
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
    ) -> Vec<(String, String)> {
        let mut conversation = Vec::new();

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
                (role.to_string(), m.content.clone())
            })
            .collect();

        conversation.extend(history);

        // Add current message
        if let Some(current) = messages.iter().find(|m| m.id == current_id) {
            conversation.push((MESSAGE_ROLE_USER.to_string(), current.content.clone()));
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
}
