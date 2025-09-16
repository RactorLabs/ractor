use super::api::{Message, MessageRole, RaworcClient};
use super::builtin_tools::{BashTool, TextEditorTool};
use super::error::Result;
use super::gpt::GptClient;
use super::guardrails::Guardrails;
use super::tool_registry::{ContainerExecMapper, Tool, ToolRegistry};
use chrono::{DateTime, Utc};
use openai_harmony::chat::{
    Author as HAuthor, Content as HContent, Conversation as HConversation, DeveloperContent,
    Message as HMessage, Role as HRole, SystemContent, ToolDescription,
};
use openai_harmony::{load_harmony_encoding, HarmonyEncodingName};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub struct MessageHandler {
    api_client: Arc<RaworcClient>,
    gpt_client: Arc<GptClient>,
    guardrails: Arc<Guardrails>,
    processed_user_message_ids: Arc<Mutex<HashSet<String>>>,
    task_created_at: DateTime<Utc>,
    tool_registry: Arc<ToolRegistry>,
}

impl MessageHandler {
    pub fn new(
        api_client: Arc<RaworcClient>,
        gpt_client: Arc<GptClient>,
        guardrails: Arc<Guardrails>,
    ) -> Self {
        Self::new_with_registry(api_client, gpt_client, guardrails, None)
    }

    pub fn new_with_registry(
        api_client: Arc<RaworcClient>,
        gpt_client: Arc<GptClient>,
        guardrails: Arc<Guardrails>,
        tool_registry: Option<Arc<ToolRegistry>>,
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

        // Initialize tool registry with built-in tools if not provided
        let tool_registry = if let Some(registry) = tool_registry {
            registry
        } else {
            let registry = Arc::new(ToolRegistry::new());

            // Register built-in tools
            let bash_tool = Box::new(BashTool);
            let text_editor_tool = Box::new(TextEditorTool);

            tokio::spawn({
                let registry = registry.clone();
                let api_client_clone = api_client.clone();
                async move {
                    registry.register_tool(bash_tool).await;
                    registry.register_tool(text_editor_tool).await;

                    // Register management tools (publish, sleep)
                    let publish_tool = Box::new(super::builtin_tools::PublishTool::new(
                        api_client_clone.clone(),
                    ));
                    let sleep_tool = Box::new(super::builtin_tools::SleepTool::new(
                        api_client_clone.clone(),
                    ));
                    registry.register_tool(publish_tool).await;
                    registry.register_tool(sleep_tool).await;

                    // Register container.exec alias for bash
                    registry
                        .register_alias(
                            "container.exec",
                            "bash",
                            Some(Box::new(ContainerExecMapper)),
                        )
                        .await;

                    info!("Registered built-in tools and aliases");
                }
            });

            registry
        };

        Self {
            api_client,
            gpt_client,
            guardrails,
            processed_user_message_ids: Arc::new(Mutex::new(HashSet::new())),
            task_created_at,
            tool_registry,
        }
    }

    /// Initialize message processing based on task creation time.
    /// Only messages created after the controller task was created should be processed.
    pub async fn initialize_processed_tracking(&self) -> Result<()> {
        info!("Initializing timestamp-based message tracking...");
        info!("Task creation time: {}", self.task_created_at);

        // Fetch all messages to initialize tracking
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
        // Get the latest window of messages (tail of stream)
        let total = self.api_client.get_message_count().await.unwrap_or(0);
        let window: u32 = 50;
        let offset = if total > window as u64 {
            (total - window as u64) as u32
        } else {
            0
        };
        let recent_messages = self
            .api_client
            .get_messages(Some(window), Some(offset))
            .await?;

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

        // Pure Harmony: do not execute user-specified JSON tool calls directly

        // Harmony-driven loop
        let enc = load_harmony_encoding(HarmonyEncodingName::HarmonyGptOss).map_err(|e| {
            super::error::HostError::Model(format!("Failed to load Harmony encoding: {}", e))
        })?;

        let mut tool_messages: Vec<HMessage> = Vec::new();
        for step in 0..10 {
            let convo = self
                .build_harmony_conversation(&enc, message, &tool_messages)
                .await?;
            let tokens = enc
                .render_conversation_for_completion(&convo, HRole::Assistant, None)
                .map_err(|e| {
                    super::error::HostError::Model(format!("Failed to render conversation: {}", e))
                })?;
            let prompt = enc.tokenizer().decode_utf8(&tokens).map_err(|e| {
                super::error::HostError::Model(format!("Failed to decode prompt: {}", e))
            })?;

            let completion = match self
                .gpt_client
                .generate(
                    &prompt,
                    Some(serde_json::json!({ "max_new_tokens": 1024, "stop": ["<|end|>", "<|call|>", "<|return|>"] })),
                )
                .await
            {
                Ok(text) => text,
                Err(e) => {
                    warn!("GPT server failed: {}", e);
                    self.finalize_with_fallback(&message.content).await?;
                    return Ok(());
                }
            };

            let completion_tokens = enc.tokenizer().encode_with_special_tokens(&completion);
            let parsed = match enc
                .parse_messages_from_completion_tokens(completion_tokens, Some(HRole::Assistant))
            {
                Ok(p) => p,
                Err(e) => {
                    warn!("Harmony parse failed: {} — sending raw text", e);
                    let sanitized = self.guardrails.validate_output(&completion)?;
                    let meta = serde_json::json!({ "type": "model_response", "model": "gpt-oss", "fallback": "raw_text_parse_error" });
                    self.api_client.send_message(sanitized, Some(meta)).await?;
                    return Ok(());
                }
            };

            let mut made_tool_call = false;
            for m in parsed.iter() {
                if m.author.role != HRole::Assistant {
                    continue;
                }

                // commentary/analysis
                if let Some(ch) = m.channel.as_deref() {
                    if ch == "analysis" || ch == "commentary" {
                        if let Some(text) = Self::first_text(&m.content) {
                            let sanitized = self.guardrails.validate_output(text)?;
                            let meta = serde_json::json!({ "type": "assistant_commentary", "model": "gpt-oss" });
                            let _ = self.api_client.send_message(sanitized, Some(meta)).await;
                        }
                        continue;
                    }
                }

                // Tool call
                if let Some(recipient) = m.recipient.as_deref() {
                    if let Some(tool_name) = recipient.strip_prefix("functions.") {
                        let args_text = Self::first_text(&m.content).unwrap_or("");
                        let args_json: serde_json::Value = serde_json::from_str(args_text)
                            .unwrap_or_else(|_| serde_json::json!({}));

                        let desc = self.describe_tool_call(tool_name, &args_json);
                        self.send_tool_message(&desc, tool_name, Some(&args_json))
                            .await?;

                        let result =
                            match self.tool_registry.execute_tool(tool_name, &args_json).await {
                                Ok(r) => r,
                                Err(e) => format!("[error] {}", e),
                            };

                        let display = self.truncate_tool_result(&result);
                        let mut meta =
                            serde_json::json!({ "type": "tool_result", "tool_type": tool_name });
                        if let Some(obj) = meta.as_object_mut() {
                            obj.insert("args".to_string(), args_json.clone());
                        }
                        let _ = self.api_client.send_message(display, Some(meta)).await;

                        tool_messages.push(HMessage {
                            author: HAuthor::new(HRole::Tool, tool_name.to_string()),
                            recipient: None,
                            content: vec![HContent::from(result)],
                            channel: None,
                            content_type: None,
                        });

                        made_tool_call = true;
                        continue;
                    }
                }

                // Final answer
                if let Some(text) = Self::first_text(&m.content) {
                    let sanitized = self.guardrails.validate_output(text)?;
                    let meta = serde_json::json!({ "type": "model_response", "model": "gpt-oss" });
                    self.api_client.send_message(sanitized, Some(meta)).await?;
                    return Ok(());
                }
            }

            if !made_tool_call {
                warn!("Harmony completion had no actionable assistant content; sending raw text");
                let sanitized = self.guardrails.validate_output(&completion)?;
                let meta = serde_json::json!({ "type": "model_response", "model": "gpt-oss", "fallback": "raw_text_no_action" });
                self.api_client.send_message(sanitized, Some(meta)).await?;
                return Ok(());
            }

            info!("Harmony loop step {} completed; continuing", step + 1);
        }

        self.finalize_with_note("step cap reached").await?;
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

    #[allow(dead_code)]
    fn extract_tool_call_from_raw(_raw: &str) -> Option<(String, serde_json::Value)> {
        None
    }

    async fn build_harmony_conversation(
        &self,
        _enc: &openai_harmony::HarmonyEncoding,
        current: &Message,
        tool_messages: &[HMessage],
    ) -> Result<HConversation> {
        // System content: set current date (channel config defaults are included by Harmony)
        let system =
            SystemContent::new().with_conversation_start_date(chrono::Utc::now().to_rfc3339());

        // Developer tools + optional instructions
        let mut dev =
            DeveloperContent::new().with_function_tools(self.collect_function_tools().await?);
        if let Ok(text) = tokio::fs::read_to_string("/agent/code/instructions.md").await {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                dev = dev.with_instructions(trimmed.to_string());
            }
        }

        let mut msgs: Vec<HMessage> = Vec::new();
        msgs.push(HMessage::from_role_and_content(HRole::System, system));
        msgs.push(HMessage::from_role_and_content(HRole::Developer, dev));

        let history = self.api_client.get_messages(None, None).await?;
        for m in history.iter().filter(|m| m.id != current.id) {
            match m.role {
                MessageRole::User => msgs.push(HMessage::from_role_and_content(
                    HRole::User,
                    m.content.clone(),
                )),
                MessageRole::Agent => {
                    if let Some(meta) = &m.metadata {
                        if meta.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                            let tool_nm = meta
                                .get("tool_type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("tool");
                            msgs.push(HMessage {
                                author: HAuthor::new(HRole::Tool, tool_nm.to_string()),
                                recipient: None,
                                content: vec![HContent::from(m.content.clone())],
                                channel: None,
                                content_type: None,
                            });
                        } else {
                            msgs.push(HMessage::from_role_and_content(
                                HRole::Assistant,
                                m.content.clone(),
                            ));
                        }
                    } else {
                        msgs.push(HMessage::from_role_and_content(
                            HRole::Assistant,
                            m.content.clone(),
                        ));
                    }
                }
                MessageRole::System => {}
            }
        }

        msgs.extend_from_slice(tool_messages);
        msgs.push(HMessage::from_role_and_content(
            HRole::User,
            current.content.clone(),
        ));
        Ok(HConversation::from_messages(msgs))
    }

    async fn send_tool_message(
        &self,
        description: &str,
        tool_name: &str,
        args: Option<&serde_json::Value>,
    ) -> Result<()> {
        // Log tool execution notification to Docker logs
        println!("TOOL_EXECUTION: {}: {}", tool_name, description);

        // Send tool execution message to user
        let mut meta = serde_json::json!({
            "type": "tool_execution",
            "tool_type": tool_name
        });
        if let Some(a) = args {
            if let Some(obj) = meta.as_object_mut() {
                obj.insert("args".to_string(), a.clone());
            }
        }
        self.api_client
            .send_message(description.to_string(), Some(meta))
            .await?;

        Ok(())
    }

    fn truncate_tool_result(&self, tool_result: &str) -> String {
        const MAX_DISPLAY_LENGTH: usize = 8192; // 8KB limit for message display
        const TRUNCATION_PREVIEW: usize = 1024; // Show first 1KB, then summary

        if tool_result.len() <= MAX_DISPLAY_LENGTH {
            return tool_result.to_string();
        }

        // Count lines and estimate content for summary
        let lines: Vec<&str> = tool_result.lines().collect();
        let line_count = lines.len();
        let char_count = tool_result.len();

        // Take first portion and add summary
        let preview = tool_result
            .chars()
            .take(TRUNCATION_PREVIEW)
            .collect::<String>();

        format!(
            "{}\n\n[OUTPUT TRUNCATED - {} total lines, {} total characters]\n[Use more specific commands like 'ls /agent/code' (no -R) to avoid large outputs]\n[Full output available in agent logs]",
            preview,
            line_count,
            char_count
        )
    }
}

// Legacy helpers removed; Harmony loop directly parses function calls via enc.parse_messages_from_completion_tokens

// Harmony-only: removed strict JSON user tool fast-path
#[allow(dead_code)]
fn parse_user_tool_call(_s: &str) -> Option<(String, serde_json::Value)> { None }

impl MessageHandler {
    fn first_text(contents: &[HContent]) -> Option<&str> {
        for c in contents {
            if let HContent::Text(t) = c {
                return Some(t.text.as_str());
            }
        }
        None
    }

    async fn collect_function_tools(&self) -> Result<Vec<ToolDescription>> {
        let bash_schema = {
            let tool = BashTool;
            tool.parameters()
        };
        let text_schema = {
            let tool = TextEditorTool;
            tool.parameters()
        };
        let manage_schema = serde_json::json!({
            "type": "object",
            "properties": {"note": {"type": "string", "description": "Optional reason or note"}}
        });
        Ok(vec![
            ToolDescription::new(
                "bash",
                "Execute a bash shell command in the /agent directory",
                Some(bash_schema),
            ),
            ToolDescription::new(
                "text_editor",
                "Perform text editing operations on files in the /agent directory",
                Some(text_schema),
            ),
            ToolDescription::new(
                "publish",
                "Publish agent content snapshot",
                Some(manage_schema.clone()),
            ),
            ToolDescription::new("sleep", "Put the agent to sleep", Some(manage_schema)),
        ])
    }

    fn describe_tool_call(&self, tool_name: &str, args: &serde_json::Value) -> String {
        match tool_name {
            "bash" => args
                .get("command")
                .and_then(|v| v.as_str())
                .or_else(|| args.get("cmd").and_then(|v| v.as_str()))
                .map(|s| s.to_string())
                .unwrap_or_else(|| "bash command".to_string()),
            "text_editor" => {
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("edit");
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("{} {}", action, path)
            }
            other => format!("Executing {} tool", other),
        }
    }
}
