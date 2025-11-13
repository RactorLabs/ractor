use super::super::error::{HostError, Result};
use super::super::inference::{ChatMessage, ModelResponse};
use super::InferenceTemplate;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
struct ChatRequestMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatRequestMessage>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct Usage {
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
}

pub struct PositronTemplate {}

impl PositronTemplate {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl InferenceTemplate for PositronTemplate {
    async fn build_request(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<String>,
        model_name: &str,
    ) -> Result<serde_json::Value> {
        let mut request_messages: Vec<ChatRequestMessage> = Vec::new();

        if let Some(sp) = system_prompt {
            request_messages.push(ChatRequestMessage {
                role: "system".to_string(),
                content: sp,
                name: None,
                tool_call_id: None,
            });
        }

        for msg in messages.iter() {
            let trimmed = msg.content.trim();
            if trimmed.is_empty() {
                continue;
            }
            request_messages.push(ChatRequestMessage {
                role: msg.role.clone(),
                content: trimmed.to_string(),
                name: msg.name.clone(),
                tool_call_id: msg.tool_call_id.clone(),
            });
        }

        if request_messages.is_empty() {
            return Err(HostError::Model("No messages provided".to_string()));
        }

        let req = ChatRequest {
            model: model_name.to_string(),
            messages: request_messages,
            stream: false,
        };

        serde_json::to_value(&req)
            .map_err(|e| HostError::Model(format!("Failed to serialize request: {}", e)))
    }

    async fn parse_response(
        &self,
        response_text: &str,
        estimated_context_length: i64,
    ) -> Result<ModelResponse> {
        let parsed: ChatResponse = serde_json::from_str(response_text)
            .map_err(|e| HostError::Model(format!("Failed to parse response: {}", e)))?;

        let choice = parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| HostError::Model("Inference response missing choices".into()))?;

        let raw_content = choice.message.content.unwrap_or_default();
        let usage = parsed.usage.unwrap_or_default();
        let context_length = usage
            .prompt_tokens
            .or(usage.total_tokens)
            .unwrap_or(estimated_context_length);

        Ok(ModelResponse {
            content: raw_content.trim().to_string(),
            total_tokens: usage.total_tokens,
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            context_length: Some(context_length.max(0)),
        })
    }

    fn format_hint(&self) -> &str {
        "Format notice: Respond with a single XML element (e.g. <run_bash .../> or <output>...</output>)."
    }
}
