use super::api::{Message, MessageRole, RaworcClient, MESSAGE_ROLE_USER};
use super::builtin_tools::{BashTool, TextEditorTool};
use super::error::Result;
use super::guardrails::Guardrails;
use super::ollama::{ChatMessage, ModelResponse, OllamaClient};
use super::tool_registry::{ContainerExecMapper, ToolRegistry};
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
    tool_registry: Arc<ToolRegistry>,
}

impl MessageHandler {
    /// Map platform roles to LLM roles.
    fn role_to_model(role: &MessageRole) -> &'static str {
        match role {
            MessageRole::User => MESSAGE_ROLE_USER,
            MessageRole::Agent => "assistant",
            MessageRole::System => "system",
        }
    }
    pub fn new(
        api_client: Arc<RaworcClient>,
        ollama_client: Arc<OllamaClient>,
        guardrails: Arc<Guardrails>,
    ) -> Self {
        Self::new_with_registry(api_client, ollama_client, guardrails, None)
    }

    pub fn new_with_registry(
        api_client: Arc<RaworcClient>,
        ollama_client: Arc<OllamaClient>,
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
                    let publish_tool = Box::new(super::builtin_tools::PublishTool::new(api_client_clone.clone()));
                    let sleep_tool = Box::new(super::builtin_tools::SleepTool::new(api_client_clone.clone()));
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

                    // Register harmony format aliases
                    registry
                        .register_alias("functions", "bash", None)
                        .await;
                    
                    // Also map text_editor harmony calls
                    registry
                        .register_alias("text_editor", "text_editor", None)
                        .await;

                    info!("Registered built-in tools and aliases");
                }
            });

            registry
        };

        Self {
            api_client,
            ollama_client,
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
                "bash" => input
                    .get("command")
                    .and_then(|v| v.as_str())
                    .or_else(|| input.get("cmd").and_then(|v| v.as_str()))
                    .unwrap_or("bash command")
                    .to_string(),
                "text_editor" => {
                    let action = input
                        .get("action")
                        .and_then(|v| v.as_str())
                        .unwrap_or("edit");
                    let path = input
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    format!("{} {}", action, path)
                }
                _ => format!("Executing {} tool", tool),
            };
            self.send_tool_message(&tool_description, &tool, Some(&input))
                .await?;

            // Execute tool through registry
            let tool_result = match self.tool_registry.execute_tool(&tool, &input).await {
                Ok(result) => result,
                Err(e) => format!("[error] {}", e),
            };

            // Send result back
            let mut metadata = serde_json::json!({ "type": "tool_result", "tool_type": tool });
            if let Some(obj) = metadata.as_object_mut() {
                obj.insert("args".to_string(), input.clone());
            }
            self.api_client
                .send_message(tool_result, Some(metadata))
                .await?;
            return Ok(());
        }

        // Native Ollama tool calling with GPT-OSS models
        // The model will use structured tool calls when tools are needed

        // Fetch full conversation
        let all_messages = self.fetch_all_agent_messages().await?;
        let mut conversation = self.prepare_conversation_history(&all_messages, message);

        // Build system prompt
        let system_prompt = self.build_system_prompt().await;

        // Loop for tool usage with no hard cap on steps
        loop {
            // Track model "thinking" duration for UI (best-effort)
            let mut thinking_secs: Option<f32> = None;
            // Simple retry/backoff for transient failures
            let mut resp: Result<ModelResponse> =
                Err(super::error::HostError::Model("uninitialized".to_string()));
            for attempt in 0..=2 {
                let started = std::time::Instant::now();
                let try_resp = self
                    .ollama_client
                    .complete_with_registry(
                        conversation.clone(),
                        Some(system_prompt.clone()),
                        Some(&*self.tool_registry),
                    )
                    .await;
                match try_resp {
                    Ok(t) => {
                        // Record approximate thinking time for this successful attempt
                        thinking_secs = Some(started.elapsed().as_secs_f32());
                        resp = Ok(t);
                        break;
                    }
                    Err(e) => {
                        if attempt < 2 {
                            let delay = 300u64 * 3u64.pow(attempt);
                            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                            resp = Err(e);
                            continue;
                        } else {
                            resp = Err(e);
                        }
                    }
                }
            }

            let model_resp = match resp {
                Ok(t) => t,
                Err(e) => {
                    warn!("Ollama API failed: {}", e);
                    self.finalize_with_fallback(&message.content).await?;
                    return Ok(());
                }
            };

            // Check if response contains structured tool calls (GPT-OSS format)
            if let Some(tool_calls) = &model_resp.tool_calls {
                if let Some(tool_call) = tool_calls.first() {
                    let tool_name = &tool_call.function.name;
                    let args = &tool_call.function.arguments;

                    // Log parsed tool call
                    info!("Structured tool call: {} with args: {:?}", tool_name, args);

                    // Send tool execution notification to user
                    let tool_description = match tool_name.as_str() {
                        "bash" => {
                            if let Some(cmd) = args
                                .get("command")
                                .and_then(|v| v.as_str())
                                .or_else(|| args.get("cmd").and_then(|v| v.as_str()))
                            {
                                cmd.to_string()
                            } else {
                                "bash command".to_string()
                            }
                        }
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
                        _ => format!("Executing {} tool", tool_name),
                    };

                    self.send_tool_message(&tool_description, tool_name, Some(args))
                        .await?;

                    // Execute tool through registry
                    let tool_result = match self.tool_registry.execute_tool(tool_name, args).await {
                        Ok(result) => result,
                        Err(e) => format!("[error] {}", e),
                    };

                    // Log tool result
                    info!("Tool result: {} ({} bytes)", tool_name, tool_result.len());

                    // Send tool result message to database for UI display
                    let mut result_metadata = serde_json::json!({
                        "type": "tool_result",
                        "tool_type": tool_name
                    });
                    if let Some(obj) = result_metadata.as_object_mut() {
                        obj.insert("args".to_string(), args.clone());
                    }
                    // Truncate large tool results for message display
                    let display_result = self.truncate_tool_result(&tool_result);
                    if let Err(e) = self.api_client
                        .send_message(display_result, Some(result_metadata))
                        .await
                    {
                        warn!("Failed to send tool result message: {}", e);
                    }

                    // Add tool result to conversation following Ollama cookbook
                    conversation.push(ChatMessage {
                        role: "tool".to_string(),
                        content: tool_result,
                        name: Some(tool_name.clone()),
                        tool_call_id: None,
                    });

                    continue;
                }
            } else if let Some((tool_name, args)) =
                parse_assistant_functions_text(&model_resp.content)
            {
                // Fallback: parse assistant<|channel|>functions.* style
                info!(
                    "Assistant functions text tool call: {} with args: {:?}",
                    tool_name, args
                );

                // Notify user
                let tool_description = match tool_name.as_str() {
                    "bash" => {
                        if let Some(cmd) = args
                            .get("command")
                            .and_then(|v| v.as_str())
                            .or_else(|| args.get("cmd").and_then(|v| v.as_str()))
                        {
                            cmd.to_string()
                        } else {
                            "bash command".to_string()
                        }
                    }
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
                    _ => format!("Executing {} tool", tool_name),
                };
                self.send_tool_message(&tool_description, &tool_name, Some(&args))
                    .await?;

                // Execute tool through registry
                let tool_result = match self.tool_registry.execute_tool(&tool_name, &args).await {
                    Ok(result) => result,
                    Err(e) => format!("[error] {}", e),
                };

                // Send tool result message to database for UI display
                let mut result_metadata = serde_json::json!({
                    "type": "tool_result",
                    "tool_type": &tool_name
                });
                if let Some(obj) = result_metadata.as_object_mut() {
                    obj.insert("args".to_string(), args.clone());
                }
                // Truncate large tool results for message display
                let display_result = self.truncate_tool_result(&tool_result);
                if let Err(e) = self.api_client
                    .send_message(display_result, Some(result_metadata))
                    .await
                {
                    warn!("Failed to send tool result message: {}", e);
                }

                // Add tool result to conversation and continue
                conversation.push(ChatMessage {
                    role: "tool".to_string(),
                    content: tool_result,
                    name: Some(tool_name.clone()),
                    tool_call_id: None,
                });
                continue;
            } else {
                // Treat as final answer
                let sanitized = self.guardrails.validate_output(&model_resp.content)?;
                let mut meta = serde_json::json!({
                    "type": "model_response",
                    "model": "gpt-oss"
                });
                if let Some(obj) = meta.as_object_mut() {
                    if let Some(thinking) = &model_resp.thinking {
                        obj.insert(
                            "thinking".to_string(),
                            serde_json::Value::String(thinking.clone()),
                        );
                    }
                    if let Some(secs) = thinking_secs {
                        obj.insert("thinking_seconds".to_string(), serde_json::json!(secs));
                    }
                }
                self.api_client.send_message(sanitized, Some(meta)).await?;
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
        current: &Message,
    ) -> Vec<ChatMessage> {
        let mut conversation: Vec<ChatMessage> = Vec::new();

        // Include ALL message history (excluding the current message being processed)
        let history: Vec<_> = messages
            .iter()
            .filter(|m| m.id != current.id)
            .filter(|m| matches!(m.role, MessageRole::User | MessageRole::Agent | MessageRole::System))
            .map(|m| {
                // Pass explicit tool results as tool role with name for the model
                if let Some(metadata) = &m.metadata {
                    if let Some(msg_type) = metadata.get("type").and_then(|v| v.as_str()) {
                        if msg_type == "tool_result" {
                            let tool_name = metadata
                                .get("tool_type")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            return ChatMessage { role: "tool".to_string(), content: m.content.clone(), name: tool_name, tool_call_id: None };
                        }
                    }
                }
                let role = Self::role_to_model(&m.role).to_string();
                ChatMessage { role, content: m.content.clone(), name: None, tool_call_id: None }
            })
            .collect();

        conversation.extend(history);

        // Always add the current message (avoids race if not yet visible in fetched list)
        conversation.push(ChatMessage {
            role: MESSAGE_ROLE_USER.to_string(),
            content: current.content.clone(),
            name: None,
            tool_call_id: None,
        });

        info!(
            "Prepared conversation with {} messages of history",
            conversation.len() - 1
        );
        conversation
    }

    async fn build_system_prompt(&self) -> String {
        // Read hosting context from environment with defaults
        let host_name = std::env::var("RAWORC_HOST_NAME").unwrap_or_else(|_| "Raworc".to_string());
        let base_url_env = std::env::var("RAWORC_HOST_URL").unwrap_or_else(|_| "http://localhost".to_string());
        let base_url = base_url_env.trim_end_matches('/').to_string();

        // Fetch agent info from API/DB (name, content_port, publish state)
        let (agent_name_ctx, content_port_ctx, is_published_ctx, published_at_ctx) =
            match self.api_client.get_agent().await {
                Ok(agent) => {
                    let nm = agent.name.clone();
                    let cp = agent.content_port;
                    let ip = agent.is_published;
                    let pa = agent.published_at.clone().unwrap_or_else(|| "".to_string());
                    (nm, cp, ip, pa)
                }
                Err(_) => ("unknown".to_string(), None, false, String::new()),
            };

        // Current timestamp in UTC for context
        let current_time_utc = chrono::Utc::now().to_rfc3339();

        let operator_url = format!("{}", base_url);
        let api_url = format!("{}/api", base_url);
        let live_url = content_port_ctx
            .map(|p| format!("{}:{}/", base_url, p))
            .unwrap_or_else(|| "(content port not assigned)".to_string());
        let published_url = format!("{}/content/{}", base_url, agent_name_ctx);

        // Start with System Context specific to Raworc runtime
        let mut prompt = String::from(format!(
            r#"SYSTEM CONTEXT

You are running as an Agent in the {host_name} system.

- System Name: {host_name}
- Base URL: {base_url}
- Current Time (UTC): {current_time_utc}
- Operator URL: {operator_url}
- API URL: {api_url}
- Your Agent Name: {agent_name}
- Your Content Port: {content_port}
- Live Content URL: {live_url}
 - Published Content URL: {published_url}
- Published: {published_flag}
 - Published At: {published_at}

Platform endpoints:
- Content Server: {base_url}/content — public gateway that serves published agent content at a stable URL.
- API Server: {base_url}/api — JSON API used by the Operator and runtimes for management, not for end users.

About content and publishing:
- Your live content is everything under /agent/content/. It is served immediately on the Live Content URL while you work.
- Publishing creates a public, stable snapshot of your current /agent/content/ and makes it available at the Published Content URL: {published_url}.
- Published content is meant to be safe for public access (HTML/JS/CSS and assets). Do not include secrets or sensitive data in /agent/content/.
- The Content Server serves the last published snapshot. It does not auto-update until you explicitly publish again.

Important behavior:
- Do NOT ask the user to start an HTTP server for /agent/content.
- Your live content is automatically served at the Live Content URL.
- When you create or modify files under /agent/content/, always include the full, absolute Live URL to the exact file(s) you touched.
  - Example: {live_url}index.html or {live_url}dashboard/report.html
- When you perform a publish action, always include the full, absolute Published URL to the exact file(s) (and the root if helpful).
  - Example: {published_url}/index.html or {published_url}/dashboard/report.html
- Use absolute URLs that include protocol and host. Do NOT use relative URLs.
- Outside of an explicit publish action, only include Published URLs if the user asks for them or asks about publish status.
- If the user wants the current live content to be available at the published URL, perform an explicit publish action (do not auto-publish without being asked).
- Publishing is an explicit action (via the Operator UI, API, or the publish tool). When asked to publish, proceed without extra confirmation.
- IMPORTANT: Always output URLs as plain text without any code formatting. Never wrap URLs in backticks or code blocks.

"#,
            host_name = host_name,
            base_url = base_url,
            operator_url = operator_url,
            api_url = api_url,
            agent_name = agent_name_ctx,
            content_port = content_port_ctx
                .map(|p| p.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            live_url = live_url,
            published_url = published_url,
            published_flag = if is_published_ctx { "true" } else { "false" },
            published_at = if is_published_ctx && !published_at_ctx.is_empty() { published_at_ctx.as_str() } else { "(not published)" },
            current_time_utc = current_time_utc,
        ));

        // Continue with the general capabilities prompt
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
└── secrets/     - Environment variables (auto-managed)
```

**Working files**: Use `/agent/code/` for everything - scripts, data files, projects, executables
**User displays**: Use `/agent/content/` for HTML, visualizations, reports, dashboards
**Special files**:
- `/agent/code/instructions.md` - Persistent instructions (auto-loaded)
- `/agent/code/setup.sh` - Initialization script (auto-executed)

## Tools Available

- **bash**: Execute any shell command - no restrictions within the container
- **text_editor**: Create, view, edit files with actions: view, create, str_replace, insert
- **Full package ecosystem**: pip, npm, apt, cargo, composer, etc.
- **Development tools**: git, curl, wget, grep, find, jq, and more
- **Programming languages**: Python, Node.js, Rust (pre-installed)

## CRITICAL: Always Use Your Tools

**When you need to run commands**: ALWAYS use the bash tool - don't just think about it, execute it
**When you need to edit files**: ALWAYS use the text_editor tool - don't just plan, do it  
**When you see errors or need to fix something**: IMMEDIATELY use tools to take corrective action
**When you want to check something**: USE bash tool to verify, don't assume

NEVER just think or plan without taking action. If you identify something that needs to be done, DO IT with your tools immediately.

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
- ❌ WRONG: `ls -R /agent/code` (can produce overwhelming output)
- ✅ CORRECT: `ls /agent/code` then explore specific subdirectories
- ✅ CORRECT: `ls /agent/code/project1` to examine a specific directory
- ✅ CORRECT: `find /agent/code -name "*.py" -maxdepth 2` for targeted file discovery

**Use specific commands** instead of broad ones to avoid large outputs:
- `ls /agent/code/*.py` to see only Python files
- `head -20 file.log` instead of `cat file.log` for large files
- `du -sh /agent/code/*` instead of recursive listings for directory sizes
- `grep -l "pattern" /agent/code/*` to find files containing patterns

**When exploring directory structures**: Build understanding incrementally:
- Start with `ls /agent/code/` to see top-level structure
- Then drill down: `ls /agent/code/project1/` for specific areas of interest
- Use `tree -L 2 /agent/code` if you need a recursive view (limit depth)
- Use `find` with specific patterns: `find /agent/code -type f -name "*.py"`

**For debugging**: Use targeted commands that give useful info without overwhelming output.

## Best Practices

**Be proactive**: Don't ask for permission to install tools or packages - just do what's needed
**Chain operations**: Combine multiple commands with `;` or `&&` for efficiency
**Use virtual environments for Python**: `python3 -m venv venv; source venv/bin/activate; pip install packages`
**Create visual outputs**: Build HTML dashboards, charts, and interactive content in `/agent/content/`
**Save your work**: Store all code and data in `/agent/code/` for persistence
**Document as you go**: Create clear file structures and comments

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
        let preview = tool_result.chars().take(TRUNCATION_PREVIEW).collect::<String>();
        
        format!(
            "{}\n\n[OUTPUT TRUNCATED - {} total lines, {} total characters]\n[Use more specific commands like 'ls /agent/code' (no -R) to avoid large outputs]\n[Full output available in agent logs]",
            preview,
            line_count,
            char_count
        )
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

fn parse_structured_tool_calls(_s: &str) -> Option<Vec<StructuredToolCall>> {
    None
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
    if end <= start {
        return None;
    }
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
    if tool != "bash" && tool != "text_editor" {
        return None;
    }
    let input = v.get("input")?.clone();
    Some((tool, input))
}
