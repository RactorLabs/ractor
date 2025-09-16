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

        let is_compact_request = message
            .metadata
            .as_ref()
            .and_then(|m| m.get("type"))
            .and_then(|v| v.as_str())
            .map(|s| s == "compact_request")
            .unwrap_or(false);

        // Pure Harmony: do not execute user-specified JSON tool calls directly

        // Harmony-driven loop
        let enc = load_harmony_encoding(HarmonyEncodingName::HarmonyGptOss).map_err(|e| {
            super::error::HostError::Model(format!("Failed to load Harmony encoding: {}", e))
        })?;

        let mut tool_messages: Vec<HMessage> = Vec::new();
        let mut step: u32 = 0;
        loop {
            // If compaction is in progress and this is not a compact_request, skip processing
            if !is_compact_request {
                if let Ok(agent) = self.api_client.get_agent().await {
                    if agent
                        .metadata
                        .get("compact_in_progress")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    {
                        tracing::info!("Compaction active — skipping normal message processing");
                        return Ok(());
                    }
                }
            }
            step += 1;
            // Build full message vector to allow token accounting by parts
            let msgs_full = self
                .build_harmony_messages(&enc, message, &tool_messages)
                .await?;

            // Token accounting by parts (system+dev, history, tools, user)
            let (mut sys_dev_end, mut hist_end, mut tools_end) = (0usize, 0usize, 0usize);
            // Identify boundaries consistent with build_harmony_messages
            // indexes: [0]=system, [1]=developer, [2..hist_end)=history, [hist_end..tools_end)=tool msgs, last=user
            sys_dev_end = 2.min(msgs_full.len());
            // Find start index of tool_messages by scanning from the end: last is user
            // We don't store explicit counts, but we can derive by removing last (user) and subtracting current tool_messages length
            let user_index = msgs_full.len().saturating_sub(1);
            // History spans from sys_dev_end up to (user_index - tool_messages.len())
            let tool_count = tool_messages.len();
            tools_end = user_index;
            hist_end = tools_end.saturating_sub(tool_count);

            // Helper to render token count
            let mut render_tokens = |slice: &[HMessage]| -> Result<usize> {
                let convo = HConversation::from_messages(slice.to_vec());
                let v = enc
                    .render_conversation_for_completion(&convo, HRole::Assistant, None)
                    .map_err(|e| super::error::HostError::Model(format!("Failed to render conversation: {}", e)))?;
                Ok(v.len())
            };

            // Compute token counts
            let tokens_sysdev = render_tokens(&msgs_full[..sys_dev_end])?;
            let tokens_sysdev_hist = render_tokens(&msgs_full[..hist_end])?;
            let tokens_sysdev_hist_tools = render_tokens(&msgs_full[..tools_end])?;
            let tokens_all = render_tokens(&msgs_full[..])?;
            let tokens_history = tokens_sysdev_hist.saturating_sub(tokens_sysdev);
            let tokens_tools = tokens_sysdev_hist_tools.saturating_sub(tokens_sysdev_hist);
            let tokens_user = tokens_all.saturating_sub(tokens_sysdev_hist_tools);

            // Render full prompt for logging and server
            let convo = HConversation::from_messages(msgs_full.clone());
            let tokens = enc
                .render_conversation_for_completion(&convo, HRole::Assistant, None)
                .map_err(|e| {
                    super::error::HostError::Model(format!("Failed to render conversation: {}", e))
                })?;
            let prompt = enc.tokenizer().decode_utf8(&tokens).map_err(|e| {
                super::error::HostError::Model(format!("Failed to decode prompt: {}", e))
            })?;

            // Prepare logging context and generation params
            let request_id = format!("{}:{}", message.id, step);
            // Optional max_new_tokens via env; if unset, omit (server uses its own default)
            let max_new_env: Option<u32> = std::env::var("RAWORC_MAX_NEW_TOKENS")
                .ok()
                .and_then(|s| s.parse::<u32>().ok());
            let mut gp = serde_json::Map::new();
            gp.insert("stop".to_string(), serde_json::json!(["<|end|>", "<|call|>", "<|return|>"]));
            if let Some(n) = max_new_env { gp.insert("max_new_tokens".to_string(), serde_json::json!(n)); }
            let gen_params = serde_json::Value::Object(gp);
            // Harmony logs are enabled by default for debugging. Disable by setting RAWORC_LOG_HARMONY to a falsey value (0,false,no,off).
            // You can also toggle via /agent/secrets/log_harmony or /agent/logs/log_harmony files.
            let log_env = std::env::var("RAWORC_LOG_HARMONY").unwrap_or_else(|_| "true".to_string()).to_lowercase();
            let mut log_full = !matches!(log_env.as_str(), "0" | "false" | "no" | "off");
            // Optional file overrides: if explicitly falsey, turn off
            for p in [
                "/agent/secrets/log_harmony",
                "/agent/logs/log_harmony",
            ] {
                if let Ok(s) = std::fs::read_to_string(p) {
                    let t = s.trim().to_lowercase();
                    if matches!(t.as_str(), "0" | "false" | "no" | "off") {
                        log_full = false;
                        break;
                    }
                }
            }
            if log_full {
                tracing::info!(
                    target: "gpt",
                    request_id = %request_id,
                    params = %gen_params.to_string(),
                    prompt_len = prompt.len(),
                    "HARMONY_MODEL_REQUEST\n{}",
                    prompt
                );
            } else {
                tracing::debug!(
                    target: "gpt",
                    request_id = %request_id,
                    params = %gen_params.to_string(),
                    prompt_len = prompt.len(),
                    "HARMONY_MODEL_REQUEST"
                );
            }

            let gen = self
                .gpt_client
                .generate(
                    &prompt,
                    Some(gen_params.clone()),
                )
                .await
                .map_err(|e| super::error::HostError::Model(format!("GPT server failed: {}", e)))?;
            let completion = gen.text.clone();

            // Log model response text (full only if enabled)
            if log_full {
                tracing::info!(
                    target: "gpt",
                    request_id = %request_id,
                    response_len = completion.len(),
                    "HARMONY_MODEL_RESPONSE\n{}",
                    completion
                );
            } else {
                tracing::debug!(
                    target: "gpt",
                    request_id = %request_id,
                    response_len = completion.len(),
                    "HARMONY_MODEL_RESPONSE"
                );
            }

            let completion_tokens = enc.tokenizer().encode_with_special_tokens(&completion);
            let parsed = match enc
                .parse_messages_from_completion_tokens(completion_tokens, Some(HRole::Assistant))
            {
                Ok(p) => p,
                Err(e) => {
                    // Clean fix: rely on normalization to avoid malformed roles; if it still fails,
                    // return a clear model error upward without trying to inject raw transcripts.
                    return Err(super::error::HostError::Model(format!(
                        "Harmony parse failed: {}",
                        e
                    )));
                }
            };
            tracing::debug!(target: "gpt", request_id = %request_id, parsed_count = parsed.len(), "HARMONY_PARSE_OK");

            // First pass: commentary + tool_call only (no execution)
            let mut segments_pre: Vec<serde_json::Value> = Vec::new();
            let mut tool_execs: Vec<(String, serde_json::Value)> = Vec::new();
            let mut final_msg: Option<(String, String)> = None;

            for m in parsed.iter() {
                if m.author.role != HRole::Assistant { continue; }

                if let Some(recipient) = m.recipient.as_deref() {
                    if let Some(tool_name) = recipient.strip_prefix("functions.") {
                        let args_text = Self::first_text(&m.content).unwrap_or("");
                        let args_json: serde_json::Value = serde_json::from_str(args_text).unwrap_or_else(|_| serde_json::json!({}));
                        segments_pre.push(serde_json::json!({
                            "type": "tool_call",
                            "tool": tool_name,
                            "args": args_json,
                        }));
                        tool_execs.push((tool_name.to_string(), args_json));
                        continue;
                    }
                }

                if let Some(ch) = m.channel.as_deref() {
                    if ch == "analysis" || ch == "commentary" {
                        if let Some(text) = Self::first_text(&m.content) {
                            let sanitized = self.guardrails.validate_output(text)?;
                            segments_pre.push(serde_json::json!({
                                "type": "commentary",
                                "channel": ch,
                                "text": sanitized,
                            }));
                            continue;
                        }
                    }
                }

                if final_msg.is_none() {
                    if let Some(text) = Self::first_text(&m.content) {
                        let ch = m.channel.clone().unwrap_or_else(|| "final".to_string());
                        final_msg = Some((ch, text.to_string()));
                    }
                }
            }

            // Create in-progress message with pre-segments
            let mut created_message_id: Option<String> = None;
            if !segments_pre.is_empty() || final_msg.is_some() {
                let mut tm = serde_json::json!({
                    "prompt_tokens": tokens.len(),
                    "prompt_bytes": prompt.len(),
                    "parts": {
                        "system_dev": tokens_sysdev,
                        "history": tokens_history,
                        "tools": tokens_tools,
                        "user": tokens_user
                    }
                });
                // Budget info (for UI)
                let max_ctx: usize = std::env::var("RAWORC_MAX_CONTEXT_TOKENS").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(8192);
                let reserve: usize = std::env::var("RAWORC_COMPLETION_MARGIN").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(1024);
                tm["budget"] = serde_json::json!({ "max_tokens": max_ctx, "completion_margin": reserve });
                if let Some(u) = gen.usage.as_ref() {
                    tm["server_usage"] = serde_json::json!({
                        "prompt_tokens": u.prompt_tokens,
                        "completion_tokens": u.completion_tokens,
                        "total_tokens": u.total_tokens,
                        "gen_ms": u.gen_ms
                    });
                }
                let meta = serde_json::json!({
                    "type": "model_response_step",
                    "model": "gpt-oss",
                    "step": step,
                    "has_final": false,
                    "in_progress": true,
                    "token_metrics": tm,
                });
                let content_json_pre = serde_json::json!({ "harmony": { "request_id": request_id, "segments": segments_pre } });
                let created = self
                    .api_client
                    .send_message_structured(
                        super::api::MessageRole::Agent,
                        "".to_string(),
                        Some(meta),
                        None,
                        None,
                        None,
                        None,
                        Some(content_json_pre),
                    )
                    .await?;
                created_message_id = Some(created.id.clone());
            }

            // Execute tools and append results
            let mut segments_all = segments_pre.clone();
            for (tool_name, args_json) in tool_execs.into_iter() {
                let result = match self.tool_registry.execute_tool(&tool_name, &args_json).await {
                    Ok(r) => r,
                    Err(e) => format!("[error] {}", e),
                };
                segments_all.push(serde_json::json!({
                    "type": "tool_result",
                    "tool": tool_name,
                    "args": args_json,
                    "output": result,
                }));
                tool_messages.push(HMessage {
                    author: HAuthor::new(HRole::Tool, tool_name.to_string()),
                    recipient: None,
                    content: vec![HContent::from(result)],
                    channel: None,
                    content_type: None,
                });
            }

            // Auto-finalize ONLY for sleep (terminal action). Publish is not auto-finalized.
            if final_msg.is_none() {
                if segments_all.iter().any(|s| s.get("type").and_then(|v| v.as_str()) == Some("tool_result") && s.get("tool").and_then(|v| v.as_str()) == Some("sleep")) {
                    final_msg = Some(("final".to_string(), "Agent is going to sleep.".to_string()));
                }
            }

            let mut content_str = String::new();
            let mut channel_for_row: Option<String> = None;
            if let Some((ch, text)) = final_msg {
                let sanitized = self.guardrails.validate_output(&text)?;
                segments_all.push(serde_json::json!({ "type": "final", "channel": ch, "text": sanitized }));
                content_str = sanitized;
                channel_for_row = Some(ch);
            }

            // Update created message with results/final
            if let Some(msg_id) = created_message_id.as_ref() {
                let mut tm = serde_json::json!({
                    "prompt_tokens": tokens.len(),
                    "prompt_bytes": prompt.len(),
                    "parts": {
                        "system_dev": tokens_sysdev,
                        "history": tokens_history,
                        "tools": tokens_tools,
                        "user": tokens_user
                    }
                });
                // Budget info (for UI)
                let max_ctx: usize = std::env::var("RAWORC_MAX_CONTEXT_TOKENS").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(8192);
                let reserve: usize = std::env::var("RAWORC_COMPLETION_MARGIN").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(1024);
                tm["budget"] = serde_json::json!({ "max_tokens": max_ctx, "completion_margin": reserve });
                if let Some(u) = gen.usage.as_ref() {
                    tm["server_usage"] = serde_json::json!({
                        "prompt_tokens": u.prompt_tokens,
                        "completion_tokens": u.completion_tokens,
                        "total_tokens": u.total_tokens,
                        "gen_ms": u.gen_ms
                    });
                }
                let meta = serde_json::json!({
                    "type": "model_response_step",
                    "model": "gpt-oss",
                    "step": step,
                    "has_final": channel_for_row.is_some(),
                    "in_progress": false,
                    "token_metrics": tm,
                });
                let content_json_all = serde_json::json!({ "harmony": { "request_id": request_id, "segments": segments_all } });
                let update_req = super::api::UpdateMessageRequest {
                    content: Some(content_str.clone()),
                    metadata: Some(meta),
                    author_name: None,
                    recipient: None,
                    channel: Some(channel_for_row.clone()),
                    content_type: None,
                    content_json: Some(Some(content_json_all)),
                };
                let _ = self.api_client.update_message(&msg_id, update_req).await;
            }

            // If final was present and no more tools to run, finish
            if channel_for_row.is_some() && tool_messages.is_empty() {
                // If this was a compact request, clear compact_in_progress flag
                if is_compact_request {
                    if let Ok(agent) = self.api_client.get_agent().await {
                        let mut meta = agent.metadata.clone();
                        let make_obj = !meta.is_object();
                        if make_obj { meta = serde_json::json!({}); }
                        let obj = meta.as_object_mut().expect("metadata object");
                        obj.insert("compact_in_progress".to_string(), serde_json::json!(false));
                        obj.insert("compact_last_at".to_string(), serde_json::json!(chrono::Utc::now().to_rfc3339()));
                        let _ = self.api_client.update_agent_metadata(meta).await;
                    }
                }
                return Ok(());
            }

            // If we executed tools or appended results, update the previously created message with results/final
            if let Some(msg_id) = created_message_id.as_ref() {
                let mut meta = serde_json::json!({
                    "type": "model_response_step",
                    "model": "gpt-oss",
                    "step": step,
                    "has_final": channel_for_row.is_some(),
                    "in_progress": false,
                });
                let content_json = serde_json::json!({
                    "harmony": {
                        "request_id": request_id,
                        "segments": segments_all,
                    }
                });
                let update_req = super::api::UpdateMessageRequest {
                    content: Some(content_str.clone()),
                    metadata: Some(meta),
                    author_name: None,
                    recipient: None,
                    channel: Some(channel_for_row.clone()),
                    content_type: None,
                    content_json: Some(Some(content_json)),
                };
                let _ = self.api_client.update_message(&msg_id, update_req).await;
            }

            // If tools were executed and no final yet, continue the loop
            if !tool_messages.is_empty() && channel_for_row.is_none() {
                info!("Harmony loop step {} completed; continuing", step);
                continue;
            }

            // Otherwise nothing actionable
            tracing::warn!("Harmony completion had no actionable assistant content");
            return Ok(());
        }
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
        // System content: require Harmony channels and set start date
        let system = SystemContent::new()
            .with_conversation_start_date(chrono::Utc::now().to_rfc3339())
            .with_required_channels(["analysis", "commentary", "final"]);

        // Developer tools + guidance: combine legacy 0.7.7 prompt with the newer Harmony guidance
        let mut dev = DeveloperContent::new().with_function_tools(self.collect_function_tools().await?);
        let legacy_prompt = self.build_legacy_system_prompt().await;
        let tool_guidance = r#"
Use function tools to act.
Channels:
- analysis/commentary: internal thinking. Do not reveal this in final.
- final: user-visible answer only.

Tools:
- Use functions.bash for shell commands (args: {"command": "..."}).
- Use functions.text_editor for file edits (actions: view/create/write/str_replace/insert).

File editing guidance:
- For new files, prefer a single create with the full content (one call).
- Do not attempt 'create' on a path that already exists. If the user wants to overwrite, use action: "write"; otherwise choose a new filename.
- Avoid many incremental insert/replace calls to build a new file; write the complete content in one go.
- Use relative paths only (no leading '/'); paths are rooted at /agent.
- Only use insert/str_replace for targeted updates to existing files.
- Do not call view before create when creating a brand new file unless verifying it already exists.

Termination & duplication rules:
- After a successful create/write that satisfies the user's request, emit a 'final' immediately and stop; do not keep iterating.
- If a tool returns an error like "file already exists" for create, do not retry the same action/path. Decide (write vs new name) and state the decision in 'final'.
- If multiple files are needed, include all text_editor tool calls in the same step if possible, then produce one 'final' summarizing the created paths.
 - Sleep is terminal: after issuing the sleep tool and receiving its result, emit a 'final' stating the agent will sleep, then stop.
 - Sleep is terminal: after issuing the sleep tool and receiving its result, emit a 'final' stating the agent will sleep, then stop.

Publishing rules:
- Publish only if the user asked to view/share/open the created content or the task explicitly requires a public URL. Otherwise, confirm creation and provide the relative path (e.g., content/foo.html) in 'final'.
- When you do publish, include the absolute published URL(s) only in 'final' (never in analysis/commentary).
 - Publish is not terminal by itself; after publishing, you should still produce a normal 'final' message summarizing results.
 - Publish is not terminal by itself; after publishing, you should still produce a normal 'final' message summarizing results.

Constraints:
- Tool call content must be valid JSON per schema. No extra prose.
- Prefer concise, accurate results in 'final' after tools complete.
"#;
        let combined_instructions = format!("{}\n\n{}", legacy_prompt, tool_guidance);
        dev = dev.with_instructions(combined_instructions);

        let mut msgs: Vec<HMessage> = Vec::new();
        msgs.push(HMessage::from_role_and_content(HRole::System, system));
        msgs.push(HMessage::from_role_and_content(HRole::Developer, dev));

        // Use a bounded history window to keep prompt size manageable (override via RAWORC_HISTORY_LIMIT)
        let hist_limit: u32 = std::env::var("RAWORC_HISTORY_LIMIT")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(40);
        let history = self.api_client.get_messages(Some(hist_limit), None).await?;
        // Align with baseline: filter out messages before metadata.compact_from
        let mut since_ts: Option<chrono::DateTime<chrono::Utc>> = None;
        if let Ok(agent) = self.api_client.get_agent().await {
            if let Some(obj) = agent.metadata.as_object() {
                if let Some(ts) = obj.get("compact_from").and_then(|v| v.as_str()) {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                        since_ts = Some(dt.with_timezone(&chrono::Utc));
                    }
                }
            }
        }

        // Reconstruct conversation using Harmony composite segments when available.
        // - For prior agent turns: include only the final text (as assistant, channel "final")
        //   and each tool_result (as role=tool, author=name=tool, content=output).
        // - Skip analysis/commentary.
        // - Ignore in-progress rows and prefer the latest row per request_id.
        use std::collections::{HashMap, VecDeque};
        #[derive(Default, Clone)]
        struct Turn {
            final_text: Option<String>,
            tool_results: Vec<(String, String)>, // (tool, output)
        }

        // Maintain insertion order of request_ids
        let mut req_order: VecDeque<String> = VecDeque::new();
        let mut turns_by_req: HashMap<String, Turn> = HashMap::new();

        for m in history.iter().filter(|m| m.id != current.id).filter(|m| {
            if let Some(since) = since_ts {
                if let Ok(mt) = chrono::DateTime::parse_from_rfc3339(&m.created_at) {
                    return mt.with_timezone(&chrono::Utc) >= since;
                }
            }
            true
        }) {
            match m.role {
                MessageRole::User => {
                    msgs.push(HMessage::from_role_and_content(
                        HRole::User,
                        m.content.clone(),
                    ));
                }
                MessageRole::Agent => {
                    // Check metadata.in_progress flag; if true, skip this row
                    let mut in_progress = false;
                    if let Some(meta) = &m.metadata {
                        if let Some(b) = meta.get("in_progress").and_then(|v| v.as_bool()) {
                            in_progress = b;
                        }
                    }
                    if in_progress {
                        continue;
                    }

                    // Parse Harmony segments if present
                    let (mut req_id_opt, mut turn) = (None::<String>, Turn::default());
                    if let Some(cj) = &m.content_json {
                        let req_id = cj
                            .get("harmony")
                            .and_then(|h| h.get("request_id"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        if let Some(segs) = cj
                            .get("harmony")
                            .and_then(|h| h.get("segments"))
                            .and_then(|v| v.as_array())
                        {
                            // Collect last final text and all tool_results
                            for seg in segs {
                                let t = seg.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                match t {
                                    "final" => {
                                        if let Some(txt) = seg.get("text").and_then(|v| v.as_str()) {
                                            turn.final_text = Some(txt.to_string());
                                        }
                                    }
                                    "tool_result" => {
                                        let tool = seg
                                            .get("tool")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        // Output can be string or object; stringify if not string
                                        let output_val = seg.get("output");
                                        let output = match output_val {
                                            Some(serde_json::Value::String(s)) => s.clone(),
                                            Some(v) => match serde_json::to_string(v) {
                                                Ok(s) => s,
                                                Err(_) => String::new(),
                                            },
                                            None => String::new(),
                                        };
                                        if !tool.is_empty() {
                                            turn.tool_results.push((tool, output));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        req_id_opt = req_id;
                    }

                    if let Some(req_id) = req_id_opt {
                        // Prefer the latest row per request_id; replace if already present
                        if !req_order.contains(&req_id) {
                            req_order.push_back(req_id.clone());
                        }
                        turns_by_req.insert(req_id, turn);
                    } else {
                        // Legacy or non-Harmony row: treat entire content as assistant final
                        msgs.push(HMessage {
                            author: HAuthor::new(HRole::Assistant, "assistant".to_string()),
                            recipient: None,
                            content: vec![HContent::from(m.content.clone())],
                            channel: Some("final".to_string()),
                            content_type: None,
                        });
                    }
                }
                MessageRole::System => {}
            }
        }

        // Emit reconstructed agent turns in original order
        while let Some(req_id) = req_order.pop_front() {
            if let Some(turn) = turns_by_req.get(&req_id) {
                // First, the assistant final (if any)
                if let Some(txt) = &turn.final_text {
                    msgs.push(HMessage {
                        author: HAuthor::new(HRole::Assistant, "assistant".to_string()),
                        recipient: None,
                        content: vec![HContent::from(txt.clone())],
                        channel: Some("final".to_string()),
                        content_type: None,
                    });
                }
                // Then each tool result as a tool message
                for (tool, output) in &turn.tool_results {
                    msgs.push(HMessage {
                        author: HAuthor::new(HRole::Tool, tool.clone()),
                        recipient: None,
                        content: vec![HContent::from(output.clone())],
                        channel: None,
                        content_type: None,
                    });
                }
            }
        }

        msgs.extend_from_slice(tool_messages);
        msgs.push(HMessage::from_role_and_content(
            HRole::User,
            current.content.clone(),
        ));
        Ok(HConversation::from_messages(msgs))
    }

    async fn build_harmony_messages(
        &self,
        enc: &openai_harmony::HarmonyEncoding,
        current: &Message,
        tool_messages: &[HMessage],
    ) -> Result<Vec<HMessage>> {
        // System content: require Harmony channels and set start date
        let system = SystemContent::new()
            .with_conversation_start_date(chrono::Utc::now().to_rfc3339())
            .with_required_channels(["analysis", "commentary", "final"]);

        // Developer tools + guidance: combine legacy 0.7.7 prompt with the newer Harmony guidance
        let mut dev = DeveloperContent::new().with_function_tools(self.collect_function_tools().await?);
        let legacy_prompt = self.build_legacy_system_prompt().await;
        let tool_guidance = r#"
Use function tools to act.
Channels:
- analysis/commentary: internal thinking. Do not reveal this in final.
- final: user-visible answer only.

Tools:
- Use functions.bash for shell commands (args: {"command": "..."}).
- Use functions.text_editor for file edits (actions: view/create/write/str_replace/insert).

File editing guidance:
- For new files, prefer a single create with the full content (one call).
- Do not attempt 'create' on a path that already exists. If the user wants to overwrite, use action: "write"; otherwise choose a new filename.
- Avoid many incremental insert/replace calls to build a new file; write the complete content in one go.
- Use relative paths only (no leading '/'); paths are rooted at /agent.
- Only use insert/str_replace for targeted updates to existing files.
- Do not call view before create when creating a brand new file unless verifying it already exists.

Termination & duplication rules:
- After a successful create/write that satisfies the user's request, emit a 'final' immediately and stop; do not keep iterating.
- If a tool returns an error like "file already exists" for create, do not retry the same action/path. Decide (write vs new name) and state the decision in 'final'.
- If multiple files are needed, include all text_editor tool calls in the same step if possible, then produce one 'final' summarizing the created paths.

Publishing rules:
- Publish only if the user asked to view/share/open the created content or the task explicitly requires a public URL. Otherwise, confirm creation and provide the relative path (e.g., content/foo.html) in 'final'.
- When you do publish, include the absolute published URL(s) only in 'final' (never in analysis/commentary).

Constraints:
- Tool call content must be valid JSON per schema. No extra prose.
- Prefer concise, accurate results in 'final' after tools complete.
"#;
        let combined_instructions = format!("{}\n\n{}", legacy_prompt, tool_guidance);
        dev = dev.with_instructions(combined_instructions);

        let mut msgs: Vec<HMessage> = Vec::new();
        msgs.push(HMessage::from_role_and_content(HRole::System, system));
        msgs.push(HMessage::from_role_and_content(HRole::Developer, dev));

        // Use a bounded history window to keep prompt size manageable (override via RAWORC_HISTORY_LIMIT)
        let hist_limit: u32 = std::env::var("RAWORC_HISTORY_LIMIT")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(40);
        let history = self.api_client.get_messages(Some(hist_limit), None).await?;

        // Reconstruct conversation using Harmony composite segments when available.
        use std::collections::{HashMap, VecDeque};
        #[derive(Default, Clone)]
        struct Turn {
            final_text: Option<String>,
            tool_results: Vec<(String, String)>, // (tool, output)
        }
        let mut req_order: VecDeque<String> = VecDeque::new();
        let mut turns_by_req: HashMap<String, Turn> = HashMap::new();

        for m in history.iter().filter(|m| m.id != current.id) {
            match m.role {
                MessageRole::User => {
                    msgs.push(HMessage::from_role_and_content(
                        HRole::User,
                        m.content.clone(),
                    ));
                }
                MessageRole::Agent => {
                    let mut in_progress = false;
                    if let Some(meta) = &m.metadata {
                        if let Some(b) = meta.get("in_progress").and_then(|v| v.as_bool()) {
                            in_progress = b;
                        }
                    }
                    if in_progress { continue; }

                    let (mut req_id_opt, mut turn) = (None::<String>, Turn::default());
                    if let Some(cj) = &m.content_json {
                        let req_id = cj
                            .get("harmony")
                            .and_then(|h| h.get("request_id"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        if let Some(segs) = cj
                            .get("harmony")
                            .and_then(|h| h.get("segments"))
                            .and_then(|v| v.as_array())
                        {
                            for seg in segs {
                                let t = seg.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                match t {
                                    "final" => {
                                        if let Some(txt) = seg.get("text").and_then(|v| v.as_str()) {
                                            turn.final_text = Some(txt.to_string());
                                        }
                                    }
                                    "tool_result" => {
                                        let tool = seg.get("tool").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let output_val = seg.get("output");
                                        let output = match output_val {
                                            Some(serde_json::Value::String(s)) => s.clone(),
                                            Some(v) => match serde_json::to_string(v) { Ok(s) => s, Err(_) => String::new() },
                                            None => String::new(),
                                        };
                                        if !tool.is_empty() { turn.tool_results.push((tool, output)); }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        req_id_opt = req_id;
                    }

                    if let Some(req_id) = req_id_opt {
                        if !req_order.contains(&req_id) { req_order.push_back(req_id.clone()); }
                        turns_by_req.insert(req_id, turn);
                    } else {
                        msgs.push(HMessage {
                            author: HAuthor::new(HRole::Assistant, "assistant".to_string()),
                            recipient: None,
                            content: vec![HContent::from(m.content.clone())],
                            channel: Some("final".to_string()),
                            content_type: None,
                        });
                    }
                }
                MessageRole::System => {}
            }
        }

        while let Some(req_id) = req_order.pop_front() {
            if let Some(turn) = turns_by_req.get(&req_id) {
                if let Some(txt) = &turn.final_text {
                    msgs.push(HMessage {
                        author: HAuthor::new(HRole::Assistant, "assistant".to_string()),
                        recipient: None,
                        content: vec![HContent::from(txt.clone())],
                        channel: Some("final".to_string()),
                        content_type: None,
                    });
                }
                for (tool, output) in &turn.tool_results {
                    msgs.push(HMessage {
                        author: HAuthor::new(HRole::Tool, tool.clone()),
                        recipient: None,
                        content: vec![HContent::from(output.clone())],
                        channel: None,
                        content_type: None,
                    });
                }
            }
        }

        // At this point msgs = [system, developer, ...history...] in chronological order
        let history_len = if msgs.len() > 2 { msgs.len() - 2 } else { 0 };
        let mut prefix: Vec<HMessage> = Vec::new();
        let mut history_msgs: Vec<HMessage> = Vec::new();
        for (i, m) in msgs.into_iter().enumerate() {
            if i < 2 { prefix.push(m); } else { history_msgs.push(m); }
        }

        // Fixed suffix: tool messages from this loop + current user message
        let mut suffix: Vec<HMessage> = Vec::new();
        suffix.extend_from_slice(tool_messages);
        suffix.push(HMessage::from_role_and_content(HRole::User, current.content.clone()));

        // Token budget decision for history selection
        let max_ctx: usize = std::env::var("RAWORC_MAX_CONTEXT_TOKENS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(8192);
        let reserve: usize = std::env::var("RAWORC_COMPLETION_MARGIN")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1024);

        let mut count_tokens = |parts: &Vec<HMessage>| -> Result<usize> {
            let c = HConversation::from_messages(parts.clone());
            let t = enc
                .render_conversation_for_completion(&c, HRole::Assistant, None)
                .map_err(|e| super::error::HostError::Model(format!("Failed to render conversation: {}", e)))?;
            Ok(t.len())
        };

        // Try with full history first
        let mut selected_start = 0usize; // index into history_msgs
        let mut attempt: Vec<HMessage> = Vec::new();
        attempt.extend(prefix.clone());
        attempt.extend(history_msgs.clone());
        attempt.extend(suffix.clone());
        let mut toks = count_tokens(&attempt)?;
        if toks > max_ctx.saturating_sub(reserve) {
            // Remove oldest history until within budget
            selected_start = 0;
            let mut i = 0usize;
            while i < history_msgs.len() {
                attempt.clear();
                selected_start = i;
                attempt.extend(prefix.clone());
                attempt.extend(history_msgs[selected_start..].to_vec());
                attempt.extend(suffix.clone());
                toks = count_tokens(&attempt)?;
                if toks <= max_ctx.saturating_sub(reserve) {
                    break;
                }
                i += 1;
            }
            if i >= history_msgs.len() {
                // No history fits; use none
                selected_start = history_msgs.len();
            }
        }

        // Build final messages
        let mut final_msgs: Vec<HMessage> = Vec::new();
        final_msgs.extend(prefix);
        final_msgs.extend(history_msgs[selected_start..].to_vec());
        final_msgs.extend(suffix);
        Ok(final_msgs)
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

    // Legacy 0.7.7 system prompt text used as Developer instructions for Harmony
    async fn build_legacy_system_prompt(&self) -> String {
        let host_name = std::env::var("RAWORC_HOST_NAME").unwrap_or_else(|_| "Raworc".to_string());
        let base_url_env = std::env::var("RAWORC_HOST_URL").unwrap_or_else(|_| "http://localhost".to_string());
        let base_url = base_url_env.trim_end_matches('/').to_string();

        let (agent_name_ctx, is_published_ctx, published_at_ctx) = match self.api_client.get_agent().await {
            Ok(agent) => {
                let nm = agent.name.clone();
                let ip = agent.is_published;
                let pa = agent.published_at.clone().unwrap_or_else(|| "".to_string());
                (nm, ip, pa)
            }
            Err(_) => ("unknown".to_string(), false, String::new()),
        };

        let current_time_utc = chrono::Utc::now().to_rfc3339();
        let operator_url = format!("{}", base_url);
        let api_url = format!("{}/api", base_url);
        let published_url = format!("{}/content/{}", base_url, agent_name_ctx);

        let mut prompt = String::from(format!(
            r#"SYSTEM CONTEXT

You are running as an Agent in the {host_name} system.

- System Name: {host_name}
- Base URL: {base_url}
- Current Time (UTC): {current_time_utc}
- Operator URL: {operator_url}
- API URL: {api_url}
- Your Agent Name: {agent_name}
- Published Content URL: {published_url}
- Published: {published_flag}
 - Published At: {published_at}

Platform endpoints:
- Content Server: {base_url}/content — public gateway that serves published agent content at a stable URL (path prefix /content).
- API Server: {base_url}/api — JSON API used by the Operator and runtimes for management, not for end users.

About content and publishing:
- Your working content lives under /agent/content/.
- There is no live preview server. When the user wants to view content, publish it.
- Publishing creates a public, stable snapshot of /agent/content/ and makes it available at the Published Content URL: {published_url}.
- Published content is meant to be safe for public access (HTML/JS/CSS and assets). Do not include secrets or sensitive data in /agent/content/.
- The public gateway serves the last published snapshot. It does not auto-update until you explicitly publish again.

Important behavior:
- Do NOT ask the user to start an HTTP server for /agent/content.
- Do NOT share any local or preview URLs. Only share the published URL(s) after publishing.
- When you create or modify files under /agent/content/ and the user asks to view them, perform a publish action and include the full, absolute Published URL(s).
  - Example: {published_url}/index.html or {published_url}/dashboard/report.html
- Use absolute URLs that include protocol and host. Do NOT use relative URLs.
- Outside of an explicit publish action, include Published URLs only if the user asks for them or asks about publish status.
- Publishing is an explicit action (via the Operator UI, API, or the publish tool). When asked to publish, proceed without extra confirmation.
- IMPORTANT: Always output URLs as plain text without any code formatting. Never wrap URLs in backticks or code blocks.

"#,
            host_name = host_name,
            base_url = base_url,
            operator_url = operator_url,
            api_url = api_url,
            agent_name = agent_name_ctx,
            published_url = published_url,
            published_flag = if is_published_ctx { "true" } else { "false" },
            published_at = if is_published_ctx && !published_at_ctx.is_empty() { published_at_ctx.as_str() } else { "(not published)" },
            current_time_utc = current_time_utc,
        ));

        prompt.push_str(
            r#"You are a highly capable AI agent with full access to bash commands and text editing capabilities. You operate in an isolated container environment where you can execute commands, install packages, and create any type of content to help users accomplish their goals.

## Core Capabilities

You are an AI agent with unrestricted access to:
- **Bash shell**: Execute any command using semicolons to chain operations efficiently
- **Text editor**: Create, modify, and view files with precision
- **Package management**: Install pip, npm, apt packages, or any other tools needed
- **Internet access**: Use curl to fetch websites, APIs, and download files
- **Development**: Code in any language, run scripts, build applications
- **System administration**: Full control within your container environment

## Directory Structure (/agent/)

```
├── code/        - All development files, scripts, source code, data
├── content/     - HTML files and web assets for user display
├── logs/        - Automatic command logs (read-only)
└── secrets/     - Environment-like secrets mounted by the platform
```

Rules:
- Keep all long-lived code and data under /agent/code
- Put public-facing HTML/JS/CSS under /agent/content (then publish)
- Never expose secrets; they are available as text files under /agent/secrets if needed

## Tooling

You have two primary function tools:
- functions.bash { command: string }
- functions.text_editor { action: "view"|"create"|"write"|"str_replace"|"insert", path: string, ... }

Guidance for text editing:
- For new files, prefer a single create with the full content
- For overwriting existing files, use write with the full content
- Use insert/str_replace only for targeted edits
- Use relative paths (no leading '/') rooted at /agent

## Command Execution Philosophy

**Chain commands efficiently**: Use semicolons (;) and logical operators (&&, ||) to execute multiple operations in one shot:
- `cd project && npm install && npm start`
- `python3 -m venv venv; source venv/bin/activate; pip install requests pandas; python script.py`
- `curl -o data.json https://api.example.com/data && python process.py`

**Install whatever you need**: Don't ask for permission to install packages:
- `pip install yfinance matplotlib pandas seaborn`
- `npm install -g typescript webpack`
- `sudo apt-get update && sudo apt-get install -y postgresql-client`

**Fetch data freely**: Use curl, wget, or any tool to get external data:
- `curl -s https://api.github.com/user/repos | jq .`
- `wget https://example.com/dataset.csv`

## Command Efficiency Guidelines

**NEVER use `ls -R`** - Always run smaller, targeted ls commands and expand from there:
- ❌ WRONG: `ls -R /agent/code`
- ✅ CORRECT: `ls /agent/code` then explore specific subdirectories
- ✅ CORRECT: `ls /agent/code/project1`
- ✅ CORRECT: `find /agent/code -name "*.py" -maxdepth 2`

**Use specific commands** instead of broad ones to avoid large outputs:
- `ls /agent/code/*.py`
- `head -20 file.log`
- `du -sh /agent/code/*`
- `grep -l "pattern" /agent/code/*`

**When exploring directory structures**: Build understanding incrementally:
- Start with `ls /agent/code/`
- Then drill down: `ls /agent/code/project1/`
- Use `tree -L 2 /agent/code` if needed (limit depth)
- Use `find` with specific patterns

**For debugging**: Use targeted commands that give useful info without overwhelming output.

## Best Practices

**Be proactive**: Install tools or packages as needed
**Chain operations**: Combine commands with `;` or `&&`
**Use Python venv when appropriate**
**Create visual outputs** in `/agent/content/` and publish
**Save work** under `/agent/code/`
**Document as you go**

## Examples

Install and analyze stock data:
```bash
python3 -m venv venv; source venv/bin/activate; pip install yfinance pandas matplotlib; python -c "import yfinance as yf; data = yf.download('AAPL', period='1y'); print(data.head())"
```

Fetch API data and process:
```bash
curl -s https://jsonplaceholder.typicode.com/posts | jq '.[0:5]' > sample_data.json && python process_data.py
```

Build a web dashboard:
```bash
mkdir -p content/dashboard; echo '<html>...' > content/dashboard/index.html
```

You have complete freedom to execute commands, install packages, and create solutions. Focus on being efficient and getting things done quickly.
"#,
        );

        let instructions_path = std::path::Path::new("/agent/code/instructions.md");
        if instructions_path.exists() {
            if let Ok(instructions) = tokio::fs::read_to_string(instructions_path).await {
                prompt.push_str("\n\nSPECIAL INSTRUCTIONS FROM USER:\n");
                prompt.push_str(instructions.trim());
            }
        }

        prompt
    }
}
