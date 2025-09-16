use super::error::{HostError, Result};
use super::tool_registry::ToolRegistry;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct OllamaClient {
    client: Client,
    base_url: String,
    reasoning_effort: Option<String>,
    thinking_budget: Option<u32>,
}

#[derive(Debug, Serialize, Clone)]
struct ChatRequestMessage<'a> {
    role: &'a str,
    content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    Function,
}

#[derive(Debug, Serialize, Clone)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Clone)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub typ: ToolType,
    pub function: ToolFunction,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatRequestMessage<'a>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<Reasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<Thinking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<RequestOptions>,
}

#[derive(Debug, Serialize)]
struct Reasoning {
    effort: String, // e.g., "low" | "medium" | "high"
}

#[derive(Debug, Serialize)]
struct Thinking {
    #[serde(rename = "type")]
    typ: String, // e.g., "enabled"
    #[serde(skip_serializing_if = "Option::is_none")]
    budget_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct RequestOptions {
    num_ctx: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ToolCall {
    #[serde(default = "generate_tool_call_id")]
    pub id: String,
    pub function: ToolCallFunction,
}

fn generate_tool_call_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    format!("call_{}", COUNTER.fetch_add(1, Ordering::SeqCst))
}

#[derive(Debug, Deserialize, Clone)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    #[serde(default)]
    role: String,
    content: String,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: ChatResponseMessage,
    #[serde(flatten)]
    extra: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ModelResponse {
    pub content: String,
    pub thinking: Option<String>,
    // Text from the harmony "commentary" channel, when present
    pub commentary: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub error: Option<String>,
}

impl ToolResult {
    pub fn from_tool_results(tool_results: Vec<ToolResult>) -> Vec<ChatMessage> {
        tool_results.into_iter().map(|tr| tr.to_chat_message()).collect()
    }
    pub fn new(tool_call_id: String, content: String) -> Self {
        Self {
            tool_call_id,
            content,
            error: None,
        }
    }

    pub fn with_error(tool_call_id: String, error: String) -> Self {
        Self {
            tool_call_id,
            content: String::new(),
            error: Some(error),
        }
    }

    pub fn to_chat_message(&self) -> ChatMessage {
        let content = if let Some(ref error) = self.error {
            format!("Error: {}", error)
        } else {
            self.content.clone()
        };

        ChatMessage {
            role: "tool".to_string(),
            content,
            name: None,
            tool_call_id: Some(self.tool_call_id.clone()),
        }
    }
}

impl OllamaClient {
    pub fn new(base_url: &str) -> Result<Self> {
        let timeout_secs = match std::env::var("OLLAMA_TIMEOUT_SECS").ok() {
            Some(timeout_str) => match timeout_str.parse::<u64>() {
                Ok(timeout) => {
                    if timeout < 10 || timeout > 7200 {
                        return Err(HostError::Model(format!(
                            "Invalid OLLAMA_TIMEOUT_SECS '{}'. Must be between 10 and 7200 seconds",
                            timeout
                        )));
                    }
                    timeout
                }
                Err(_) => {
                    return Err(HostError::Model(format!(
                        "Invalid OLLAMA_TIMEOUT_SECS '{}'. Must be a valid positive integer",
                        timeout_str
                    )));
                }
            },
            None => 1800, // 30 minutes default for large context inference
        };

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| HostError::Model(format!("Failed to create client: {}", e)))?;

        // Reasoning controls via env with sensible defaults
        // Default to high reasoning effort and a thinking budget if not provided
        let reasoning_effort = match std::env::var("OLLAMA_REASONING_EFFORT").ok() {
            Some(effort) => {
                let effort_lower = effort.to_lowercase();
                if !["low", "medium", "high"].contains(&effort_lower.as_str()) {
                    return Err(HostError::Model(format!(
                        "Invalid OLLAMA_REASONING_EFFORT '{}'. Must be one of: low, medium, high",
                        effort
                    )));
                }
                Some(effort_lower)
            }
            None => Some("high".to_string()),
        };
        let thinking_budget = match std::env::var("OLLAMA_THINKING_TOKENS").ok() {
            Some(budget_str) => match budget_str.parse::<u32>() {
                Ok(budget) => {
                    if budget < 100 || budget > 100000 {
                        return Err(HostError::Model(format!(
                            "Invalid OLLAMA_THINKING_TOKENS '{}'. Must be between 100 and 100000",
                            budget
                        )));
                    }
                    Some(budget)
                }
                Err(_) => {
                    return Err(HostError::Model(format!(
                        "Invalid OLLAMA_THINKING_TOKENS '{}'. Must be a valid positive integer",
                        budget_str
                    )));
                }
            },
            None => Some(4096),
        };

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            reasoning_effort,
            thinking_budget,
        })
    }

    pub async fn complete(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<String>,
    ) -> Result<ModelResponse> {
        self.complete_with_registry(messages, system_prompt, None)
            .await
    }

    pub async fn complete_with_registry(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<String>,
        tool_registry: Option<&ToolRegistry>,
    ) -> Result<ModelResponse> {
        self.complete_with_tool_execution(messages, system_prompt, tool_registry, false).await
    }

    pub async fn complete_with_tool_execution(
        &self,
        mut messages: Vec<ChatMessage>,
        system_prompt: Option<String>,
        tool_registry: Option<&ToolRegistry>,
        enable_tool_execution: bool,
    ) -> Result<ModelResponse> {
        const MAX_ITERATIONS: usize = 10; // Prevent infinite loops
        let mut iteration = 0;

        loop {
            iteration += 1;
            if iteration > MAX_ITERATIONS {
                return Err(HostError::Model(
                    "Tool execution exceeded maximum iterations".to_string(),
                ));
            }

            let response = self.complete_single_turn(messages.clone(), system_prompt.clone(), tool_registry).await?;
            
            // If no tool calls or tool execution disabled, return response
            if !enable_tool_execution || response.tool_calls.is_none() {
                return Ok(response);
            }

            let tool_calls = response.tool_calls.as_ref().unwrap();
            tracing::info!("Processing {} tool calls in iteration {}", tool_calls.len(), iteration);

            // Add the assistant's message with tool calls to conversation
            messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: response.content.clone(),
                name: None,
                tool_call_id: None,
            });

            // Execute tool calls if registry is available
            if let Some(registry) = tool_registry {
                let mut tool_results = Vec::new();
                
                for tool_call in tool_calls {
                    let result = self.execute_tool_call(tool_call, registry).await;
                    tool_results.push(result);
                }

                // Add tool result messages to conversation
                let tool_messages = ToolResult::from_tool_results(tool_results);
                messages.extend(tool_messages);
            } else {
                // No registry available - return response with tool calls
                tracing::warn!("Tool calls present but no registry available for execution");
                return Ok(response);
            }

            // Continue loop to get model's final response
        }
    }

    async fn complete_single_turn(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<String>,
        tool_registry: Option<&ToolRegistry>,
    ) -> Result<ModelResponse> {
        // Build chat messages for Ollama
        let mut chat_messages: Vec<ChatRequestMessage> = Vec::new();
        if let Some(sp) = system_prompt.as_ref() {
            chat_messages.push(ChatRequestMessage {
                role: "system",
                content: sp,
                name: None,
                tool_call_id: None,
            });
        }

        for msg in messages.iter() {
            // allow roles: user, assistant, tool
            let role = match msg.role.as_str() {
                "assistant" => "assistant",
                "tool" => "tool",
                _ => "user",
            };

            // Skip empty messages completely - tool results should always have content
            if msg.content.trim().is_empty() {
                tracing::warn!("Skipping empty message with role: {}", role);
                continue;
            }

            let content = msg.content.trim();

            chat_messages.push(ChatRequestMessage {
                role,
                content,
                name: msg.name.as_deref(),
                tool_call_id: msg.tool_call_id.as_deref(),
            });
        }

        if chat_messages.is_empty() {
            return Err(HostError::Model("No messages provided".to_string()));
        }

        // Use dynamic tool definitions from registry - required for GPT-OSS tool calling
        let tools = if let Some(registry) = tool_registry {
            registry.generate_ollama_tools().await
        } else {
            // No registry means no tool support - this is expected behavior
            tracing::info!("No tool registry available, completing without tool support");
            Vec::new()
        };

        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gpt-oss:20b".to_string());
        // For tool calling, disable thinking as it may cause parsing issues
        let include_thinking = tools.is_empty(); // Only include thinking when no tools are present
        
        // Retry loop for parse errors from tool calling
        const PARSE_RETRIES: usize = 10;
        let url = format!("{}/api/chat", self.base_url);
        let format_hint = "Format notice: previous reply had invalid tool-calling format. If you call a tool, return valid JSON in tool_calls with function.name and function.arguments as an object. No code fences or extra text.";

        for attempt in 0..PARSE_RETRIES {
            // Append a small hint on retries to nudge correct formatting
            let mut attempt_messages = chat_messages.clone();
            if attempt > 0 {
                attempt_messages.push(ChatRequestMessage {
                    role: "system",
                    content: format_hint,
                    name: None,
                    tool_call_id: None,
                });
            }

            let req = ChatRequest {
                model: &model,
                messages: attempt_messages,
                stream: false,
                tools: if tools.is_empty() { None } else { Some(tools.clone()) },
                reasoning: self
                    .reasoning_effort
                    .as_ref()
                    .map(|effort| Reasoning { effort: effort.clone() }),
                thinking: if include_thinking {
                    Some(Thinking { typ: "enabled".to_string(), budget_tokens: self.thinking_budget })
                } else {
                    None
                },
                options: None,
            };

            // Log request to file
            self.log_ollama_request(&req).await;

            let resp = self
                .client
                .post(&url)
                .json(&req)
                .send()
                .await
                .map_err(|e| HostError::Request(e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "<failed to read response>".to_string());
                return Err(HostError::Model(format!(
                    "Ollama chat error ({}): {}",
                    status, text
                )));
            }

            let response_text = resp
                .text()
                .await
                .map_err(|e| HostError::Model(format!("Failed to read response text: {}", e)))?;

            // Log response to file
            self.log_ollama_response(&response_text).await;

            // Treat non-JSON or explicit tool parsing error as a parse failure to retry
            if let Ok(parsed) = serde_json::from_str::<ChatResponse>(&response_text) {
                tracing::info!(
                    "Ollama response: content_len={}, tool_calls={}",
                    parsed.message.content.len(),
                    parsed
                        .message
                        .tool_calls
                        .as_ref()
                        .map_or(0, |tc| tc.len())
                );

                // Build structured response for caller (no harmony parsing)
                let model_resp = ModelResponse {
                    content: parsed.message.content.clone(),
                    thinking: parsed.message.thinking.clone(),
                    commentary: None,
                    tool_calls: parsed.message.tool_calls.clone(),
                };
                return Ok(model_resp);
            } else {
                // If the body contains a known tool-call parse error, retry silently with hint
                if response_text.contains("error parsing tool call") {
                    tracing::warn!("Retrying due to tool call parse error (attempt {}/{})", attempt + 1, PARSE_RETRIES);
                    continue;
                }
                // Otherwise, retry on generic parse failure
                tracing::warn!("Retrying due to unparseable response (attempt {}/{})", attempt + 1, PARSE_RETRIES);
                continue;
            }
        }

        Err(HostError::Model(
            "Failed to obtain valid model response after 10 parse retries".to_string(),
        ))
    }

    async fn execute_tool_call(&self, tool_call: &ToolCall, registry: &ToolRegistry) -> ToolResult {
        tracing::info!("Executing tool call: {} with id: {}", tool_call.function.name, tool_call.id);
        
        // Try to execute the tool via registry
        let result = registry.execute_tool(
            &tool_call.function.name,
            &tool_call.function.arguments,
        ).await;

        match result {
            Ok(output) => {
                tracing::info!("Tool call {} completed successfully", tool_call.id);
                ToolResult::new(tool_call.id.clone(), output)
            }
            Err(e) => {
                tracing::error!("Tool call {} failed: {}", tool_call.id, e);
                ToolResult::with_error(tool_call.id.clone(), e.to_string())
            }
        }
    }

    // All harmony/error-salvage parsing removed; rely on native tool_calls only and retry on parse errors.
    
    async fn log_ollama_request(&self, req: &ChatRequest<'_>) {
        if let Ok(req_json) = serde_json::to_string_pretty(req) {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let filename = format!("/agent/logs/ollama_request_{}.json", timestamp);
            
            let log_content = format!(
                "=== OLLAMA REQUEST {} ===\nModel: {}\nMessage Count: {}\nTools Count: {}\n\n{}",
                timestamp,
                req.model,
                req.messages.len(),
                req.tools.as_ref().map_or(0, |t| t.len()),
                req_json
            );
            
            if let Err(e) = tokio::fs::write(&filename, log_content).await {
                tracing::warn!("Failed to write Ollama request log to {}: {}", filename, e);
            }
        }
    }
    
    async fn log_ollama_response(&self, response_text: &str) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let filename = format!("/agent/logs/ollama_response_{}.json", timestamp);
        
        let log_content = format!(
            "=== OLLAMA RESPONSE {} ===\nResponse Length: {} characters\n\n{}",
            timestamp,
            response_text.len(),
            response_text
        );
        
        if let Err(e) = tokio::fs::write(&filename, log_content).await {
            tracing::warn!("Failed to write Ollama response log to {}: {}", filename, e);
        }
    }
}
