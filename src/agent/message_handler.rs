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
        let mut step: u32 = 0;
        loop {
            step += 1;
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
            // Full Harmony logs are enabled by default now.
            // Disable by setting RAWORC_LOG_HARMONY to a falsey value (0,false,no,off),
            // or by writing such a value into /agent/secrets/log_harmony (or /agent/logs/log_harmony).
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

            let completion = self
                .gpt_client
                .generate(
                    &prompt,
                    Some(gen_params.clone()),
                )
                .await
                .map_err(|e| super::error::HostError::Model(format!("GPT server failed: {}", e)))?;

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
            let parsed = enc
                .parse_messages_from_completion_tokens(completion_tokens, Some(HRole::Assistant))
                .map_err(|e| super::error::HostError::Model(format!("Harmony parse failed: {}", e)))?;
            tracing::debug!(target: "gpt", request_id = %request_id, parsed_count = parsed.len(), "HARMONY_PARSE_OK");

            // Aggregate Harmony segments in original order for a single composite DB row
            let mut segments: Vec<serde_json::Value> = Vec::new();
            let mut final_msg: Option<(String, String)> = None; // (channel, text)

            for m in parsed.iter() {
                if m.author.role != HRole::Assistant { continue; }

                // Tool call encountered
                if let Some(recipient) = m.recipient.as_deref() {
                    if let Some(tool_name) = recipient.strip_prefix("functions.") {
                        let args_text = Self::first_text(&m.content).unwrap_or("");
                        let args_json: serde_json::Value = serde_json::from_str(args_text).unwrap_or_else(|_| serde_json::json!({}));

                        // Add tool_call segment in-order
                        segments.push(serde_json::json!({
                            "type": "tool_call",
                            "tool": tool_name,
                            "args": args_json.clone(),
                        }));

                        // Execute tool immediately and add tool_result segment
                        let result = match self.tool_registry.execute_tool(tool_name, &args_json).await {
                            Ok(r) => r,
                            Err(e) => format!("[error] {}", e),
                        };

                        segments.push(serde_json::json!({
                            "type": "tool_result",
                            "tool": tool_name,
                            "args": args_json.clone(),
                            "output": result,
                        }));

                        // For idempotent management tools, finalize the turn after success
                        if final_msg.is_none() {
                            if tool_name == "publish" {
                                let text = "Publish completed.".to_string();
                                final_msg = Some(("final".to_string(), text));
                            } else if tool_name == "sleep" {
                                let text = "Agent is going to sleep.".to_string();
                                final_msg = Some(("final".to_string(), text));
                            }
                        }

                        // Feed Harmony for the next step
                        tool_messages.push(HMessage {
                            author: HAuthor::new(HRole::Tool, tool_name.to_string()),
                            recipient: None,
                            content: vec![HContent::from(result)],
                            channel: None,
                            content_type: None,
                        });
                        continue;
                    }
                }

                // Commentary/analysis channel
                if let Some(ch) = m.channel.as_deref() {
                    if ch == "analysis" || ch == "commentary" {
                        if let Some(text) = Self::first_text(&m.content) {
                            let sanitized = self.guardrails.validate_output(text)?;
                            segments.push(serde_json::json!({
                                "type": "commentary",
                                "channel": ch,
                                "text": sanitized,
                            }));
                            continue;
                        }
                    }
                }

                // Potential final text (first non-tool/non-commentary assistant text)
                if final_msg.is_none() {
                    if let Some(text) = Self::first_text(&m.content) {
                        let ch = m.channel.clone().unwrap_or_else(|| "final".to_string());
                        final_msg = Some((ch, text.to_string()));
                    }
                }
            }

            // Tools and commentary were already aggregated in-order during parsing loop

            // 3) Post this loop step as its own message (per-step composite)
            // Add final segment if present so the step captures it
            let mut content_str = String::new();
            let mut channel_for_row: Option<String> = None;
            if let Some((ch, text)) = final_msg {
                let sanitized = self.guardrails.validate_output(&text)?;
                segments.push(serde_json::json!({
                    "type": "final",
                    "channel": ch,
                    "text": sanitized,
                }));
                content_str = sanitized;
                channel_for_row = Some(ch);
            }

            if !segments.is_empty() {
                let meta = serde_json::json!({
                    "type": "model_response_step",
                    "model": "gpt-oss",
                    "step": step,
                    "has_final": channel_for_row.is_some(),
                });
                let content_json = serde_json::json!({
                    "harmony": {
                        "request_id": request_id,
                        "segments": segments,
                    }
                });
                self
                    .api_client
                    .send_message_structured(
                        super::api::MessageRole::Agent,
                        content_str,
                        Some(meta),
                        None,
                        None,
                        channel_for_row.clone(),
                        None,
                        Some(content_json),
                    )
                    .await?;
            }

            // Stop if final was present in this step
            if channel_for_row.is_some() {
                return Ok(());
            }

            // If we executed tools but no final yet, continue the loop for next step
            if !tool_messages.is_empty() {
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
- Use functions.text_editor for file edits (actions: view/create/str_replace/insert).

File editing guidance:
- When creating a new file, prefer a single create call with the full content in one go.
- Avoid issuing many incremental insert/replace calls to build a new file; write the complete HTML content in the create call.
- Use relative paths only (no leading '/'); paths are rooted at /agent.
- Only use insert/str_replace for targeted updates to existing files.
- Do not call view before create when creating a brand new file unless verifying it already exists.

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
        for m in history.iter().filter(|m| m.id != current.id) {
            match m.role {
                MessageRole::User => msgs.push(HMessage::from_role_and_content(HRole::User, m.content.clone())),
                MessageRole::Agent => {
                    // Prefer treating prior agent messages as final-channel assistant messages
                    // This avoids leaking prior analysis/commentary and keeps history concise.
                    msgs.push(HMessage {
                        author: HAuthor::new(HRole::Assistant, "assistant".to_string()),
                        recipient: None,
                        content: vec![HContent::from(m.content.clone())],
                        channel: Some("final".to_string()),
                        content_type: None,
                    });
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
