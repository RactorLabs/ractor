use super::config::Config;
use super::error::{HostError, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

// Import constants from shared module
#[path = "../shared/models/constants.rs"]
pub mod constants;
pub use constants::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Agent,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub agent_name: String, // Changed from agent_id in v0.4.0
    pub role: MessageRole,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateMessageRequest {
    pub role: MessageRole,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct CreateMessageRequestStructured {
    role: MessageRole,
    content: String,
    metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    content_json: Option<serde_json::Value>,
}

// Import constants from shared models


#[derive(Debug, Clone, Deserialize)]
pub struct Agent {
    pub name: String, // Primary key in v0.4.0
    pub created_by: String,
    pub state: String,
    pub parent_agent_name: Option<String>, // Changed from parent_agent_id
    pub created_at: String,
    pub last_activity_at: Option<String>,
    pub metadata: serde_json::Value,
    pub is_published: bool,
    pub published_at: Option<String>,
    pub published_by: Option<String>,
    pub publish_permissions: serde_json::Value,
    pub idle_timeout_seconds: i32,
    pub busy_timeout_seconds: i32,
    pub idle_from: Option<String>,
    pub busy_from: Option<String>,
    // Removed: id, container_id, persistent_volume_id (derived from name in v0.4.0)
}

pub struct RaworcClient {
    client: Client,
    config: Arc<Config>,
}

impl RaworcClient {
    pub fn new(config: Arc<Config>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, config }
    }

    /// Get agent information
    pub async fn get_agent(&self) -> Result<Agent> {
        let url = format!(
            "{}/api/v0/agents/{}",
            self.config.api_url, self.config.agent_name
        );

        debug!("Fetching agent info from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => {
                let agent = response.json::<Agent>().await?;
                debug!("Fetched agent info for: {}", agent.name);
                Ok(agent)
            }
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api(format!(
                "Agent {} not found",
                self.config.agent_name
            ))),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "API error ({}): {}",
                    status, error_text
                )))
            }
        }
    }


    /// Create a new response (user input)
    pub async fn create_response(&self, input_text: &str) -> Result<ResponseView> {
        let url = format!(
            "{}/api/v0/agents/{}/responses",
            self.config.api_url, self.config.agent_name
        );
        let req = CreateResponseRequest { input: serde_json::json!({ "text": input_text }) };
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&req)
            .send()
            .await?;
        match response.status() {
            StatusCode::OK | StatusCode::CREATED => Ok(response.json::<ResponseView>().await?),
            StatusCode::UNAUTHORIZED => Err(HostError::Api("Unauthorized - check API token".to_string())),
            StatusCode::NOT_FOUND => Err(HostError::Api("Agent not found".to_string())),
            status => {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!("Failed to create response ({}): {}", status, error_text)))
            }
        }
    }

    /// Update an existing response with output/status
    pub async fn update_response(
        &self,
        id: &str,
        status: Option<String>,
        output_text: Option<String>,
        items: Option<Vec<serde_json::Value>>,
    ) -> Result<ResponseView> {
        let url = format!(
            "{}/api/v0/agents/{}/responses/{}",
            self.config.api_url, self.config.agent_name, id
        );
        let mut output = serde_json::Map::new();
        if let Some(t) = output_text { output.insert("text".to_string(), serde_json::json!(t)); }
        if let Some(list) = items { output.insert("items".to_string(), serde_json::Value::Array(list)); }
        let req = UpdateResponseRequest {
            status,
            input: None,
            output: Some(serde_json::Value::Object(output)),
        };
        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&req)
            .send()
            .await?;
        match response.status() {
            StatusCode::OK => Ok(response.json::<ResponseView>().await?),
            StatusCode::UNAUTHORIZED => Err(HostError::Api("Unauthorized - check API token".to_string())),
            StatusCode::NOT_FOUND => Err(HostError::Api("Response not found".to_string())),
            status => {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!("Failed to update response ({}): {}", status, error_text)))
            }
        }
    }

    /// List responses for current agent
    pub async fn get_responses(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<ResponseView>> {
        let mut url = format!(
            "{}/api/v0/agents/{}/responses",
            self.config.api_url, self.config.agent_name
        );
        let mut sep = '?';
        if let Some(l) = limit { url.push_str(&format!("{}limit={}", sep, l)); sep = '&'; }
        if let Some(o) = offset { url.push_str(&format!("{}offset={}", sep, o)); }
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;
        match response.status() {
            StatusCode::OK => Ok(response.json::<Vec<ResponseView>>().await.map_err(|e| HostError::Api(e.to_string()))?),
            StatusCode::UNAUTHORIZED => Err(HostError::Api("Unauthorized - check API token".to_string())),
            StatusCode::NOT_FOUND => Err(HostError::Api("Agent not found".to_string())),
            status => {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!("Failed to fetch responses ({}): {}", status, error_text)))
            }
        }
    }

    /// Get response count for current agent
    pub async fn get_response_count(&self) -> Result<u64> {
        let url = format!(
            "{}/api/v0/agents/{}/responses/count",
            self.config.api_url, self.config.agent_name
        );
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;
        match response.status() {
            StatusCode::OK => {
                let v = response.json::<serde_json::Value>().await.map_err(|e| HostError::Api(e.to_string()))?;
                let count = v.get("count").and_then(|c| c.as_i64()).unwrap_or(0) as u64;
                Ok(count)
            }
            StatusCode::UNAUTHORIZED => Err(HostError::Api("Unauthorized - check API token".to_string())),
            StatusCode::NOT_FOUND => Err(HostError::Api("Agent not found".to_string())),
            status => {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!("Failed to get response count ({}): {}", status, error_text)))
            }
        }
    }

    

    /// Update agent to busy (clears idle_from)
    pub async fn update_agent_to_busy(&self) -> Result<()> {
        let url = format!(
            "{}/api/v0/agents/{}/busy",
            self.config.api_url, self.config.agent_name
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;

        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => {
                info!("Agent state updated to: busy (timeout paused)");
                Ok(())
            }
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api(format!(
                "Agent {} not found",
                self.config.agent_name
            ))),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to update to busy ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Update agent to idle (sets idle_from)
    pub async fn update_agent_to_idle(&self) -> Result<()> {
        let url = format!(
            "{}/api/v0/agents/{}/idle",
            self.config.api_url, self.config.agent_name
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;

        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => {
                info!("Agent state updated to: idle (timeout started)");
                Ok(())
            }
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api(format!(
                "Agent {} not found",
                self.config.agent_name
            ))),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to update to idle ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Publish the current agent by name
    pub async fn publish_agent(&self) -> Result<()> {
        let url = format!(
            "{}/api/v0/agents/{}/publish",
            self.config.api_url, self.config.agent_name
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&serde_json::json!({
                "code": true,
                "secrets": true,
                "content": true
            }))
            .send()
            .await?;

        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT | StatusCode::CREATED => Ok(()),
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api(format!(
                "Agent {} not found",
                self.config.agent_name
            ))),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to publish agent ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Sleep the current agent by name
    pub async fn sleep_agent(&self) -> Result<()> {
        let url = format!(
            "{}/api/v0/agents/{}/sleep",
            self.config.api_url, self.config.agent_name
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;

        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => Ok(()),
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api(format!(
                "Agent {} not found",
                self.config.agent_name
            ))),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to sleep agent ({}): {}",
                    status, error_text
                )))
            }
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseView {
    pub id: String,
    pub agent_name: String,
    pub status: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateResponseRequest {
    pub input: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct UpdateResponseRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
}
