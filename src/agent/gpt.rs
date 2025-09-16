use super::error::{HostError, Result};
use reqwest::Client;

pub struct GptClient {
    base_url: String,
    http: Client,
}

impl GptClient {
    pub fn new(base_url: &str) -> Result<Self> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|e| HostError::Model(format!("Failed to build HTTP client: {}", e)))?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
        })
    }

    fn normalize_completion(raw: &str) -> String {
        let mut s = raw.replace('\u{ff5c}', "|");
        // Repair a couple of common header glitches without altering content otherwise
        s = s.replace("<|assistant<|channel|>", "<|assistant|><|channel|>");
        s = s.replace("<|assistant<|message|>", "<|assistant|><|message|>");
        s
    }

    pub async fn generate(
        &self,
        prompt: &str,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        let url = format!("{}/generate", self.base_url);
        let mut body = serde_json::json!({ "prompt": prompt });
        if let Some(obj) = params {
            if let Some(map) = obj.as_object() {
                for (k, v) in map.iter() {
                    body.as_object_mut().unwrap().insert(k.clone(), v.clone());
                }
            }
        }
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HostError::Model(format!("GPT server error: {}", e)))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(HostError::Model(format!(
                "GPT server responded {}: {}",
                status, text
            )));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| HostError::Model(format!("Invalid JSON from GPT server: {}", e)))?;
        let text = v
            .get("text")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        Ok(Self::normalize_completion(&text))
    }
}
