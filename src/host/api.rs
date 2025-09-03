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
    Host,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_name: String, // Changed from session_id in v0.4.0
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

// Import constants from shared models

#[derive(Debug, Serialize)]
pub struct UpdateSessionStateRequest {
    pub state: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Session {
    pub name: String, // Primary key in v0.4.0
    pub created_by: String,
    pub state: String,
    pub parent_session_name: Option<String>, // Changed from parent_session_id
    pub created_at: String,
    pub last_activity_at: Option<String>,
    pub metadata: serde_json::Value,
    pub is_published: bool,
    pub published_at: Option<String>,
    pub published_by: Option<String>,
    pub publish_permissions: serde_json::Value,
    pub timeout_seconds: i32,
    pub auto_close_at: Option<String>,
    pub content_port: Option<i32>,
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

    /// Get session information including content_port
    pub async fn get_session(&self) -> Result<Session> {
        let url = format!(
            "{}/api/v0/sessions/{}",
            self.config.api_url, self.config.session_name
        );

        debug!("Fetching session info from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => {
                let session = response.json::<Session>().await?;
                debug!("Fetched session info for: {}", session.name);
                Ok(session)
            }
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api(format!(
                "Session {} not found",
                self.config.session_name
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

    /// Get messages for the current session
    pub async fn get_messages(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Message>> {
        let mut url = format!(
            "{}/api/v0/sessions/{}/messages",
            self.config.api_url, self.config.session_name
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
                "Session {} not found",
                self.config.session_name
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

    /// Send a message as the Host
    pub async fn send_message(
        &self,
        content: String,
        metadata: Option<serde_json::Value>,
    ) -> Result<Message> {
        let url = format!(
            "{}/api/v0/sessions/{}/messages",
            self.config.api_url, self.config.session_name
        );

        let request = CreateMessageRequest {
            role: MessageRole::Host,
            content,
            metadata,
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
                "Session {} not found",
                self.config.session_name
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

    /// Update session state (generic)
    pub async fn update_session_state(&self, state: String) -> Result<()> {
        let url = format!(
            "{}/api/v0/sessions/{}/state",
            self.config.api_url, self.config.session_name
        );

        let request = UpdateSessionStateRequest {
            state: state.clone(),
        };

        debug!("Updating session state to: {:?}", state);

        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&request)
            .send()
            .await?;

        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => {
                info!("Session state updated to: {:?}", state);
                Ok(())
            }
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api(format!(
                "Session {} not found",
                self.config.session_name
            ))),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to update state ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Update session to busy (clears auto_close_at)
    pub async fn update_session_to_busy(&self) -> Result<()> {
        let url = format!(
            "{}/api/v0/sessions/{}/busy",
            self.config.api_url, self.config.session_name
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;

        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => {
                info!("Session state updated to: busy (timeout paused)");
                Ok(())
            }
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api(format!(
                "Session {} not found",
                self.config.session_name
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

    /// Update session to idle (sets auto_close_at)
    pub async fn update_session_to_idle(&self) -> Result<()> {
        let url = format!(
            "{}/api/v0/sessions/{}/idle",
            self.config.api_url, self.config.session_name
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;

        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => {
                info!("Session state updated to: idle (timeout started)");
                Ok(())
            }
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api(format!(
                "Session {} not found",
                self.config.session_name
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
}
