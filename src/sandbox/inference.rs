use super::error::{HostError, Result};
use super::inference_templates::get_template;
use reqwest::Client;
use std::cmp::max;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct InferenceClient {
    client: Client,
    base_url: String,
    auth_header: Option<String>,
    log_seq: Arc<AtomicU64>,
    template: String,
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

        let template_raw =
            std::env::var("TSBX_INFERENCE_TEMPLATE").unwrap_or_else(|_| "openai".to_string());
        let template = match template_raw.trim().to_ascii_lowercase().as_str() {
            "positron" => "positron".to_string(),
            "openai" | "" => "openai".to_string(),
            other => other.to_string(),
        };

        // Validate template name
        let _ = get_template(&template)?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_header,
            log_seq: Arc::new(AtomicU64::new(0)),
            template,
        })
    }

    pub async fn complete(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<String>,
    ) -> Result<ModelResponse> {
        const PARSE_RETRIES: usize = 5;
        let model_name = std::env::var("TSBX_INFERENCE_MODEL")
            .unwrap_or_else(|_| "llama-3.2-3b-instruct-fast-tp2".to_string());
        let url = format!("{}/chat/completions", self.base_url);

        let template = get_template(&self.template)?;
        let format_hint = template.format_hint();

        let base_messages = messages.clone();

        for attempt in 0..PARSE_RETRIES {
            let mut attempt_messages = base_messages.clone();

            // Add format hint on retries
            if attempt > 0 {
                attempt_messages.push(ChatMessage {
                    role: "system".to_string(),
                    content: format_hint.to_string(),
                    name: None,
                    tool_call_id: None,
                });
            }

            let req_value = template
                .build_request(attempt_messages.clone(), system_prompt.clone(), &model_name)
                .await?;

            let estimated_context_length = Self::estimate_context_length(&attempt_messages);

            let log_id = self.log_seq.fetch_add(1, Ordering::SeqCst) + 1;
            self.log_inference_request(&req_value, log_id).await;

            let mut request_builder = self.client.post(&url).json(&req_value);
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

            match template
                .parse_response(&response_text, estimated_context_length)
                .await
            {
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
            let per_message_overhead = 4; // rough allowance for role/name metadata
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

fn warn_missing_tool(status: reqwest::StatusCode, body: &str, attempt: usize) {
    tracing::warn!(
        "Retrying inference call due to error (attempt {}/5) status={} body={}",
        attempt + 1,
        status,
        body
    );
}
