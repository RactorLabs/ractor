use super::config::Config;
use super::error::{HostError, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

// (Removed legacy message types and constants import; API now uses Responses.)

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
    pub idle_timeout_seconds: i32,
    pub busy_timeout_seconds: i32,
    pub idle_from: Option<String>,
    pub busy_from: Option<String>,
    pub context_cutoff_at: Option<String>,
    pub last_context_length: i64,
    // Removed: id, container_id, persistent_volume_id (derived from name in v0.4.0)
}

pub struct RactorClient {
    client: Client,
    config: Arc<Config>,
}

impl RactorClient {
    pub fn new(config: Arc<Config>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, config }
    }

    // Expose session name for prompts/logging
    pub fn session_name(&self) -> &str {
        &self.config.session_name
    }

    /// Get a response by id for current session
    pub async fn get_response_by_id(&self, id: &str) -> Result<ResponseView> {
        let url = format!(
            "{}/api/v0/sessions/{}/responses/{}",
            self.config.api_url, self.config.session_name, id
        );
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;
        match response.status() {
            StatusCode::OK => Ok(response.json::<ResponseView>().await?),
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api("Response not found".to_string())),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to get response ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Get session information
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

    /// Create a new response (user input)
    pub async fn create_response(&self, input_text: &str) -> Result<ResponseView> {
        let url = format!(
            "{}/api/v0/sessions/{}/responses",
            self.config.api_url, self.config.session_name
        );
        let req = CreateResponseRequest {
            input: serde_json::json!({ "content": [{"type":"text","content": input_text}] }),
            background: None,
        };
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&req)
            .send()
            .await?;
        match response.status() {
            StatusCode::OK | StatusCode::CREATED => Ok(response.json::<ResponseView>().await?),
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api("Session not found".to_string())),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to create response ({}): {}",
                    status, error_text
                )))
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
            "{}/api/v0/sessions/{}/responses/{}",
            self.config.api_url, self.config.session_name, id
        );
        let mut output = serde_json::Map::new();
        if let Some(t) = output_text {
            output.insert("text".to_string(), serde_json::json!(t));
        }
        if let Some(list) = items {
            output.insert("items".to_string(), serde_json::Value::Array(list));
        }
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
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api("Response not found".to_string())),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to update response ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// List responses for current session
    pub async fn get_responses(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<ResponseView>> {
        let mut url = format!(
            "{}/api/v0/sessions/{}/responses",
            self.config.api_url, self.config.session_name
        );
        let mut sep = '?';
        if let Some(l) = limit {
            url.push_str(&format!("{}limit={}", sep, l));
            sep = '&';
        }
        if let Some(o) = offset {
            url.push_str(&format!("{}offset={}", sep, o));
        }
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;
        match response.status() {
            StatusCode::OK => Ok(response
                .json::<Vec<ResponseView>>()
                .await
                .map_err(|e| HostError::Api(e.to_string()))?),
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api("Session not found".to_string())),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to fetch responses ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Get response count for current session
    pub async fn get_response_count(&self) -> Result<u64> {
        let url = format!(
            "{}/api/v0/sessions/{}/responses/count",
            self.config.api_url, self.config.session_name
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
                    .map_err(|e| HostError::Api(e.to_string()))?;
                let count = v.get("count").and_then(|c| c.as_i64()).unwrap_or(0) as u64;
                Ok(count)
            }
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api("Session not found".to_string())),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to get response count ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Update session to busy (clears idle_from)
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

    /// Update session to idle (sets idle_from)
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

    pub async fn update_session_context_length(&self, tokens: i64) -> Result<()> {
        #[derive(Serialize)]
        struct ContextUsageReq {
            tokens: i64,
        }

        let url = format!(
            "{}/api/v0/sessions/{}/context/usage",
            self.config.api_url, self.config.session_name
        );

        let body = ContextUsageReq {
            tokens: tokens.max(0),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&body)
            .send()
            .await?;

        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => Ok(()),
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api("Session not found".to_string())),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to update context usage ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Publish the current session by name
    pub async fn publish_session(&self) -> Result<()> {
        let url = format!(
            "{}/api/v0/sessions/{}/publish",
            self.config.api_url, self.config.session_name
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&serde_json::json!({
                "code": true,
                "env": true,
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
                "Session {} not found",
                self.config.session_name
            ))),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to publish session ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Sleep the current session by name after an optional delay (seconds, min 5) with optional note
    pub async fn sleep_session(
        &self,
        delay_seconds: Option<u64>,
        note: Option<String>,
    ) -> Result<()> {
        let url = format!(
            "{}/api/v0/sessions/{}/sleep",
            self.config.api_url, self.config.session_name
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&{
                let mut body = serde_json::json!({ "delay_seconds": delay_seconds.unwrap_or(5) });
                if let Some(n) = note {
                    let t = n.trim().to_string();
                    if !t.is_empty() {
                        body["note"] = serde_json::json!(t);
                    }
                }
                body
            })
            .send()
            .await?;

        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT | StatusCode::CREATED => Ok(()),
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
                    "Failed to sleep session ({}): {}",
                    status, error_text
                )))
            }
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseView {
    pub id: String,
    pub session_name: String,
    pub status: String,
    #[serde(default)]
    pub input_content: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub output_content: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub segments: Option<Vec<serde_json::Value>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateResponseRequest {
    pub input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
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
