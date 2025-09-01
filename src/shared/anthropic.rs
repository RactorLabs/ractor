use anyhow::Result;
use serde::{Deserialize, Serialize};
use reqwest::Client;
use tracing::{info, error};

#[derive(Debug, Serialize)]
struct CreateKeyRequest {
    #[serde(rename = "type")]
    key_type: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct CreateKeyResponse {
    key: String,
    key_id: String,
}

pub struct AnthropicKeyManager {
    client: Client,
    api_key: String,
}

impl AnthropicKeyManager {
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY environment variable is required"))?;
        
        if api_key.is_empty() {
            return Err(anyhow::anyhow!("ANTHROPIC_API_KEY cannot be empty"));
        }

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            client,
            api_key,
        })
    }

    pub async fn generate_session_api_key(&self, session_id: &str) -> Result<String> {
        info!("Using shared ANTHROPIC_API_KEY for session: {}", session_id);
        
        // Since Anthropic does not support programmatic API key generation,
        // we use the same regular API key for all sessions. This key is:
        // 1. Set only as environment variable in each container
        // 2. Not persisted to disk
        // 3. Isolated per container
        // 4. Not accessible system-wide
        
        info!("Providing regular API key for session: {}", session_id);
        Ok(self.api_key.clone())
    }

    /// Test if the admin key can access the Messages API
    async fn test_messages_api_access(&self) -> Result<()> {
        let test_request = CreateKeyRequest {
            key_type: "test".to_string(),
            name: "test".to_string(),
        };

        // Try a simple Messages API call to validate key access
        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": "claude-3-5-sonnet-20241022",
                "max_tokens": 1,
                "messages": [{"role": "user", "content": "test"}]
            }))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow::anyhow!("Messages API test failed ({}): {}", status, error_text))
        }
    }

    // This method will be implemented once Anthropic provides API key generation endpoints
    #[allow(dead_code)]
    async fn create_api_key_via_api(&self, session_id: &str) -> Result<String> {
        let request = CreateKeyRequest {
            key_type: "session".to_string(),
            name: format!("raworc-session-{}", session_id),
        };

        let response = self.client
            .post("https://api.anthropic.com/v1/keys") // This endpoint doesn't exist yet
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create API key: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("API key creation failed ({}): {}", status, error_text));
        }

        let key_response: CreateKeyResponse = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse key creation response: {}", e))?;

        info!("Successfully generated API key with ID: {}", key_response.key_id);
        Ok(key_response.key)
    }
}