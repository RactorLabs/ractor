use super::error::{HostError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct InferenceClient {
    client: Client,
    base_url: String,
    auth_header: Option<String>,
    log_seq: Arc<AtomicU64>,
}

#[derive(Debug, Serialize, Clone)]
struct ChatRequestMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
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

#[derive(Debug, Clone)]
pub struct ModelResponse {
    pub content: String,
    pub total_tokens: Option<i64>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
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

        let auth_header = std::env::var("TSBX_INFERENCE_API_KEY")
            .ok()
            .map(|key| format!("Bearer {}", key.trim()));

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_header,
            log_seq: Arc::new(AtomicU64::new(0)),
        })
    }

    pub async fn complete(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<String>,
    ) -> Result<ModelResponse> {
        const PARSE_RETRIES: usize = 5;
        let model_name = std::env::var("TSBX_INFERENCE_MODEL")
            .or_else(|_| std::env::var("TSBX_DEFAULT_MODEL"))
            .unwrap_or_else(|_| "llama-3.1-8b-instruct-good-tp2".to_string());
        let url = format!("{}/chat/completions", self.base_url);
        let format_hint =
            "Format notice: Respond with a single XML element (e.g. <run_bash .../> or <output>...</output>).";

        let mut base_messages: Vec<ChatRequestMessage> = Vec::new();
        if let Some(sp) = system_prompt {
            base_messages.push(ChatRequestMessage {
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
            base_messages.push(ChatRequestMessage {
                role: msg.role.clone(),
                content: trimmed.to_string(),
                name: msg.name.clone(),
                tool_call_id: msg.tool_call_id.clone(),
            });
        }

        if base_messages.is_empty() {
            return Err(HostError::Model("No messages provided".to_string()));
        }

        for attempt in 0..PARSE_RETRIES {
            let mut attempt_messages = base_messages.clone();
            if attempt > 0 {
                attempt_messages.push(ChatRequestMessage {
                    role: "system".to_string(),
                    content: format_hint.to_string(),
                    name: None,
                    tool_call_id: None,
                });
            }

            let req = ChatRequest {
                model: model_name.clone(),
                messages: attempt_messages,
                stream: false,
            };

            let log_id = self.log_seq.fetch_add(1, Ordering::SeqCst) + 1;
            self.log_inference_request(&req, log_id).await;

            let mut request_builder = self.client.post(&url).json(&req);
            if let Some(header) = &self.auth_header {
                request_builder = request_builder.header("Authorization", header);
            }

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

            match serde_json::from_str::<ChatResponse>(&response_text) {
                Ok(parsed) => {
                    let choice = parsed.choices.into_iter().next().ok_or_else(|| {
                        HostError::Model("Inference response missing choices".into())
                    })?;

                    let raw_content = choice.message.content.unwrap_or_default();
                    let usage = parsed.usage.unwrap_or_default();

                    return Ok(ModelResponse {
                        content: raw_content.trim().to_string(),
                        total_tokens: usage.total_tokens,
                        prompt_tokens: usage.prompt_tokens,
                        completion_tokens: usage.completion_tokens,
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse inference response JSON (attempt {}/{}): {}",
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

    async fn log_inference_request(&self, req: &ChatRequest, id: u64) {
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

fn warn_missing_tool(status: reqwest::StatusCode, body: &str, attempt: usize) {
    tracing::warn!(
        "Retrying inference call due to error (attempt {}/5) status={} body={}",
        attempt + 1,
        status,
        body
    );
}
