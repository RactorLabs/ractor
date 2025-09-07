use super::api::RaworcClient;
use super::error::{HostError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct OllamaClient {
    client: Client,
    base_url: String,
    api_client: Option<Arc<RaworcClient>>, // kept for parity; unused currently
}

#[derive(Debug, Serialize)]
struct ChatRequestMessage<'a> {
    role: &'a str,
    content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ToolType {
    Function,
}

#[derive(Debug, Serialize)]
struct ToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ToolDef {
    #[serde(rename = "type")]
    typ: ToolType,
    function: ToolFunction,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatRequestMessage<'a>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDef>>,
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    function: ToolCallFunction,
}

#[derive(Debug, Deserialize)]
struct ToolCallFunction {
    name: String,
    arguments: serde_json::Value,
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

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_client: None,
        })
    }

    pub fn set_api_client(&mut self, api_client: Arc<RaworcClient>) {
        self.api_client = Some(api_client);
    }

    pub async fn complete(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<String>,
    ) -> Result<String> {
        // Build chat messages for Ollama
        let mut chat_messages: Vec<ChatRequestMessage> = Vec::new();
        if let Some(sp) = system_prompt.as_ref() {
            chat_messages.push(ChatRequestMessage {
                role: "system",
                content: sp,
                name: None,
            });
        }

        for msg in messages.iter() {
            // allow roles: user, assistant, tool
            let role = match msg.role.as_str() {
                "assistant" => "assistant",
                "tool" => "tool",
                _ => "user",
            };
            if msg.content.trim().is_empty() {
                continue;
            }
            chat_messages.push(ChatRequestMessage {
                role,
                content: msg.content.trim(),
                name: msg.name.as_deref(),
            });
        }

        if chat_messages.is_empty() {
            return Err(HostError::Model("No messages provided".to_string()));
        }

        // Clean tool definitions following Ollama cookbook standards
        let tools = vec![
            ToolDef {
                typ: ToolType::Function,
                function: ToolFunction {
                    name: "bash".to_string(),
                    description: "Execute a bash shell command in the /agent directory".to_string(),
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
                    description: "Perform text editing operations on files in the /agent directory".to_string(),
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
        ];

        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gpt-oss:20b".to_string());
        let req = ChatRequest {
            model: &model,
            messages: chat_messages,
            stream: false,
            tools: Some(tools),
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
        let response_text = resp.text().await
            .map_err(|e| HostError::Model(format!("Failed to read response text: {}", e)))?;
        
        tracing::info!("Raw Ollama response: {}", response_text);
        
        let parsed: ChatResponse = serde_json::from_str(&response_text)
            .map_err(|e| HostError::Model(format!("Failed to parse Ollama response: {} | Raw: {}", e, response_text)))?;

        tracing::info!("Ollama response parsed successfully, content length: {}", parsed.message.content.len());
        tracing::info!("Content preview: {:?}", if parsed.message.content.len() > 100 { 
            format!("{}...", &parsed.message.content[..100]) 
        } else { 
            parsed.message.content.clone() 
        });
        tracing::info!("Tool calls present: {:?}", parsed.message.tool_calls.is_some());
        if let Some(ref tool_calls) = parsed.message.tool_calls {
            tracing::info!("Number of tool calls: {}", tool_calls.len());
        }

        // Handle structured tool calls first (GPT-OSS native format)
        if let Some(tool_calls) = &parsed.message.tool_calls {
            if let Some(first_call) = tool_calls.first() {
                tracing::info!("Found structured tool call: {} with args: {:?}", 
                    first_call.function.name, first_call.function.arguments);
                
                // Return the structured tool call directly
                let tool_call_json = serde_json::json!({
                    "tool_calls": [{
                        "function": {
                            "name": first_call.function.name,
                            "arguments": first_call.function.arguments
                        }
                    }]
                });
                return Ok(tool_call_json.to_string());
            }
        }

        // Return content as regular text response
        tracing::info!("No structured tool calls found, returning content as text response");
        Ok(parsed.message.content.clone())
    }
}
