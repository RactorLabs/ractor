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

        // Valid tags that should not be rewritten when seen inside <|...|>
        const VALID_SIMPLE_TAGS: &[&str] = &[
            "assistant",
            "user",
            "system",
            "developer",
            "tool",
            "channel",
            "recipient",
            "message",
        ];

        // Map alias names to canonical tool function names
        fn map_alias(name: &str) -> String {
            let n = name.trim();
            let n = n.strip_prefix("functions.").unwrap_or(n);
            let n = n.strip_prefix("tools.").unwrap_or(n);
            let n = n.strip_prefix("tool.").unwrap_or(n);
            match n {
                "container.exec" => "bash".to_string(),
                other => other.to_string(),
            }
        }

        // 1) Fix patterns like <|assistant|><|bash|> to <|assistant|><|recipient|>functions.bash
        let mut fixed = String::with_capacity(s.len() + 32);
        let mut i = 0usize;
        let bytes = s.as_bytes();
        while i < bytes.len() {
            if bytes[i..].starts_with(b"<|assistant|><|") {
                let start = i + "<|assistant|><|".len();
                if let Some(end_rel) = s[start..].find("|>") {
                    let end = start + end_rel;
                    let token = &s[start..end];
                    if !VALID_SIMPLE_TAGS.contains(&token) {
                        let mapped = map_alias(token);
                        fixed.push_str("<|assistant|><|recipient|>functions.");
                        fixed.push_str(&mapped);
                        i = end + 2; // skip |>
                        continue;
                    }
                }
            }
            fixed.push(bytes[i] as char);
            i += 1;
        }
        s = fixed;

        // 2) Fix standalone wrong role tags like <|bash|> → <|assistant|><|recipient|>functions.bash
        let mut out = String::with_capacity(s.len() + 32);
        let mut i2 = 0usize;
        let b2 = s.as_bytes();
        while i2 < b2.len() {
            if b2[i2..].starts_with(b"<|") {
                let start = i2 + 2; // after <|
                if let Some(end_rel) = s[start..].find("|>") {
                    let end = start + end_rel;
                    let token = &s[start..end];
                    if !VALID_SIMPLE_TAGS.contains(&token) {
                        let mapped = map_alias(token);
                        out.push_str("<|assistant|><|recipient|>functions.");
                        out.push_str(&mapped);
                        i2 = end + 2;
                        continue;
                    }
                }
            }
            out.push(b2[i2] as char);
            i2 += 1;
        }
        s = out;

        // 3) Fix bad recipient payloads like <|recipient|>bash → <|recipient|>functions.bash
        let mut out3 = String::with_capacity(s.len() + 16);
        let mut j = 0usize;
        let bj = s.as_bytes();
        while j < bj.len() {
            if bj[j..].starts_with(b"<|recipient|>") {
                out3.push_str("<|recipient|>");
                j += "<|recipient|>".len();
                let start = j;
                while j < bj.len() {
                    let c = bj[j] as char;
                    if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                        j += 1;
                    } else {
                        break;
                    }
                }
                let token = &s[start..j];
                if !token.is_empty() {
                    let mapped = map_alias(token);
                    out3.push_str("functions.");
                    out3.push_str(&mapped);
                    continue;
                }
            }
            out3.push(bj[j] as char);
            j += 1;
        }
        s = out3;

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
