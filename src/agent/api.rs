use super::config::Config;
use super::error::{HostError, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

// Import constants from shared module
#[path = "../shared/models/constants.rs"]
pub mod constants;

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
    pub author_name: Option<String>,
    pub recipient: Option<String>,
    pub channel: Option<String>,
    pub content: String,
    pub content_type: Option<String>,
    pub content_json: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateMessageRequest {
    pub role: MessageRole,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub author_name: Option<String>,
    pub recipient: Option<String>,
    pub channel: Option<String>,
    pub content_type: Option<String>,
    pub content_json: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct UpdateMessageRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_name: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_json: Option<Option<serde_json::Value>>,
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

    /// Get messages for the current agent
    pub async fn get_messages(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Message>> {
        let mut url = format!(
            "{}/api/v0/agents/{}/messages",
            self.config.api_url, self.config.agent_name
        );

        let mut params = vec![];
        if let Some(limit) = limit {
            params.push(format!("limit={}", limit));
        }
        if let Some(offset) = offset {
            params.push(format!("offset={}", offset));
        }

        if !params.is_empty() {
            url.push_str("?");
            url.push_str(&params.join("&"));
        }

        debug!("Fetching messages from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => {
                let messages = response.json::<Vec<Message>>().await?;
                debug!("Fetched {} messages", messages.len());
                Ok(messages)
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

    /// Get total message count for the current agent
    pub async fn get_message_count(&self) -> Result<u64> {
        let url = format!(
            "{}/api/v0/agents/{}/messages/count",
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
                let v = response
                    .json::<serde_json::Value>()
                    .await
                    .map_err(|e| HostError::Api(format!("Failed to parse count: {}", e)))?;
                let count = v
                    .get("count")
                    .and_then(|c| c.as_u64())
                    .ok_or_else(|| HostError::Api("Missing count field".to_string()))?;
                Ok(count)
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

    /// Send a message as the Agent
    pub async fn send_message(
        &self,
        content: String,
        metadata: Option<serde_json::Value>,
    ) -> Result<Message> {
        let url = format!(
            "{}/api/v0/agents/{}/messages",
            self.config.api_url, self.config.agent_name
        );

        let request = CreateMessageRequest {
            role: MessageRole::Agent,
            content,
            metadata,
            author_name: None,
            recipient: None,
            channel: None,
            content_type: None,
            content_json: None,
        };

        debug!("Sending message to: {}", url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&request)
            .send()
            .await?;

        match response.status() {
            StatusCode::OK | StatusCode::CREATED => {
                let message = response.json::<Message>().await?;
                info!("Message sent successfully: {}", message.id);
                Ok(message)
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
                    "Failed to send message ({}): {}",
                    status, error_text
                )))
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

    /// Send a structured message with Harmony fields
    pub async fn send_message_structured(
        &self,
        role: MessageRole,
        content: String,
        metadata: Option<serde_json::Value>,
        author_name: Option<String>,
        recipient: Option<String>,
        channel: Option<String>,
        content_type: Option<String>,
        content_json: Option<serde_json::Value>,
    ) -> Result<Message> {
        let url = format!(
            "{}/api/v0/agents/{}/messages",
            self.config.api_url, self.config.agent_name
        );

        let request = CreateMessageRequest {
            role,
            content,
            metadata,
            author_name,
            recipient,
            channel,
            content_type,
            content_json,
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&request)
            .send()
            .await?;

        match response.status() {
            StatusCode::OK | StatusCode::CREATED => {
                let message = response.json::<Message>().await?;
                info!("Message sent successfully: {}", message.id);
                Ok(message)
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
                    "Failed to send message ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Update an existing message by id (PATCH)
    pub async fn update_message(
        &self,
        message_id: &str,
        req: UpdateMessageRequest,
    ) -> Result<Message> {
        let url = format!(
            "{}/api/v0/agents/{}/messages/{}",
            self.config.api_url, self.config.agent_name, message_id
        );
        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&req)
            .send()
            .await?;
        match response.status() {
            StatusCode::OK => {
                let message = response.json::<Message>().await?;
                Ok(message)
            }
            status => {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!("Failed to update message ({}): {}", status, error_text)))
            }
        }
    }
}
