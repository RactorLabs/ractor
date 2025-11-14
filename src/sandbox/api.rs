use super::config::Config;
use super::error::{HostError, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

// (Removed legacy message types and constants import; API now uses Tasks.)

#[derive(Debug, Clone, Deserialize)]
pub struct Sandbox {
    pub id: String, // UUID primary key
    pub created_by: String,
    pub state: String,
    pub snapshot_id: Option<String>,
    pub created_at: String,
    pub last_activity_at: Option<String>,
    pub metadata: serde_json::Value,
    pub idle_timeout_seconds: i32,
    pub idle_from: Option<String>,
    pub busy_from: Option<String>,
    #[serde(default)]
    pub inference_prompt_tokens: Option<i64>,
    #[serde(default)]
    pub inference_completion_tokens: Option<i64>,
    #[serde(default)]
    pub tool_usage: Option<serde_json::Value>,
    #[serde(default)]
    pub total_runtime_seconds: Option<i64>,
    #[serde(default)]
    pub current_runtime_seconds: Option<i64>,
    #[serde(default)]
    pub tasks_completed_total: Option<i64>,
}

pub struct TSBXClient {
    client: Client,
    config: Arc<Config>,
    sandbox_id: String,
}

impl TSBXClient {
    pub fn new(config: Arc<Config>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let sandbox_id = config.sandbox_id.clone();

        Self {
            client,
            config,
            sandbox_id,
        }
    }

    pub async fn update_task_usage(
        &self,
        id: &str,
        context_length: i64,
        prompt_tokens: i64,
        completion_tokens: i64,
    ) -> Result<()> {
        let clamped = context_length.max(0);
        let url = format!(
            "{}/api/v0/sandboxes/{}/tasks/{}",
            self.config.api_url, self.sandbox_id, id
        );
        let req = UpdateTaskRequest {
            status: None,
            input: None,
            output: None,
            steps: None,
            timeout_seconds: None,
            context_length: Some(clamped),
            prompt_tokens_delta: Some(prompt_tokens.max(0)),
            completion_tokens_delta: Some(completion_tokens.max(0)),
            tool_used: None,
        };
        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .json(&req)
            .send()
            .await?;
        match response.status() {
            StatusCode::OK => Ok(()),
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
                    "Failed to update task context length ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Get a task by id for current sandbox
    pub async fn get_task_by_id(&self, id: &str) -> Result<TaskView> {
        let url = format!(
            "{}/api/v0/sandboxes/{}/tasks/{}",
            self.config.api_url, self.sandbox_id, id
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

    /// Get sandbox information
    pub async fn get_sandbox(&self) -> Result<Sandbox> {
        let url = format!(
            "{}/api/v0/sandboxes/{}",
            self.config.api_url, self.sandbox_id
        );

        debug!("Fetching sandbox info from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => {
                let sandbox = response.json::<Sandbox>().await?;
                debug!("Fetched sandbox info for: {}", sandbox.id);
                Ok(sandbox)
            }
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api(format!(
                "Sandbox {} not found",
                self.sandbox_id
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

    pub async fn get_stats(&self) -> Result<SandboxStats> {
        let url = format!(
            "{}/api/v0/sandboxes/{}/stats",
            self.config.api_url, self.sandbox_id
        );
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;
        match response.status() {
            StatusCode::OK => Ok(response.json::<SandboxStats>().await?),
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api("Sandbox not found".to_string())),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(HostError::Api(format!(
                    "Failed to fetch sandbox stats ({}): {}",
                    status, error_text
                )))
            }
        }
    }

    /// Create a new task (user input)
    pub async fn create_task(&self, input_text: &str) -> Result<TaskView> {
        let url = format!(
            "{}/api/v0/sandboxes/{}/tasks",
            self.config.api_url, self.sandbox_id
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
            StatusCode::NOT_FOUND => Err(HostError::Api("Sandbox not found".to_string())),
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
        steps: Option<Vec<serde_json::Value>>,
        context_length: Option<i64>,
        tool_used: Option<String>,
    ) -> Result<TaskView> {
        let url = format!(
            "{}/api/v0/sandboxes/{}/tasks/{}",
            self.config.api_url, self.sandbox_id, id
        );
        let mut output = serde_json::Map::new();
        if let Some(t) = output_text {
            output.insert("text".to_string(), serde_json::json!(t));
        }
        let output_value = if output.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(output))
        };
        let req = UpdateTaskRequest {
            status,
            input: None,
            output: output_value,
            steps,
            timeout_seconds: None,
            context_length,
            prompt_tokens_delta: None,
            completion_tokens_delta: None,
            tool_used,
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

    /// List tasks for current sandbox
    pub async fn get_tasks(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<TaskSummary>> {
        let mut url = format!(
            "{}/api/v0/sandboxes/{}/tasks",
            self.config.api_url, self.sandbox_id
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
                .json::<Vec<TaskSummary>>()
                .await
                .map_err(|e| HostError::Api(e.to_string()))?),
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api("Sandbox not found".to_string())),
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

    /// Update sandbox to busy (clears idle_from)
    pub async fn update_sandbox_to_busy(&self) -> Result<()> {
        let url = format!(
            "{}/api/v0/sandboxes/{}/state/busy",
            self.config.api_url, self.sandbox_id
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;

        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => {
                info!("Sandbox state updated to: busy (timeout paused)");
                Ok(())
            }
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api(format!(
                "Sandbox {} not found",
                self.sandbox_id
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

    /// Update sandbox to idle (sets idle_from)
    pub async fn update_sandbox_to_idle(&self) -> Result<()> {
        let url = format!(
            "{}/api/v0/sandboxes/{}/state/idle",
            self.config.api_url, self.sandbox_id
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;

        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => {
                info!("Sandbox state updated to: idle (timeout started)");
                Ok(())
            }
            StatusCode::UNAUTHORIZED => {
                Err(HostError::Api("Unauthorized - check API token".to_string()))
            }
            StatusCode::NOT_FOUND => Err(HostError::Api(format!(
                "Sandbox {} not found",
                self.sandbox_id
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub id: String,
    pub sandbox_id: String,
    pub status: String,
    #[serde(default)]
    pub input_content: Vec<serde_json::Value>,
    pub context_length: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskView {
    pub id: String,
    pub sandbox_id: String,
    pub status: String,
    #[serde(default)]
    pub input_content: Vec<serde_json::Value>,
    #[serde(default)]
    pub output_content: Vec<serde_json::Value>,
    #[serde(default)]
    pub segments: Vec<serde_json::Value>,
    #[serde(default)]
    pub steps: Vec<serde_json::Value>,
    #[serde(default)]
    pub output: serde_json::Value,
    pub context_length: i64,
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
    pub steps: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_length: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_delta: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens_delta: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_used: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SandboxStats {
    pub sandbox_id: String,
    pub container_state: String,
    pub tasks_completed_total: i64,
    pub total_tasks: i64,
    pub cpu_usage_percent: f64,
    pub cpu_limit_cores: f64,
    pub memory_usage_bytes: u64,
    pub memory_limit_bytes: u64,
    pub inference_url: Option<String>,
    pub inference_model: Option<String>,
    pub inference_prompt_tokens: i64,
    pub inference_completion_tokens: i64,
    pub inference_total_tokens: i64,
    pub tool_usage: serde_json::Value,
    pub total_runtime_seconds: i64,
    pub current_runtime_seconds: i64,
    pub captured_at: String,
}
