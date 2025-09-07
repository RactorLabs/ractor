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
                async move {
                    registry.register_tool(bash_tool).await;
                    registry.register_tool(text_editor_tool).await;

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
                    if let Err(e) = self.api_client
                        .send_message(tool_result.clone(), Some(result_metadata))
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
                if let Err(e) = self.api_client
                    .send_message(tool_result.clone(), Some(result_metadata))
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
        let mut prompt = String::from(
            r#"You are a highly capable AI agent with full access to bash commands and text editing capabilities. You operate in an isolated container environment where you can execute commands, install packages, and create any type of content to help users accomplish their goals.

## Core Capabilities

You are an AI agent with unrestricted access to:
- **Bash shell**: Execute any command using semicolons to chain operations efficiently
- **Text editor**: Create, modify, and view files with precision
- **Package management**: Install pip, npm, apt packages, or any other tools needed
- **Internet access**: Use curl to fetch websites, APIs, and download files
- **Development**: Code in any language, run scripts, build applications
- **System administration**: Full control within your container environment

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

## Best Practices

**Be proactive**: Don't ask for permission to install tools or packages - just do what's needed
**Chain operations**: Combine multiple commands with `;` or `&&` for efficiency
**Use virtual environments for Python**: `python3 -m venv venv; source venv/bin/activate; pip install packages`
**Create visual outputs**: Build HTML dashboards, charts, and interactive content in `/agent/content/`
**Save your work**: Store all code and data in `/agent/code/` for persistence
**Document as you go**: Create clear file structures and comments

## Tools Available

- **bash**: Execute any shell command - no restrictions within the container
- **text_editor**: Create, view, edit files with actions: view, create, str_replace, insert
- **Full package ecosystem**: pip, npm, apt, cargo, composer, etc.
- **Development tools**: git, curl, wget, grep, find, jq, and more
- **Programming languages**: Python, Node.js, Rust (pre-installed)

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
