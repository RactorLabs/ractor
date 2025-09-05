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
struct ChatResponseMessage {
    #[serde(default)]
    role: String,
    content: String,
    #[serde(default)]
    thinking: Option<String>,
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
            .unwrap_or(120);

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

        // Advertise tools per cookbook so the model emits structured tool calls
        let tools = vec![
            ToolDef {
                typ: ToolType::Function,
                function: ToolFunction {
                    name: "bash".to_string(),
                    description: "Execute a bash shell command in /agent".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "cmd": {"type": "string", "description": "Command to run"},
                            "args": {"oneOf": [{"type":"array","items":{"type":"string"}}, {"type":"string"}]},
                            "command": {"type": "string"}
                        }
                    }),
                },
            },
            ToolDef {
                typ: ToolType::Function,
                function: ToolFunction {
                    name: "text_editor".to_string(),
                    description: "View/create/replace/insert in files under /agent".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "action": {"type": "string", "enum": ["view","create","str_replace","insert"]},
                            "operation": {"type":"string"},
                            "path": {"type": "string"},
                            "file_path": {"type":"string"},
                            "content": {"type": "string"},
                            "file_text": {"type":"string"},
                            "target": {"type": "string"},
                            "old_str": {"type":"string"},
                            "replacement": {"type": "string"},
                            "new_str": {"type":"string"},
                            "line": {"type": "integer", "minimum": 1},
                            "insert_line": {"type":"integer","minimum":1},
                            "start_line": {"type": "integer", "minimum": 1},
                            "end_line": {"type": "integer", "minimum": 1},
                            "view_range": {"type":"array","items":{"type":"integer"},"minItems":2,"maxItems":2}
                        }
                    }),
                },
            },
        ];

        let req = ChatRequest {
            model: "gpt-oss",
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

        let parsed: ChatResponse = resp
            .json()
            .await
            .map_err(|e| HostError::Model(format!("Failed to parse Ollama response: {}", e)))?;

        Ok(parsed.message.content)
    }
}
