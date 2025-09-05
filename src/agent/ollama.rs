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
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatRequestMessage<'a>>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: ChatResponseMessage,
}

impl OllamaClient {
    pub fn new(base_url: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
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
        messages: Vec<(String, String)>, // (role, content)
        system_prompt: Option<String>,
    ) -> Result<String> {
        // Build chat messages for Ollama
        let mut chat_messages: Vec<ChatRequestMessage> = Vec::new();
        if let Some(sp) = system_prompt.as_ref() {
            chat_messages.push(ChatRequestMessage {
                role: "system",
                content: sp,
            });
        }

        for (role, content) in messages.iter() {
            let role = match role.as_str() {
                "assistant" | "HOST" => "assistant",
                _ => "user",
            };
            if content.trim().is_empty() {
                continue;
            }
            chat_messages.push(ChatRequestMessage {
                role,
                content: content.trim(),
            });
        }

        if chat_messages.is_empty() {
            return Err(HostError::Model("No messages provided".to_string()));
        }

        let req = ChatRequest {
            model: "gpt-oss",
            messages: chat_messages,
            stream: false,
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

