use super::error::{HostError, Result};
use reqwest::Client;
use serde::Deserialize;

pub struct GptClient {
    base_url: String,
    http: Client,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GenerateUsage {
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub gen_ms: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GenerateResult {
    pub text: String,
    #[serde(default)]
    pub usage: Option<GenerateUsage>,
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

        // Helper to map a tool-ish role into a proper assistant+recipient pair
        let mut map_role_token = |role: &str| -> Option<String> {
            // Recognized non-tool roles that should remain untouched
            match role {
                "assistant" | "user" | "system" | "developer" | "tool" => return None,
                _ => {}
            }
            // Normalize various tool name spellings
            let mut name = role.trim();
            if let Some(rest) = name.strip_prefix("functions.") {
                name = rest;
            }
            if let Some(rest) = name.strip_prefix("tool.") {
                name = rest;
            }
            // Map aliases
            let mapped = match name {
                "container.exec" => "bash",
                other => other,
            };
            Some(format!(
                "<|assistant|><|recipient|>functions.{}",
                mapped
            ))
        };

        // Replace obvious wrong role headers for common tools and their function.* variants
        let names = ["bash", "text_editor", "publish", "sleep", "container.exec"];
        for name in names.iter() {
            // Standalone wrong role tags like <|bash|>
            let wrong = format!("<|{}|>", name);
            if let Some(fix) = map_role_token(name) {
                s = s.replace(&wrong, &fix);
            }
            // Mis-nested after assistant: <|assistant|><|bash|>
            let wrong = format!("<|assistant|><|{}|>", name);
            if let Some(fix_only_recipient) = map_role_token(name) {
                // This already includes <|assistant|>, so keep only recipient part
                let fix_rec = fix_only_recipient.replace("<|assistant|>", "");
                s = s.replace(&wrong, &format!("<|assistant|>{}", fix_rec));
            }
            // Bad recipient values like <|recipient|>bash → prefix functions.
            let wrong = format!("<|recipient|>{}", name);
            let mapped = match *name {
                "container.exec" => "bash",
                other => other,
            };
            let fix = format!("<|recipient|>functions.{}", mapped);
            s = s.replace(&wrong, &fix);

            // Wrong role using functions.* as role: <|functions.bash|>
            let wrong = format!("<|functions.{}|>", name);
            if let Some(fix2) = map_role_token(&format!("functions.{}", name)) {
                s = s.replace(&wrong, &fix2);
            }
        }

        s
    }

    

    pub async fn generate(
        &self,
        prompt: &str,
        params: Option<serde_json::Value>,
    ) -> Result<GenerateResult> {
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
        let mut v: GenerateResult = resp
            .json()
            .await
            .map_err(|e| HostError::Model(format!("Invalid JSON from GPT server: {}", e)))?;
        v.text = Self::normalize_completion(&v.text);
        Ok(v)
    }
}
