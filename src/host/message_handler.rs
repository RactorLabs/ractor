use super::api::{RaworcClient, Message, MessageRole, MESSAGE_ROLE_USER};
use super::claude::ClaudeClient;
use super::error::Result;
use super::guardrails::Guardrails;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub struct MessageHandler {
    api_client: Arc<RaworcClient>,
    claude_client: Arc<ClaudeClient>,
    guardrails: Arc<Guardrails>,
    processed_user_message_ids: Arc<Mutex<HashSet<String>>>,
}

impl MessageHandler {
    pub fn new(
        api_client: Arc<RaworcClient>,
        claude_client: Arc<ClaudeClient>,
        guardrails: Arc<Guardrails>,
    ) -> Self {
        Self {
            api_client,
            claude_client,
            guardrails,
            processed_user_message_ids: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Find which user messages already have Host responses to avoid reprocessing.
    /// Simple and reliable approach for both fresh and restored sessions.
    pub async fn initialize_processed_tracking(&self) -> Result<()> {
        info!("Initializing processed message tracking...");
        
        let all_messages = self.api_client.get_messages(None, None).await?;
        
        if all_messages.is_empty() {
            info!("No existing messages - fresh session");
            return Ok(());
        }

        // Find user messages that have corresponding Host responses
        // Simple approach: if there are any Host messages, assume all previous user messages have responses
        let mut user_messages_with_responses = HashSet::new();
        
        // Collect all user and Host message IDs first
        let mut user_messages = Vec::new();
        let mut host_count = 0;
        
        for message in &all_messages {
            match message.role {
                MessageRole::User => {
                    user_messages.push(message.id.clone());
                },
                MessageRole::Host => {
                    host_count += 1;
                },
                MessageRole::System => {
                    // System messages don't affect counting
                }
            }
        }
        
        // Mark the first N user messages as having responses (where N = host_count)
        for (i, user_msg_id) in user_messages.iter().enumerate() {
            if i < host_count {
                user_messages_with_responses.insert(user_msg_id.clone());
            }
        }
        
        info!("Found {} user messages, {} Host responses, marking first {} user messages as processed", 
              user_messages.len(), host_count, user_messages_with_responses.len());

        // Mark user messages that have responses as processed
        let mut processed = self.processed_user_message_ids.lock().await;
        *processed = user_messages_with_responses;
        
        info!("Initialized tracking: {} user messages already have responses", processed.len());
        Ok(())
    }
    
    
    pub async fn poll_and_process(&self) -> Result<usize> {
        // Get recent messages
        let recent_messages = self.api_client.get_messages(Some(50), None).await?;
        
        if recent_messages.is_empty() {
            return Ok(0);
        }
        
        
        // Find user messages that need processing (much simpler approach)
        let mut unprocessed_user_messages = Vec::new();
        
        // Simple logic: check if each user message has a Host message after it
        for (i, message) in recent_messages.iter().enumerate() {
            if message.role == MessageRole::User {
                // Check if the next message is a Host response
                let has_immediate_response = i + 1 < recent_messages.len() 
                    && recent_messages[i + 1].role == MessageRole::Host;
                
                
                // Only process if no immediate Host response and not already processed
                let processed_ids = self.processed_user_message_ids.lock().await;
                let already_processed = processed_ids.contains(&message.id);
                drop(processed_ids);
                
                if !already_processed && !has_immediate_response {
                    unprocessed_user_messages.push(message.clone());
                }
            }
        }
        
        
        if unprocessed_user_messages.is_empty() {
            return Ok(0);
        }
        
        // Sort by creation time to process in order
        unprocessed_user_messages.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        
        
        // Update session state to BUSY (pauses timeout)
        if let Err(e) = self.api_client.update_session_to_busy().await {
            warn!("Failed to update session state to BUSY: {}", e);
        }
        
        // Process each message
        for message in &unprocessed_user_messages {
            if let Err(e) = self.process_message(message).await {
                error!("Failed to process message {}: {}", message.id, e);
                
                // Generate error response
                let error_response = format!("Sorry, I encountered an error processing your message: {}", e);
                if let Err(send_err) = self.api_client.send_message(
                    error_response,
                    Some(serde_json::json!({
                        "type": "error_response",
                        "original_error": e.to_string()
                    })),
                ).await {
                    error!("Failed to send error response: {}", send_err);
                }
            }
            
            // Mark this user message as processed
            let mut processed_ids = self.processed_user_message_ids.lock().await;
            processed_ids.insert(message.id.clone());
        }
        
        // Update session state back to IDLE (starts timeout)
        if let Err(e) = self.api_client.update_session_to_idle().await {
            warn!("Failed to update session state to IDLE: {}", e);
        }
        
        Ok(unprocessed_user_messages.len())
    }
    
    async fn process_message(&self, message: &Message) -> Result<()> {
        info!("Processing message: {}", message.id);
        
        // Validate input with guardrails
        self.guardrails.validate_input(&message.content)?;
        
        // Use Claude API directly
        info!("Using Claude API for message processing");
        
        // Fetch ALL messages from session for complete conversation history
        info!("Fetching complete conversation history for Claude");
        let all_messages = self.fetch_all_session_messages().await?;
        
        // Prepare conversation history for Claude
        let conversation = self.prepare_conversation_history(&all_messages, &message.id);
        
        // Get Claude's response with fallback
        let system_prompt = self.build_system_prompt().await;
        let response_result = self.claude_client
            .complete(conversation, Some(system_prompt))
            .await;
            
        let (response_text, response_type) = match response_result {
            Ok(claude_response) => {
                // Validate and sanitize output
                let sanitized_response = self.guardrails.validate_output(&claude_response)?;
                (sanitized_response, "claude_response")
            }
            Err(e) => {
                warn!("Claude API failed: {}, using fallback response", e);
                let fallback_response = format!(
                    "I'm currently experiencing technical difficulties with my AI processing. Here's what I can tell you:\n\n\
                    Your message was: \"{}\"\n\n\
                    I'm a Raworc Host (Computer Use Agent) designed to help with various tasks including:\n\
                    - Code generation and analysis\n\
                    - File operations\n\
                    - Session management\n\n\
                    Please try your request again.",
                    message.content
                );
                (fallback_response, "fallback_response")
            }
        };
        
        // Send response back via API
        self.api_client.send_message(
            response_text,
            Some(serde_json::json!({
                "type": response_type,
                "model": "claude-sonnet-4-20250514"
            })),
        ).await?;
        
        Ok(())
    }
    
    async fn fetch_all_session_messages(&self) -> Result<Vec<Message>> {
        // Fetch ALL messages in session without pagination limits
        let all_messages = self.api_client.get_messages(None, None).await?;
        
        info!("Fetched {} total messages for conversation history", all_messages.len());
        Ok(all_messages)
    }
    
    fn prepare_conversation_history(&self, messages: &[Message], current_id: &str) -> Vec<(String, String)> {
        let mut conversation = Vec::new();
        
        // Include ALL message history (excluding the current message being processed)
        let history: Vec<_> = messages
            .iter()
            .filter(|m| m.id != current_id)
            .filter(|m| m.role == MessageRole::User || m.role == MessageRole::Host)
            .map(|m| {
                let role = match m.role {
                    MessageRole::User => MESSAGE_ROLE_USER,
                    MessageRole::Host => "assistant", // Claude expects "assistant" not "host"  
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
        
        info!("Prepared conversation with {} messages of history", conversation.len() - 1);
        conversation
    }
    
    async fn build_system_prompt(&self) -> String {
        let mut prompt = String::from(
            r#"You are a helpful AI assistant operating within a Raworc session with bash command execution capabilities.

Key capabilities:
- You can help users with various tasks and answer questions
- You maintain conversation context within this session
- You can create, read, and modify files within the session directory
- You have access to a bash tool that can execute shell commands
- You have access to a text_editor tool for precise file editing operations
- You have access to a web_search tool for real-time information beyond your knowledge cutoff

Bash Tool Usage:
- Use the bash tool to execute shell commands when needed
- Commands are executed in the /session/ directory with persistent state
- You can run any typical bash/shell commands: ls, cat, grep, find, python, npm, git, etc.
- File operations, code execution, system administration, package management are all supported
- The bash environment persists between commands within the conversation
- For system package management (apt-get, yum, etc.), use sudo when needed but confirm with user first
- Example: "I need to install a package with sudo apt-get. Is that okay?" before running privileged commands
- All bash executions are automatically logged to /session/logs/ and Docker logs for debugging

Text Editor Tool Usage:
- Use the text_editor tool for precise file editing operations
- Available commands: view, create, str_replace, insert
- All paths are relative to /session/ directory
- view: Examine file contents or list directory contents (supports line ranges)
- create: Create new files with specified content
- str_replace: Replace exact text strings in files (must be unique matches)
- insert: Insert text at specific line numbers
- Ideal for code editing, configuration files, and precise text modifications
- All text editor operations are automatically logged to /session/logs/ and Docker logs for debugging

Web Search Tool Usage:
- Use the web_search tool to find current information beyond your knowledge cutoff
- Automatically searches the web and provides real-time results with citations
- Perfect for finding latest documentation, current news, recent updates, or trending information
- Search results include source URLs and are automatically cited in responses
- Limited to 10 searches per conversation to manage usage costs
- Use when users ask for current information or when your knowledge might be outdated

Working Directory and File Operations:
- Your working directory is /session/
- When creating files, writing code, or performing file operations, use /session/ as your base directory
- The session has persistent storage mounted at /session/ with the following structure and usage patterns:

  /session/code/ - Code artifacts and development files:
    - Store all source code files (Python, JavaScript, Rust, etc.)
    - Save scripts, automation tools, and executable files
    - Keep project configuration files (package.json, requirements.txt, Cargo.toml)
    - Place build artifacts and compiled outputs
    - Store development documentation and README files
    - Example: /session/code/my_script.py, /session/code/package.json

  /session/data/ - Session-specific data and working files:
    - Store input data files, datasets, and raw materials
    - Save processing results, outputs, and generated reports
    - Keep temporary files and intermediate processing stages
    - Place downloaded files, API responses, and external data
    - Store analysis results and debugging information
    - Example: /session/data/dataset.csv, /session/data/results.json

  /session/logs/ - Command execution logs and system activity:
    - Automatically stores individual bash command execution logs
    - Each bash command creates a timestamped log file (bash_TIMESTAMP.log)
    - Contains command, exit code, stdout, stderr, and execution details
    - Useful for debugging, auditing, and reviewing command history
    - Not copied during session remix - logs are unique per session instance
    - Example: /session/logs/bash_1641234567.log

  /session/secrets/ - Environment variables and configuration:
    - Contains environment variables automatically sourced by the session
    - Secrets and API keys are loaded from this directory
    - Configuration files for authentication and external services
    - This directory is automatically processed - you typically don't need to manage it directly

Special Files with Automatic Processing:
  /session/code/instructions.md - Session instructions (auto-included in system prompt):
    - If this file exists, its contents are automatically appended to your system prompt
    - Use this for persistent session-specific instructions or context
    - Perfect for project requirements, coding standards, or ongoing task context
    - Contents become part of your instructions for every message in the session

  /session/code/setup.sh - Session initialization script (auto-executed on container start):
    - If this file exists, it's automatically executed when the session container starts
    - Use this for environment setup, package installation, or initial configuration
    - Runs once at the beginning of each session (including session restores)
    - Perfect for installing dependencies, setting up tools, or preparing the environment

- Use /session/code/ for anything that is executable, reusable, or represents project structure
- Use /session/data/ for files that are consumed, processed, or generated during work
- /session/logs/ contains automatic execution logs - not for user files
- All file paths should be relative to /session/ unless specifically working with system files

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
- Use the web_search tool for current information, latest updates, or when your knowledge is outdated
- Choose the right tool: bash for operations, text_editor for files, web_search for current info
- Respect user privacy and security
- When creating files, organize them appropriately:
  - Save source code, scripts, and project files to /session/code/
  - Save data files, results, and working materials to /session/data/
  - Create /session/code/instructions.md for persistent session context (auto-loaded)
  - Create /session/code/setup.sh for environment initialization (auto-executed)
- Assume the current working directory is /session/
- Show command outputs to users when relevant
- Organize files logically: code in /session/code/, data in /session/data/

Current session context:
- This is an isolated session environment with persistent storage
- Messages are persisted in the Raworc system
- You're operating as the Host (Computer Use Agent) within this session
- Your session persists between container restarts
- You have full bash access for development, analysis, and automation tasks"#
        );

        // Read instructions from /session/code/instructions.md if it exists
        let instructions_path = std::path::Path::new("/session/code/instructions.md");
        info!("Checking for instructions file at: {}", instructions_path.display());
        if instructions_path.exists() {
            info!("Instructions file exists, reading contents...");
            match tokio::fs::read_to_string(instructions_path).await {
                Ok(instructions) => {
                    info!("Read instructions content: '{}'", instructions.trim());
                    prompt.push_str("\n\nSPECIAL INSTRUCTIONS FROM USER:\n");
                    prompt.push_str(&instructions);
                    info!("Loaded instructions from /session/code/instructions.md");
                }
                Err(e) => {
                    warn!("Failed to read instructions file: {}", e);
                }
            }
        } else {
            info!("No instructions file found at {}", instructions_path.display());
        }

        prompt
    }
}