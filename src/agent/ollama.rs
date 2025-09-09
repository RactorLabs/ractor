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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    Function,
}

#[derive(Debug, Serialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
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
        
        let req = ChatRequest {
            model: &model,
            messages: chat_messages,
            stream: false,
            tools: if tools.is_empty() { None } else { Some(tools) },
            reasoning: self.reasoning_effort.as_ref().map(|effort| Reasoning { effort: effort.clone() }),
            thinking: if include_thinking { 
                Some(Thinking { typ: "enabled".to_string(), budget_tokens: self.thinking_budget })
            } else { 
                None 
            },
            options: None, // Context length now set via CLI environment variables
        };

        // Log request to file
        self.log_ollama_request(&req).await;

        let url = format!("{}/api/chat", self.base_url);
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

        // Debug: Get raw response text first
        let response_text = resp
            .text()
            .await
            .map_err(|e| HostError::Model(format!("Failed to read response text: {}", e)))?;

        // Log response to file
        self.log_ollama_response(&response_text).await;
        
        // Additional debug: Check if this is a tool calling error
        if response_text.contains("error parsing tool call") {
            tracing::error!("=== OLLAMA TOOL CALL PARSING ERROR ===");
            tracing::error!("Full error response: {}", response_text);
            tracing::error!("This suggests malformed JSON in tool call arguments");
            tracing::error!("Will attempt to extract and sanitize tool calls manually");
            tracing::error!("=== END ERROR DEBUG ===");
            
            // Try to extract tool calls manually from the error
            if let Some(tool_calls) = self.extract_failed_tool_calls(&response_text) {
                tracing::info!("Successfully extracted {} tool calls from error response", tool_calls.len());
                return Ok(ModelResponse {
                    content: "".to_string(),
                    thinking: None,
                    tool_calls: Some(tool_calls),
                });
            }
        }

        let parsed: ChatResponse = serde_json::from_str(&response_text).map_err(|e| {
            HostError::Model(format!(
                "Failed to parse Ollama response: {} | Raw: {}",
                e, response_text
            ))
        })?;

        // Minimal logging for Docker logs
        tracing::info!(
            "Ollama response: content_len={}, tool_calls={}", 
            parsed.message.content.len(),
            parsed.message.tool_calls.as_ref().map_or(0, |tc| tc.len())
        );

        // Handle structured tool calls first (GPT-OSS native format)
        // Check for harmony format channels in content
        let (final_content, analysis_thinking, commentary_tools) = self.parse_harmony_channels(&parsed.message.content);
        
        // Use harmony-parsed content if available, otherwise use original
        let response_content = if !final_content.is_empty() {
            final_content
        } else {
            parsed.message.content.clone()
        };
        
        let response_thinking = if let Some(harmony_thinking) = analysis_thinking {
            Some(harmony_thinking)
        } else {
            parsed.message.thinking.clone()
        };
        
        let response_tool_calls = if let Some(harmony_tools) = commentary_tools {
            Some(harmony_tools)
        } else {
            parsed.message.tool_calls.clone()
        };
        
        // Build structured response for caller
        let model_resp = ModelResponse {
            content: response_content,
            thinking: response_thinking,
            tool_calls: response_tool_calls,
        };

        // Tool calls logged in minimal response info above

        Ok(model_resp)
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

    fn extract_failed_tool_calls(&self, error_response: &str) -> Option<Vec<ToolCall>> {
        // Try to extract tool calls from Ollama error responses
        // Look for patterns like: "error parsing tool call: raw='{"command":"..."}'
        
        if let Some(start) = error_response.find("raw='") {
            let start_idx = start + 5; // Length of "raw='"
            if let Some(end_idx) = error_response[start_idx..].find("', err=") {
                let raw_content = &error_response[start_idx..start_idx + end_idx];
                tracing::info!("Extracted raw tool call content: {}", raw_content);
                
                // Try to parse as JSON and fix common issues
                let sanitized = self.sanitize_tool_call_json(raw_content);
                
                if let Ok(args) = serde_json::from_str::<serde_json::Value>(&sanitized) {
                    // Create a tool call - we need to infer the tool name from context
                    let tool_name = self.infer_tool_name_from_args(&args);
                    
                    let tool_call = ToolCall {
                        id: generate_tool_call_id(),
                        function: ToolCallFunction {
                            name: tool_name,
                            arguments: args,
                        },
                    };
                    
                    return Some(vec![tool_call]);
                }
            }
        }
        
        None
    }
    
    fn sanitize_tool_call_json(&self, raw_json: &str) -> String {
        // Fix common JSON issues in GPT-OSS tool calls
        let mut sanitized = raw_json.to_string();
        
        // Fix escaped quotes and newlines
        sanitized = sanitized.replace("\\\"", "\"");
        sanitized = sanitized.replace("\\n", "\n");
        sanitized = sanitized.replace("\\t", "\t");
        
        // Fix unescaped quotes in string values
        // This is more complex - for now, just log and return as-is
        tracing::info!("Sanitized JSON: {}", sanitized);
        
        sanitized
    }
    
    fn infer_tool_name_from_args(&self, args: &serde_json::Value) -> String {
        // Infer tool name from arguments structure
        if args.get("command").is_some() {
            "bash".to_string()
        } else if args.get("action").is_some() || args.get("path").is_some() {
            "text_editor".to_string()
        } else {
            "bash".to_string() // Default fallback
        }
    }
    
    fn parse_harmony_channels(&self, content: &str) -> (String, Option<String>, Option<Vec<ToolCall>>) {
        // Parse harmony format channels from content
        // Format: <|start|>assistant<|channel|>{channel}<|message|>{content}<|end|>
        
        let mut final_content = String::new();
        let mut analysis_content = None;
        let mut commentary_tools = None;
        
        // Look for channel patterns
        for line in content.lines() {
            if line.contains("<|channel|>final<|message|>") {
                if let Some(msg_start) = line.find("<|message|>") {
                    let start_idx = msg_start + 11; // Length of "<|message|>"
                    if let Some(end_idx) = line.find("<|end|>") {
                        final_content = line[start_idx..end_idx].to_string();
                    } else {
                        final_content = line[start_idx..].to_string();
                    }
                }
            } else if line.contains("<|channel|>analysis<|message|>") {
                if let Some(msg_start) = line.find("<|message|>") {
                    let start_idx = msg_start + 11;
                    if let Some(end_idx) = line.find("<|end|>") {
                        analysis_content = Some(line[start_idx..end_idx].to_string());
                    } else {
                        analysis_content = Some(line[start_idx..].to_string());
                    }
                }
            } else if line.contains("<|channel|>commentary<|message|>") {
                if let Some(msg_start) = line.find("<|message|>") {
                    let start_idx = msg_start + 11;
                    let commentary_text = if let Some(end_idx) = line.find("<|end|>") {
                        &line[start_idx..end_idx]
                    } else {
                        &line[start_idx..]
                    };
                    
                    // Try to parse tool calls from commentary
                    commentary_tools = self.parse_commentary_tool_calls(commentary_text);
                }
            }
        }
        
        (final_content, analysis_content, commentary_tools)
    }
    
    fn parse_commentary_tool_calls(&self, commentary: &str) -> Option<Vec<ToolCall>> {
        // Try to parse tool calls from commentary channel
        // Commentary might contain function call descriptions
        
        // For now, return None - this would need more sophisticated parsing
        // based on the actual harmony format structure
        tracing::info!("Commentary channel content: {}", commentary);
        None
    }
    
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
