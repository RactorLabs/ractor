use super::agent_manager::AgentManager;
use super::api::{RaworcClient, Message, MessageRole, SESSION_STATE_IDLE, SESSION_STATE_BUSY, MESSAGE_ROLE_USER};
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
    agent_manager: Arc<Mutex<AgentManager>>,
    processed_message_ids: Arc<Mutex<HashSet<String>>>,
}

impl MessageHandler {
    pub fn new(
        api_client: Arc<RaworcClient>,
        claude_client: Arc<ClaudeClient>,
        guardrails: Arc<Guardrails>,
        agent_manager: Arc<Mutex<AgentManager>>,
    ) -> Self {
        Self {
            api_client,
            claude_client,
            guardrails,
            agent_manager,
            processed_message_ids: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Initialize processed message IDs by checking existing messages in the session.
    /// This prevents reprocessing messages when a session is restored.
    pub async fn initialize_processed_messages(&self) -> Result<()> {
        info!("Initializing processed message tracking...");
        
        // Fetch all existing messages
        let all_messages = self.api_client.get_messages(None, None).await?;
        
        if all_messages.is_empty() {
            info!("No existing messages found - starting fresh session");
            return Ok(());
        }

        // Build a set of all message IDs that already have responses
        let mut messages_with_responses = HashSet::new();
        let mut last_user_message_id: Option<String> = None;
        
        // Iterate through messages to find which user messages have agent responses
        for message in all_messages.iter().rev() {  // Process in chronological order
            match message.role {
                MessageRole::User => {
                    last_user_message_id = Some(message.id.clone());
                },
                MessageRole::Agent | MessageRole::System => {
                    // If this is a response to a user message, mark that user message as processed
                    if let Some(user_msg_id) = &last_user_message_id {
                        messages_with_responses.insert(user_msg_id.clone());
                    }
                }
            }
        }

        // Only mark user messages as processed if they already have responses
        // This allows new user messages after restore to be picked up properly
        let mut processed_ids = self.processed_message_ids.lock().await;
        for message in &all_messages {
            if message.role == MessageRole::User && messages_with_responses.contains(&message.id) {
                processed_ids.insert(message.id.clone());
            } else if message.role != MessageRole::User {
                // Always mark non-user messages (agent/system) as processed
                processed_ids.insert(message.id.clone());
            }
            // Don't mark user messages without responses as processed - they might need processing
        }
        
        let processed_count = processed_ids.len();
        info!("Initialized processed message tracking: {} total messages, {} with responses, {} marked as processed", 
              all_messages.len(), messages_with_responses.len(), processed_count);
        
        Ok(())
    }
    
    
    pub async fn poll_and_process(&self) -> Result<usize> {
        // Get recent messages to check for new ones
        let recent_messages = self.api_client.get_messages(Some(50), None).await?;
        
        if recent_messages.is_empty() {
            return Ok(0);
        }
        
        // Build a set of user message IDs that have responses
        let mut messages_with_responses = HashSet::new();
        let mut last_user_message_id: Option<String> = None;
        
        // Iterate through messages to find which user messages have agent responses
        for message in recent_messages.iter().rev() {  // Process in chronological order
            match message.role {
                MessageRole::User => {
                    last_user_message_id = Some(message.id.clone());
                },
                MessageRole::Agent | MessageRole::System => {
                    // If this is a response to a user message, mark that user message as processed
                    if let Some(user_msg_id) = &last_user_message_id {
                        messages_with_responses.insert(user_msg_id.clone());
                    }
                }
            }
        }
        
        // Find unprocessed user messages
        let mut processed_ids = self.processed_message_ids.lock().await;
        let mut new_messages = Vec::new();
        
        for message in recent_messages.iter() {
            if !processed_ids.contains(&message.id) {
                if message.role == MessageRole::User {
                    // Only consider it new if it doesn't have a response yet
                    if !messages_with_responses.contains(&message.id) {
                        new_messages.push(message.clone());
                    }
                }
                processed_ids.insert(message.id.clone());
            }
        }
        
        if new_messages.is_empty() {
            return Ok(0);
        }
        
        info!("Found {} new user messages to process", new_messages.len());
        
        // Update session state to BUSY
        if let Err(e) = self.api_client.update_session_state(SESSION_STATE_BUSY.to_string()).await {
            warn!("Failed to update session state to BUSY: {}", e);
        }
        
        // Process each new message
        for message in new_messages.iter() {
            if let Err(e) = self.process_message(message).await {
                error!("Failed to process message {}: {}", message.id, e);
                
                // Generate error response so user gets feedback
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
        }
        
        // Update session state back to IDLE
        if let Err(e) = self.api_client.update_session_state(SESSION_STATE_IDLE.to_string()).await {
            warn!("Failed to update session state to IDLE: {}", e);
        }
        
        Ok(new_messages.len())
    }
    
    async fn process_message(&self, message: &Message) -> Result<()> {
        info!("Processing message: {}", message.id);
        
        // Validate input with guardrails
        self.guardrails.validate_input(&message.content)?;
        

        // Try agent delegation first
        if let Some(response) = self.try_agent_delegation(&message.content).await? {
            // Send agent response
            self.api_client.send_message(
                response,
                Some(serde_json::json!({
                    "type": "agent_response"
                })),
            ).await?;
            return Ok(());
        }
        
        // Fallback to Claude API if no agents available or delegation failed
        info!("No suitable agent found, using Claude API");
        
        // Fetch ALL messages from session for complete conversation history
        info!("Fetching complete conversation history for Claude");
        let all_messages = self.fetch_all_session_messages().await?;
        
        // Prepare conversation history for Claude
        let conversation = self.prepare_conversation_history(&all_messages, &message.id);
        
        // Get Claude's response with fallback
        let system_prompt = self.build_system_prompt();
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
                    I'm a Raworc host agent designed to help with various tasks including:\n\
                    - Code generation and analysis\n\
                    - File operations\n\
                    - Agent delegation\n\n\
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
                "model": "claude-3-5-sonnet-20241022"
            })),
        ).await?;
        
        Ok(())
    }
    
    async fn try_agent_delegation(&self, message_content: &str) -> Result<Option<String>> {
        let agent_manager = self.agent_manager.lock().await;
        
        // Use Claude-powered delegation to find best agent
        if let Some(agent_name) = agent_manager.get_agent_for_message(message_content).await {
            info!("Claude delegating message to agent: {}", agent_name);
            
            // Prepare context for agent
            let context = serde_json::json!({
                "session_id": std::env::var("RAWORC_SESSION_ID").unwrap_or_default(),
                "space": std::env::var("RAWORC_SPACE_ID").unwrap_or_else(|_| "default".to_string()),
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            });
            
            // Execute agent
            match agent_manager.execute_agent(&agent_name, message_content, &context).await {
                Ok(response) => {
                    info!("Agent {} executed successfully via Claude delegation", agent_name);
                    return Ok(Some(response));
                }
                Err(e) => {
                    warn!("Agent {} execution failed: {}, falling back to Claude", agent_name, e);
                    return Ok(None);
                }
            }
        }
        
        Ok(None)
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
            .filter(|m| m.role == MessageRole::User || m.role == MessageRole::Agent)
            .map(|m| {
                let role = match m.role {
                    MessageRole::User => MESSAGE_ROLE_USER,
                    MessageRole::Agent => "assistant", // Claude expects "assistant" not "agent"  
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
    
    fn build_system_prompt(&self) -> String {
        format!(
            r#"You are a helpful AI assistant operating within a Raworc session.

Key capabilities:
- You can help users with various tasks and answer questions
- You maintain conversation context within this session
- You can create, read, and modify files within the session directory

Working Directory and File Operations:
- Your working directory is /session/
- When creating files, writing code, or performing file operations, use /session/ as your base directory
- The session has persistent storage mounted at /session/ with the following structure:
  - /session/ - Main working directory for user files and code
  - /session/agents/ - Agent deployments and builds (managed automatically)
  - /session/cache/ - Build caches for cargo, pip, npm, git
  - /session/state/ - Session state
- All file paths should be relative to /session/ unless specifically working with system files

Guidelines:
- Be helpful, accurate, and concise
- Respect user privacy and security
- Do not execute or suggest harmful commands
- If asked to perform actions outside your capabilities, explain your limitations
- When generating code or creating files, place them in /session/
- Assume the current working directory is /session/


Current session context:
- This is an isolated session environment with persistent storage
- Messages are persisted in the Raworc system
- You're operating as an agent within this session
- Your session persists between container restarts"#
        )
    }
}