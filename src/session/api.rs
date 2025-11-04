use super::config::Config;
use super::error::{HostError, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

// (Removed legacy message types and constants import; API now uses Tasks.)

#[derive(Debug, Clone, Deserialize)]
pub struct Session {
    pub id: String, // UUID primary key
    pub name: String, // Unique name
    pub created_by: String,
    pub state: String,
    pub parent_session_id: Option<String>,
    pub created_at: String,
    pub last_activity_at: Option<String>,
    pub metadata: serde_json::Value,
    pub stop_timeout_seconds: i32,
    pub archive_timeout_seconds: i32,
    pub idle_from: Option<String>,
    pub busy_from: Option<String>,
    pub context_cutoff_at: Option<String>,
    pub last_context_length: i64,
}

pub struct TaskSandboxClient {
    client: Client,
    config: Arc<Config>,
    session_id: String,
}

impl TaskSandboxClient {
    pub fn new(config: Arc<Config>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let session_id = config.session_id.clone();

        Self { client, config, session_id }
    }

    // Expose session name for prompts/logging
    pub fn session_name(&self) -> &str {
        &self.config.session_name
    }

    /// Get a task by id for current session
    pub async fn get_task_by_id(&self, id: &str) -> Result<TaskView> {
        let url = format!(
            "{}/api/v0/sessions/{}/tasks/{}",
            self.config.api_url, self.session_id, id
        );
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;
        match response.status() {
            StatusCode::OK => Ok(response.json::<TaskView>().await?),
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api("Task not found".to_string())),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to get task ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Get session information
    pub async fn get_session(&self) -> Result<Session> {
        let url = format!(
            "{}/api/v0/sessions/{}",
            self.config.api_url, self.session_id
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

    /// Create a new task (user input)
    pub async fn create_task(&self, input_text: &str) -> Result<TaskView> {
        let url = format!(
            "{}/api/v0/sessions/{}/tasks",
            self.config.api_url, self.session_id
        );
        let req = CreateTaskRequest {
            input: serde_json::json!({ "content": [{"type":"text","content": input_text}] }),
            background: None,
            timeout_seconds: None,
        };
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&req)
            .send()
            .await?;
        match response.status() {
            StatusCode::OK | StatusCode::CREATED => Ok(response.json::<TaskView>().await?),
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
                    "Failed to create task ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Update an existing task with output/status
    pub async fn update_task(
        &self,
        id: &str,
        status: Option<String>,
        output_text: Option<String>,
        items: Option<Vec<serde_json::Value>>,
    ) -> Result<TaskView> {
        let url = format!(
            "{}/api/v0/sessions/{}/tasks/{}",
            self.config.api_url, self.session_id, id
        );
        let mut output = serde_json::Map::new();
        if let Some(t) = output_text {
            output.insert("text".to_string(), serde_json::json!(t));
        }
        if let Some(list) = items {
            output.insert("items".to_string(), serde_json::Value::Array(list));
        }
        let req = UpdateTaskRequest {
            status,
            input: None,
            output: Some(serde_json::Value::Object(output)),
            timeout_seconds: None,
        };
        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&req)
            .send()
            .await?;
        match response.status() {
            StatusCode::OK => Ok(response.json::<TaskView>().await?),
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api("Task not found".to_string())),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to update task ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// List tasks for current session
    pub async fn get_tasks(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<TaskView>> {
        let mut url = format!(
            "{}/api/v0/sessions/{}/tasks",
            self.config.api_url, self.session_id
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
                .json::<Vec<TaskView>>()
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
                    "Failed to fetch tasks ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Get task count for current session
    pub async fn get_task_count(&self) -> Result<u64> {
        let url = format!(
            "{}/api/v0/sessions/{}/tasks/count",
            self.config.api_url, self.session_id
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
                    "Failed to get task count ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Update session to busy (clears idle_from)
    pub async fn update_session_to_busy(&self) -> Result<()> {
        let url = format!(
            "{}/api/v0/sessions/{}/busy",
            self.config.api_url, self.session_id
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
            self.config.api_url, self.session_id
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
            self.config.api_url, self.session_id
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


    /// Stop the current session by ID after an optional delay (seconds, min 5) with optional note
    pub async fn stop_session(
        &self,
        delay_seconds: Option<u64>,
        note: Option<String>,
    ) -> Result<()> {
        let url = format!(
            "{}/api/v0/sessions/{}/stop",
            self.config.api_url, self.session_id
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
                    "Failed to stop session ({}): {}",
                    status, error_text
                )))
            }
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskView {
    pub id: String,
    pub session_name: String,
    pub status: String,
    #[serde(default)]
    pub input_content: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub output_content: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub segments: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub timeout_seconds: Option<i32>,
    #[serde(default)]
    pub timeout_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateTaskRequest {
    pub input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct UpdateTaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<i32>,
}
