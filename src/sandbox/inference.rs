use super::error::{HostError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct InferenceClient {
    client: Client,
    base_url: String,
    auth_header: String,
    log_seq: Arc<AtomicU64>,
    model: String,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolCall {
    pub id: Option<String>,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ModelResponse {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub total_tokens: Option<i64>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub context_length: Option<i64>,
}

const FORMAT_HINT: &str =
    "Format notice: Respond with a single XML element (e.g. <run_bash .../> or <output>...</output>).";

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
    #[serde(default)]
    tool_calls: Vec<ChoiceToolCall>,
}

#[derive(Debug, Deserialize)]
struct ChoiceToolCall {
    id: Option<String>,
    #[serde(default)]
    function: ChoiceFunction,
}

#[derive(Debug, Deserialize, Default)]
struct ChoiceFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize, Default)]
struct Usage {
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
}

impl InferenceClient {
    pub fn new(base_url: &str) -> Result<Self> {
        let timeout_secs = std::env::var("TSBX_INFERENCE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(900);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| HostError::Model(format!("Failed to create inference client: {}", e)))?;

        let raw_key = std::env::var("TSBX_INFERENCE_API_KEY").unwrap_or_else(|_| "".to_string());
        let trimmed_key = raw_key.trim();
        let auth_header = if trimmed_key.is_empty() {
            "".to_string()
        } else {
            format!("Bearer {}", trimmed_key)
        };

        let raw_model = std::env::var("TSBX_INFERENCE_MODEL")
            .map_err(|_| HostError::Model("TSBX_INFERENCE_MODEL must be set".to_string()))?;
        let trimmed_model = raw_model.trim();
        if trimmed_model.is_empty() {
            return Err(HostError::Model(
                "TSBX_INFERENCE_MODEL must not be empty".to_string(),
            ));
        }
        let model = trimmed_model.to_string();

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_header,
            log_seq: Arc::new(AtomicU64::new(0)),
            model,
        })
    }

    pub async fn complete(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<String>,
    ) -> Result<ModelResponse> {
        const PARSE_RETRIES: usize = 5;
        let base_messages = messages.clone();

        for attempt in 0..PARSE_RETRIES {
            let mut attempt_messages = base_messages.clone();

            if attempt > 0 {
                attempt_messages.push(ChatMessage {
                    role: "system".to_string(),
                    content: FORMAT_HINT.to_string(),
                    name: None,
                    tool_call_id: None,
                });
            }

            let req_value =
                build_request(attempt_messages.clone(), system_prompt.clone(), &self.model)?;

            let estimated_context_length = Self::estimate_context_length(&attempt_messages);

            let log_id = self.log_seq.fetch_add(1, Ordering::SeqCst) + 1;
            self.log_inference_request(&req_value, log_id).await;

            let mut request_builder = self.client.post(self.base_url.as_str()).json(&req_value);
            request_builder = request_builder.header("Authorization", &self.auth_header);

            let resp = request_builder.send().await.map_err(HostError::Request)?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "<failed to read response>".to_string());

                if attempt + 1 < PARSE_RETRIES {
                    warn_missing_tool(status, &text, attempt);
                    continue;
                }

                return Err(HostError::Model(format!(
                    "Inference service error ({}): {}",
                    status, text
                )));
            }

            let response_text = resp
                .text()
                .await
                .map_err(|e| HostError::Model(format!("Failed to read response text: {}", e)))?;

            self.log_inference_response(&response_text, log_id).await;

            match parse_response(&response_text, estimated_context_length) {
                Ok(response) => return Ok(response),
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse inference response (attempt {}/{}): {}",
                        attempt + 1,
                        PARSE_RETRIES,
                        e
                    );
                }
            }
        }

        Err(HostError::Model(
            "Inference response parsing failed after retries".to_string(),
        ))
    }

    fn estimate_context_length(messages: &[ChatMessage]) -> i64 {
        messages.iter().fold(0i64, |acc, msg| {
            let content = msg.content.trim();
            if content.is_empty() {
                return acc;
            }
            let char_count = content.chars().count() as i64;
            let word_count = content.split_whitespace().filter(|w| !w.is_empty()).count() as i64;
            let approx_content_tokens = max((char_count + 3) / 4, max(word_count, 1));
            let per_message_overhead = 4;
            acc.saturating_add(approx_content_tokens + per_message_overhead)
        })
    }

    async fn log_inference_request(&self, req: &serde_json::Value, id: u64) {
        if let Ok(json) = serde_json::to_string_pretty(req) {
            let filename = format!("/sandbox/logs/inference_{}_request.json", id);
            if let Err(e) = tokio::fs::write(&filename, json).await {
                tracing::warn!("Failed to write inference request log: {}", e);
            }
        }
    }

    async fn log_inference_response(&self, response_text: &str, id: u64) {
        let filename = format!("/sandbox/logs/inference_{}_response.json", id);
        if let Err(e) = tokio::fs::write(&filename, response_text).await {
            tracing::warn!("Failed to write inference response log: {}", e);
        }
    }
}

fn build_request(
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

fn parse_response(response_text: &str, estimated_context_length: i64) -> Result<ModelResponse> {
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

    let tool_calls = if choice.message.tool_calls.is_empty() {
        None
    } else {
        let mut calls = Vec::new();
        for call in choice.message.tool_calls {
            let args_value =
                match serde_json::from_str::<serde_json::Value>(&call.function.arguments) {
                    Ok(v) => v,
                    Err(_) => serde_json::Value::Null,
                };
            calls.push(ToolCall {
                id: call.id,
                name: call.function.name,
                arguments: args_value,
            });
        }
        Some(calls)
    };

    Ok(ModelResponse {
        content: Some(raw_content.trim().to_string()),
        tool_calls,
        total_tokens: usage.total_tokens,
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        context_length: Some(context_length.max(0)),
    })
}

fn warn_missing_tool(status: reqwest::StatusCode, body: &str, attempt: usize) {
    tracing::warn!(
        "Retrying inference call due to error (attempt {}/5) status={} body={}",
        attempt + 1,
        status,
        body
    );
}
