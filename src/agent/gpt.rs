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

        // Whitelist normalization only for known tool names and aliases.
        // This avoids touching Harmony control tokens like <|end|>, <|call|>, <|return|>.
        let tool_names = ["bash", "text_editor", "publish", "sleep", "container.exec"];
        let function_names = [
            "functions.bash",
            "functions.text_editor",
            "functions.publish",
            "functions.sleep",
            "functions.container.exec",
        ];

        // 1) Mis-nested after assistant: <|assistant|><|bash|>
        for name in tool_names.iter() {
            let wrong = format!("<|assistant|><|{}|>", name);
            let mapped = if *name == "container.exec" { "bash" } else { name };
            let fix = format!("<|assistant|><|recipient|>functions.{}", mapped);
            s = s.replace(&wrong, &fix);
        }
        for fname in function_names.iter() {
            let wrong = format!("<|assistant|><|{}|>", fname);
            let val = fname.strip_prefix("functions.").unwrap_or(fname);
            let mapped = if val == "container.exec" { "bash" } else { val };
            let fix = format!("<|assistant|><|recipient|>functions.{}", mapped);
            s = s.replace(&wrong, &fix);
        }

        // 2) Standalone wrong role tags like <|bash|>
        for name in tool_names.iter() {
            let wrong = format!("<|{}|>", name);
            let mapped = if *name == "container.exec" { "bash" } else { name };
            let fix = format!("<|assistant|><|recipient|>functions.{}", mapped);
            s = s.replace(&wrong, &fix);
        }
        for fname in function_names.iter() {
            let wrong = format!("<|{}|>", fname);
            let val = fname.strip_prefix("functions.").unwrap_or(fname);
            let mapped = if val == "container.exec" { "bash" } else { val };
            let fix = format!("<|assistant|><|recipient|>functions.{}", mapped);
            s = s.replace(&wrong, &fix);
        }

        // 3) Bad recipient payloads like <|recipient|>bash
        for name in tool_names.iter() {
            let wrong = format!("<|recipient|>{}", name);
            let mapped = if *name == "container.exec" { "bash" } else { name };
            let fix = format!("<|recipient|>functions.{}", mapped);
            s = s.replace(&wrong, &fix);
        }
        for fname in function_names.iter() {
            let wrong = format!("<|recipient|>{}", fname);
            let val = fname.strip_prefix("functions.").unwrap_or(fname);
            let mapped = if val == "container.exec" { "bash" } else { val };
            let fix = format!("<|recipient|>functions.{}", mapped);
            s = s.replace(&wrong, &fix);
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

#[cfg(test)]
mod tests {
    use super::GptClient;

    #[test]
    fn normalizes_bash_role_tag() {
        let s = "<|bash|> do things";
        let out = GptClient::normalize_completion(s);
        assert!(out.contains("<|assistant|><|recipient|>functions.bash"));
    }

    #[test]
    fn normalizes_assistant_then_bash_tag() {
        let s = "<|assistant|><|bash|> {\"command\":\"echo hi\"}";
        let out = GptClient::normalize_completion(s);
        assert!(out.contains("<|assistant|><|recipient|>functions.bash"));
    }

    #[test]
    fn normalizes_recipient_bash_value() {
        let s = "<|assistant|><|recipient|>bash<|message|>run";
        let out = GptClient::normalize_completion(s);
        assert!(out.contains("<|recipient|>functions.bash"));
    }

    #[test]
    fn maps_container_exec_to_bash() {
        let s = "<|assistant|><|recipient|>container.exec<|message|>run";
        let out = GptClient::normalize_completion(s);
        assert!(out.contains("<|recipient|>functions.bash"));
    }
}
