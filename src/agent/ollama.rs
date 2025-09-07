use super::api::RaworcClient;
use super::error::{HostError, Result};
use super::tool_registry::ToolRegistry;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct OllamaClient {
    client: Client,
    base_url: String,
    api_client: Option<Arc<RaworcClient>>, // kept for parity; unused currently
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

#[derive(Debug, Deserialize, Clone)]
pub struct ToolCall {
    pub id: String,
    pub function: ToolCallFunction,
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
        let timeout_secs = std::env::var("OLLAMA_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(600);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| HostError::Model(format!("Failed to create client: {}", e)))?;

        // Reasoning controls via env with sensible defaults
        // Default to high reasoning effort and a thinking budget if not provided
        let reasoning_effort = Some(
            std::env::var("OLLAMA_REASONING_EFFORT")
                .ok()
                .unwrap_or_else(|| "high".to_string()),
        );
        let thinking_budget = std::env::var("OLLAMA_THINKING_TOKENS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .or(Some(4096));

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_client: None,
            reasoning_effort,
            thinking_budget,
        })
    }

    pub fn set_api_client(&mut self, api_client: Arc<RaworcClient>) {
        self.api_client = Some(api_client);
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

            // Skip empty messages, but preserve tool messages even if empty
            // to maintain conversation flow after tool calls
            if msg.content.trim().is_empty() && role != "tool" {
                continue;
            }

            // For tool messages, use a placeholder if content is empty to maintain flow
            let content = if role == "tool" && msg.content.trim().is_empty() {
                "[tool output]"
            } else {
                msg.content.trim()
            };

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

        // Use dynamic tool definitions from registry if available, otherwise fallback to static tools
        let tools = if let Some(registry) = tool_registry {
            registry.generate_ollama_tools().await
        } else {
            // Fallback to static tool definitions for backward compatibility
            vec![
                ToolDef {
                    typ: ToolType::Function,
                    function: ToolFunction {
                        name: "bash".to_string(),
                        description: "Execute a bash shell command in the /agent directory"
                            .to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {
                                "command": {
                                    "type": "string",
                                    "description": "The bash command to execute"
                                }
                            },
                            "required": ["command"]
                        }),
                    },
                },
                ToolDef {
                    typ: ToolType::Function,
                    function: ToolFunction {
                        name: "text_editor".to_string(),
                        description:
                            "Perform text editing operations on files in the /agent directory"
                                .to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {
                                "action": {
                                    "type": "string",
                                    "enum": ["view", "create", "str_replace", "insert"],
                                    "description": "The editing action to perform"
                                },
                                "path": {
                                    "type": "string",
                                    "description": "The file path relative to /agent"
                                },
                                "content": {
                                    "type": "string",
                                    "description": "Content for create/insert operations"
                                },
                                "target": {
                                    "type": "string",
                                    "description": "Text to find for str_replace operation"
                                },
                                "replacement": {
                                    "type": "string",
                                    "description": "Replacement text for str_replace operation"
                                },
                                "line": {
                                    "type": "integer",
                                    "minimum": 1,
                                    "description": "Line number for insert operation"
                                },
                                "start_line": {
                                    "type": "integer",
                                    "minimum": 1,
                                    "description": "Start line for view operation"
                                },
                                "end_line": {
                                    "type": "integer",
                                    "minimum": 1,
                                    "description": "End line for view operation"
                                }
                            },
                            "required": ["action", "path"]
                        }),
                    },
                },
            ]
        };

        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gpt-oss:20b".to_string());
        let req = ChatRequest {
            model: &model,
            messages: chat_messages,
            stream: false,
            tools: Some(tools),
            reasoning: self.reasoning_effort.as_ref().map(|effort| Reasoning { effort: effort.clone() }),
            thinking: Some(Thinking { typ: "enabled".to_string(), budget_tokens: self.thinking_budget }),
        };

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

        tracing::info!("Raw Ollama response: {}", response_text);

        let parsed: ChatResponse = serde_json::from_str(&response_text).map_err(|e| {
            HostError::Model(format!(
                "Failed to parse Ollama response: {} | Raw: {}",
                e, response_text
            ))
        })?;

        tracing::info!(
            "Ollama response parsed successfully, content length: {}",
            parsed.message.content.len()
        );
        tracing::info!(
            "Content preview: {:?}",
            if parsed.message.content.chars().count() > 100 {
                format!("{}...", parsed.message.content.chars().take(100).collect::<String>())
            } else {
                parsed.message.content.clone()
            }
        );
        tracing::info!(
            "Tool calls present: {:?}",
            parsed.message.tool_calls.is_some()
        );
        if let Some(ref tool_calls) = parsed.message.tool_calls {
            tracing::info!("Number of tool calls: {}", tool_calls.len());
        }

        // Handle structured tool calls first (GPT-OSS native format)
        // Build structured response for caller
        let model_resp = ModelResponse {
            content: parsed.message.content.clone(),
            thinking: parsed.message.thinking.clone(),
            tool_calls: parsed.message.tool_calls.clone(),
        };

        if let Some(ref calls) = model_resp.tool_calls {
            tracing::info!("Structured tool calls present: {}", calls.len());
        } else {
            tracing::info!("No structured tool calls found, content-only response");
        }

        Ok(model_resp)
    }
}
