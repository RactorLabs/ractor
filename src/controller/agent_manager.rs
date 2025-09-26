use anyhow::Result;
use bollard::Docker;
use chrono::{DateTime, Utc, TimeZone};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sqlx::{mysql::MySqlPoolOptions, MySql, Pool};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{error, info, warn};

// Import constants from shared module
#[path = "../shared/models/constants.rs"]
pub mod constants;
pub use constants::AGENT_STATE_INIT;

// Using local Ollama via OLLAMA_HOST

#[path = "../shared/rbac.rs"]
pub mod rbac;
use rbac::{RbacClaims, SubjectType};

use super::docker_manager::DockerManager;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentTask {
    id: String,
    task_type: String,
    agent_name: String,
    created_by: String,
    payload: serde_json::Value,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    error: Option<String>,
}

pub struct AgentManager {
    pool: Pool<MySql>,
    docker_manager: DockerManager,
    jwt_secret: String,
}

impl AgentManager {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        let docker = Docker::connect_with_socket_defaults()?;
        let docker_manager = DockerManager::new(docker, pool.clone());

        let jwt_secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "default-secret-change-in-production".to_string());

        Ok(Self {
            pool,
            docker_manager,
            jwt_secret,
        })
    }

    pub async fn run(&self) -> Result<()> {
        info!(
            "Agent Manager started, polling for tasks, auto-sleep monitoring, and health checks..."
        );

        // Run frequent task polling; run heavier maintenance on a slower cadence
        let mut last_auto_sleep = Instant::now() - Duration::from_secs(60);
        let mut last_health = Instant::now() - Duration::from_secs(60);
        loop {
            // Process pending tasks (fast path)
            let tasks_processed = match self.process_pending_tasks().await {
                Ok(processed) => processed,
                Err(e) => {
                    error!("Error processing tasks: {}", e);
                    0
                }
            };

            // Process auto-sleep every 10s
            let mut agents_slept = 0;
            if last_auto_sleep.elapsed() >= Duration::from_secs(10) {
                agents_slept = match self.process_auto_sleep().await {
                    Ok(slept) => slept,
                    Err(e) => {
                        error!("Error processing auto-sleep: {}", e);
                        0
                    }
                };
                last_auto_sleep = Instant::now();
            }

            // Check health every 10s
            let mut agents_recovered = 0;
            if last_health.elapsed() >= Duration::from_secs(10) {
                agents_recovered = match self.check_agent_health().await {
                    Ok(recovered) => recovered,
                    Err(e) => {
                        error!("Error checking agent health: {}", e);
                        0
                    }
                };
                last_health = Instant::now();
            }

            // If no work was done, short sleep before next poll (improves responsiveness)
            if tasks_processed == 0 && agents_slept == 0 && agents_recovered == 0 {
                sleep(Duration::from_millis(250)).await;
            }
        }
    }

    /// Ensure the agent container is running and healthy; wake if needed and wait up to timeout_secs
    pub async fn ensure_agent_running(&self, agent_name: &str, timeout_secs: u64) -> Result<()> {
        // Quick healthy check
        match self.docker_manager.is_container_healthy(agent_name).await {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => {
                tracing::warn!("health check error for {}: {}", agent_name, e);
            }
        }

        // If DB says slept or container absent, wake
        if let Some((state,)) = sqlx::query_as::<_, (String,)>(
            r#"SELECT state FROM agents WHERE name = ?"#,
        )
        .bind(agent_name)
        .fetch_optional(&self.pool)
        .await? {
            if state.to_lowercase() == "slept" {
                tracing::info!("Agent {} is slept; waking container", agent_name);
                let _ = self.docker_manager.wake_container(agent_name).await?;
            }
        } else {
            // No row; nothing we can do
            tracing::warn!("Agent {} not found in DB during ensure_agent_running", agent_name);
        }

        // Wait for healthy
        let mut waited = 0u64;
        let step = 500u64; // ms
        while waited / 1000 < timeout_secs {
            if let Ok(true) = self.docker_manager.is_container_healthy(agent_name).await {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(step)).await;
            waited += step;
        }
        Err(anyhow::anyhow!("agent {} not ready in {}s", agent_name, timeout_secs))
    }

    /// Proxy exec with stdout/stderr collection
    pub async fn exec_collect(
        &self,
        agent_name: &str,
        cmd: Vec<String>,
    ) -> Result<(i32, Vec<u8>, Vec<u8>)> {
        self.docker_manager.exec_collect(agent_name, cmd).await
    }

    // No external API key required for local Ollama

    /// Process agents that need auto-closing due to timeout
    async fn process_auto_sleep(&self) -> Result<usize> {
        // Ensure all idle agents have idle_from set
        let _ = sqlx::query(
            r#"
            UPDATE agents
            SET idle_from = NOW()
            WHERE state = 'idle' AND idle_from IS NULL
            "#,
        )
        .execute(&self.pool)
        .await;

        // Ensure all busy agents have busy_from set
        let _ = sqlx::query(
            r#"
            UPDATE agents
            SET busy_from = NOW()
            WHERE state = 'busy' AND busy_from IS NULL
            "#,
        )
        .execute(&self.pool)
        .await;

        // Find agents that need auto-sleep due to idle timeout
        let agents_to_close: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT name
            FROM agents
            WHERE (state = 'idle' AND idle_from IS NOT NULL AND TIMESTAMPADD(SECOND, idle_timeout_seconds, idle_from) <= NOW())
               OR (state = 'busy' AND busy_from IS NOT NULL AND TIMESTAMPADD(SECOND, busy_timeout_seconds, busy_from) <= NOW())
            ORDER BY
              CASE WHEN state = 'idle' THEN TIMESTAMPADD(SECOND, idle_timeout_seconds, idle_from)
                   ELSE TIMESTAMPADD(SECOND, busy_timeout_seconds, busy_from)
              END ASC
            LIMIT 50
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to find agents to auto-sleep: {}", e))?;

        let mut slept_count = 0;

        for (agent_name,) in agents_to_close {
            info!("Auto-sleeping agent {} due to timeout", agent_name);

            // Create sleep task for the agent
            let task_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(r#"
                INSERT INTO agent_tasks (id, agent_name, task_type, created_by, payload, status)
                VALUES (?, ?, 'sleep_agent', 'system', '{"reason": "auto_sleep_timeout"}', 'pending')
                "#)
            .bind(&task_id)
            .bind(&agent_name)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create auto-sleep task for agent {}: {}", agent_name, e))?;

            info!(
                "Created auto-sleep task {} for agent {}",
                task_id, agent_name
            );
            slept_count += 1;
        }

        if slept_count > 0 {
            info!("Scheduled {} agents for auto-sleep", slept_count);
        }

        Ok(slept_count)
    }

    /// Generate a agent-specific RAWORC token for the given principal
    fn generate_agent_token(
        &self,
        principal: &str,
        principal_type: SubjectType,
        agent_name: &str,
    ) -> Result<String> {
        let exp = chrono::Utc::now() + chrono::Duration::hours(24);
        let claims = RbacClaims {
            sub: principal.to_string(), // Use original principal name for API server compatibility
            sub_type: principal_type,
            exp: exp.timestamp() as usize,
            iat: chrono::Utc::now().timestamp() as usize,
            iss: "raworc-agent-manager".to_string(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_ref()),
        )
        .map_err(|e| anyhow::anyhow!("Failed to generate agent token: {}", e))?;

        info!(
            "Generated agent token for principal: {} (agent: {})",
            principal, agent_name
        );
        Ok(token)
    }

    async fn process_pending_tasks(&self) -> Result<usize> {
        let tasks = self.fetch_pending_tasks().await?;
        let mut processed = 0;

        for task in tasks {
            match self.process_task(task).await {
                Ok(_) => processed += 1,
                Err(e) => error!("Failed to process task: {}", e),
            }
        }

        Ok(processed)
    }

    async fn fetch_pending_tasks(&self) -> Result<Vec<AgentTask>> {
        // MySQL doesn't support RETURNING, so we need to do this in two steps
        // First, get and lock the pending tasks
        let task_ids: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT id
            FROM agent_tasks
            WHERE status = 'pending'
            ORDER BY created_at
            LIMIT 5
            FOR UPDATE
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        if task_ids.is_empty() {
            return Ok(vec![]);
        }

        // Update the tasks
        let ids: Vec<String> = task_ids.into_iter().map(|(id,)| id).collect();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "UPDATE agent_tasks SET status = 'processing', started_at = NOW(), updated_at = NOW() WHERE id IN ({placeholders})"
        );

        let mut query = sqlx::query(&query_str);
        for id in &ids {
            query = query.bind(id);
        }
        query.execute(&self.pool).await?;

        // Fetch the updated tasks
        let query_str = format!("SELECT * FROM agent_tasks WHERE id IN ({placeholders})");
        let mut query = sqlx::query_as::<_, AgentTask>(&query_str);
        for id in &ids {
            query = query.bind(id);
        }
        let tasks = query.fetch_all(&self.pool).await?;

        Ok(tasks)
    }

    async fn process_task(&self, task: AgentTask) -> Result<()> {
        info!("Processing task {} of type {}", task.id, task.task_type);

        let result = match task.task_type.as_str() {
            "create_agent" => self.handle_create_agent(task.clone()).await,
            "destroy_agent" => self.handle_destroy_agent(task.clone()).await,
            "execute_command" => self.handle_execute_command(task.clone()).await,
            "sleep_agent" => self.handle_sleep_agent(task.clone()).await,
            "wake_agent" => self.handle_wake_agent(task.clone()).await,
            "publish_agent" => self.handle_publish_agent(task.clone()).await,
            "unpublish_agent" => self.handle_unpublish_agent(task.clone()).await,
            "create_response" => self.handle_create_response(task.clone()).await,
            "file_read" => self.handle_file_read(task.clone()).await,
            "file_metadata" => self.handle_file_metadata(task.clone()).await,
            "file_list" => self.handle_file_list(task.clone()).await,
            "file_delete" => self.handle_file_delete(task.clone()).await,
            _ => {
                warn!("Unknown task type: {}", task.task_type);
                Err(anyhow::anyhow!("Unknown task type"))
            }
        };

        match result {
            Ok(_) => {
                self.mark_task_completed(&task.id).await?;
                info!("Task {} completed successfully", task.id);
            }
            Err(e) => {
                self.mark_task_failed(&task.id, &e.to_string()).await?;
                error!("Task {} failed: {}", task.id, e);
            }
        }

        Ok(())
    }

    pub async fn handle_create_agent(&self, task: AgentTask) -> Result<()> {
        let agent_name = task.agent_name.clone();

        // Parse the payload to get agent creation parameters
        let secrets = task
            .payload
            .get("secrets")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                    .collect::<std::collections::HashMap<String, String>>()
            })
            .unwrap_or_default();

        let instructions = task
            .payload
            .get("instructions")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let setup = task
            .payload
            .get("setup")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let prompt = task
            .payload
            .get("prompt")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Extract principal information for logging and token generation
        let principal = task
            .payload
            .get("principal")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let principal_type_str = task
            .payload
            .get("principal_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // Parse principal type for token generation
        let principal_type = match principal_type_str {
            "Admin" => SubjectType::Admin,
            "User" => SubjectType::Subject,
            _ => SubjectType::Subject,
        };

        // Generate dynamic token for this agent (for Raworc auth)
        info!("Generating dynamic token for agent {}", agent_name);
        let agent_token = self
            .generate_agent_token(principal, principal_type, &agent_name)
            .map_err(|e| anyhow::anyhow!("Failed to generate agent token: {}", e))?;

        info!(
            "Generated dynamic tokens for agent {} (principal: {})",
            agent_name, principal
        );

        info!("Creating agent {} for principal {} ({:?}) with {} secrets, instructions: {}, setup: {}, prompt: {}", 
              agent_name, principal, principal_type, secrets.len(), instructions.is_some(), setup.is_some(), prompt.is_some());

        // Check if this is a remix agent from task payload
        let is_remix = task
            .payload
            .get("remix")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // For remix agents, extract prompt from task payload
        let remix_prompt = if is_remix {
            task.payload
                .get("prompt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        if is_remix {
            let parent_agent_name = task
                .payload
                .get("parent_agent_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing parent_agent_name for remix"))?;

            let copy_data = task
                .payload
                .get("copy_data")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let copy_code = task
                .payload
                .get("copy_code")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let copy_secrets = task
                .payload
                .get("copy_secrets")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let copy_content = task
                .payload
                .get("copy_content")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            // For remix agents, get principal info from remix task payload
            let remix_principal = task
                .payload
                .get("principal")
                .and_then(|v| v.as_str())
                .unwrap_or(principal);
            let remix_principal_type_str = task
                .payload
                .get("principal_type")
                .and_then(|v| v.as_str())
                .unwrap_or(principal_type_str);

            info!(
                "DEBUG: Remix task payload principal: {:?}, principal_type: {:?}",
                task.payload.get("principal"),
                task.payload.get("principal_type")
            );
            info!(
                "DEBUG: Using remix_principal: {}, remix_principal_type_str: {}",
                remix_principal, remix_principal_type_str
            );
            let remix_principal_type = match remix_principal_type_str {
                "Admin" => SubjectType::Admin,
                "User" => SubjectType::Subject,
                _ => SubjectType::Subject,
            };

            info!("Creating remix agent {} from parent {} (copy_data: {}, copy_code: {}, copy_secrets: {}, copy_content: {}) for principal {} ({})", 
                  agent_name, parent_agent_name, copy_data, copy_code, copy_secrets, copy_content, remix_principal, remix_principal_type_str);

            // For remix agents, create container with selective volume copy from parent
            // Generate fresh token for remix agent
            let remix_token = self
                .generate_agent_token(remix_principal, remix_principal_type, &agent_name)
                .map_err(|e| anyhow::anyhow!("Failed to generate remix agent token: {}", e))?;

            self.docker_manager
                .create_container_with_selective_copy_and_tokens(
                    &agent_name,
                    parent_agent_name,
                    copy_data,
                    copy_code,
                    copy_secrets,
                    copy_content,
                    remix_token,
                    remix_principal.to_string(),
                    remix_principal_type_str.to_string(),
                    task.created_at,
                )
                .await?;
        } else {
            info!("Creating new agent {}", agent_name);

            // For regular agents, create container with agent parameters and generated tokens
            self.docker_manager
                .create_container_with_params_and_tokens(
                    &agent_name,
                    secrets,
                    instructions,
                    setup,
                    agent_token,
                    principal.to_string(),
                    principal_type_str.to_string(),
                    task.created_at,
                )
                .await?;
        }

        // Send prompt if provided (BEFORE setting state to IDLE)
        let prompt_to_send = prompt.or(remix_prompt);
        if let Some(prompt) = prompt_to_send {
            info!("Sending prompt to agent {}: {}", agent_name, prompt);

            // Create response record in database (pending)
            let response_id = uuid::Uuid::new_v4().to_string();
            let input_json =
                serde_json::json!({ "content": [ { "type": "text", "content": prompt } ] });
            let output_json = serde_json::json!({ "items": [] });
            sqlx::query(
                r#"
                INSERT INTO agent_responses (id, agent_name, created_by, status, input, output, created_at, updated_at)
                VALUES (?, ?, ?, 'pending', ?, ?, NOW(), NOW())
                "#,
            )
            .bind(&response_id)
            .bind(&agent_name)
            .bind(&principal)
            .bind(&input_json)
            .bind(&output_json)
            .execute(&self.pool)
            .await?;
            info!(
                "Prompt response {} created for agent {}",
                response_id, agent_name
            );
        }

        // Set agent state to INIT after container creation only if it hasn't changed yet.
        // This avoids overwriting an agent that already set itself to IDLE.
        sqlx::query(r#"UPDATE agents SET state = ?, last_activity_at = NOW() WHERE name = ? AND state = 'init'"#)
            .bind(AGENT_STATE_INIT)
            .bind(&agent_name)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn handle_destroy_agent(&self, task: AgentTask) -> Result<()> {
        let agent_name = task.agent_name;

        info!("Deleting container and volume for agent {}", agent_name);
        self.docker_manager.delete_container(&agent_name).await?;

        // No need to update agent state - DELETE endpoint performs hard delete of agent row

        Ok(())
    }

    pub async fn handle_execute_command(&self, task: AgentTask) -> Result<()> {
        let agent_name = task.agent_name;
        let command = task.payload["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing command in payload"))?;

        info!("Executing command in agent {}: {}", agent_name, command);
        let output = self
            .docker_manager
            .execute_command(&agent_name, command)
            .await?;

        sqlx::query(
            r#"
            INSERT INTO command_results (id, agent_name, command, output, created_at)
            VALUES (?, ?, ?, ?, NOW())
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(agent_name)
        .bind(command)
        .bind(output)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn mark_task_completed(&self, task_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE agent_tasks
            SET status = 'completed',
                completed_at = NOW(),
                updated_at = NOW()
            WHERE id = ?
            "#,
        )
        .bind(task_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn mark_task_failed(&self, task_id: &str, error: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE agent_tasks
            SET status = 'failed',
                error = ?,
                completed_at = NOW(),
                updated_at = NOW()
            WHERE id = ?
            "#,
        )
        .bind(task_id)
        .bind(error)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn handle_sleep_agent(&self, task: AgentTask) -> Result<()> {
        let agent_name = task.agent_name;
        // Optional delay before sleeping (in seconds), minimum 5 seconds
        let delay_secs = task
            .payload
            .get("delay_seconds")
            .and_then(|v| v.as_u64())
            .map(|d| if d < 5 { 5 } else { d })
            .unwrap_or(5);
        if delay_secs > 0 {
            info!(
                "Delaying sleep for agent {} by {} seconds",
                agent_name, delay_secs
            );
            sleep(Duration::from_secs(delay_secs)).await;
        }
        // Capture prior state and created_at for runtime measurement
        let agent_row_opt: Option<(chrono::DateTime<Utc>, String)> =
            sqlx::query_as(r#"SELECT created_at, state FROM agents WHERE name = ?"#)
                .bind(&agent_name)
                .fetch_optional(&self.pool)
                .await?;
        let (agent_created_at, prior_state) = agent_row_opt
            .map(|(c, s)| (c, s))
            .unwrap_or((chrono::Utc::now(), String::new()));

        info!("Sleeping container for agent {}", agent_name);

        // Sleep the Docker container but keep the persistent volume
        self.docker_manager.sleep_container(&agent_name).await?;

        // Update agent state to slept
        sqlx::query(r#"UPDATE agents SET state = 'slept' WHERE name = ?"#)
            .bind(&agent_name)
            .execute(&self.pool)
            .await?;

        info!("Agent {} state updated to slept", agent_name);
        // Create a chat marker response to indicate the agent has slept
        let response_id = uuid::Uuid::new_v4().to_string();
        let created_by = task.created_by.clone();
        let now_text = chrono::Utc::now().to_rfc3339();
        // Determine note: auto timeout vs user-triggered
        let auto =
            task.payload.get("reason").and_then(|v| v.as_str()) == Some("auto_sleep_timeout");
        let reason = if auto {
            if prior_state.to_lowercase() == "busy" {
                "busy_timeout"
            } else {
                "idle_timeout"
            }
        } else {
            "user"
        };
        let note = if auto {
            if reason == "busy_timeout" {
                "Busy timeout"
            } else {
                "Idle timeout"
            }
        } else {
            task.payload
                .get("note")
                .and_then(|v| v.as_str())
                .unwrap_or("User requested sleep")
        };

        // Mark the latest in-progress response as cancelled (processing or pending) (applies to any sleep reason)
        if let Some((resp_id, output_json)) = sqlx::query_as::<_, (String, serde_json::Value)>(
            r#"SELECT id, output FROM agent_responses WHERE agent_name = ? AND status IN ('processing','pending') ORDER BY created_at DESC LIMIT 1"#,
        )
        .bind(&agent_name)
        .fetch_optional(&self.pool)
        .await
        .unwrap_or(None)
        {
            let mut new_output = output_json.clone();
            // Append a cancelled marker item to the output
            let mut items = new_output
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_else(Vec::new);
            items.push(serde_json::json!({
                "type": "cancelled",
                "reason": reason,
                "at": now_text,
            }));
            if let serde_json::Value::Object(ref mut map) = new_output {
                map.insert("items".to_string(), serde_json::Value::Array(items));
            } else {
                new_output = serde_json::json!({"text":"","items": [ {"type":"cancelled","reason": reason, "at": now_text } ]});
            }
            // Update response status to 'cancelled'
            let _ = sqlx::query(
                r#"UPDATE agent_responses SET status = 'cancelled', output = ?, updated_at = NOW() WHERE id = ?"#,
            )
            .bind(&new_output)
            .bind(&resp_id)
            .execute(&self.pool)
            .await;
        } else {
            // If no response row exists yet (pre-insert race), try to find the latest create_response task and insert a cancelled response
            if let Some((task_id, created_by, payload)) = sqlx::query_as::<_, (String, String, serde_json::Value)>(
                r#"SELECT id, created_by, payload FROM agent_tasks WHERE agent_name = ? AND task_type = 'create_response' AND status IN ('pending','processing') ORDER BY created_at DESC LIMIT 1"#
            )
            .bind(&agent_name)
            .fetch_optional(&self.pool)
            .await
            .unwrap_or(None)
            {
                if let Some(resp_id) = payload.get("response_id").and_then(|v| v.as_str()) {
                    let input = payload.get("input").cloned().unwrap_or_else(|| serde_json::json!({"text":""}));
                    let cancelled_item = serde_json::json!({"type":"cancelled","reason": reason, "at": now_text});
                    let output = serde_json::json!({"text":"","items":[cancelled_item]});
                    let _ = sqlx::query(
                        r#"INSERT INTO agent_responses (id, agent_name, created_by, status, input, output, created_at, updated_at)
                            VALUES (?, ?, ?, 'cancelled', ?, ?, NOW(), NOW())
                            ON DUPLICATE KEY UPDATE status='cancelled', output=VALUES(output), updated_at=NOW()"#
                    )
                    .bind(resp_id)
                    .bind(&agent_name)
                    .bind(&created_by)
                    .bind(&input)
                    .bind(&output)
                    .execute(&self.pool)
                    .await;
                    let _ = sqlx::query(r#"UPDATE agent_tasks SET status='completed', updated_at=NOW(), completed_at=NOW(), error='cancelled' WHERE id = ?"#)
                        .bind(&task_id)
                        .execute(&self.pool)
                        .await;
                }
            }
        }
        // Determine runtime: time from last wake marker (or agent.created_at if none)
        let recent_rows: Vec<(chrono::DateTime<Utc>, serde_json::Value)> = sqlx::query_as(
            r#"SELECT created_at, output FROM agent_responses WHERE agent_name = ? ORDER BY created_at DESC LIMIT 50"#
        )
        .bind(&agent_name)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();
        let mut start_ts = agent_created_at;
        for (row_created_at, output) in recent_rows {
            if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
                let mut found = false;
                for it in items {
                    if it.get("type").and_then(|v| v.as_str()) == Some("woke") {
                        start_ts = row_created_at;
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
        }
        let now = chrono::Utc::now();
        let mut runtime_seconds = (now - start_ts).num_seconds();
        if runtime_seconds < 0 {
            runtime_seconds = 0;
        }

        let output_json = serde_json::json!({
            "text": "",
            "items": [
                { "type": "slept", "note": note, "reason": reason, "by": created_by, "delay_seconds": delay_secs, "at": now_text, "runtime_seconds": runtime_seconds }
            ]
        });

        sqlx::query(
            r#"
            INSERT INTO agent_responses (id, agent_name, created_by, status, input, output, created_at, updated_at)
            VALUES (?, ?, ?, 'completed', ?, ?, NOW(), NOW())
            "#,
        )
        .bind(&response_id)
        .bind(&agent_name)
        .bind(&created_by)
        .bind(&serde_json::json!({"text": ""}))
        .bind(&output_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn handle_wake_agent(&self, task: AgentTask) -> Result<()> {
        let agent_name = task.agent_name;
        // Prefer explicitly provided principal/principal_type from payload (owner),
        // fall back to task.created_by as a regular subject.
        let effective_principal = task
            .payload
            .get("principal")
            .and_then(|v| v.as_str())
            .unwrap_or(&task.created_by)
            .to_string();
        let effective_principal_type = task
            .payload
            .get("principal_type")
            .and_then(|v| v.as_str())
            .unwrap_or("User")
            .to_string();

        info!("Waking container for agent {}", agent_name);

        // Generate fresh tokens for woken agent
        info!("Generating fresh tokens for woken agent {}", agent_name);
        let wake_token = self
            .generate_agent_token(
                &effective_principal,
                match effective_principal_type.as_str() { "Admin" => SubjectType::Admin, _ => SubjectType::Subject },
                &agent_name,
            )
            .map_err(|e| anyhow::anyhow!("Failed to generate wake agent token: {}", e))?;

        // All woken agents were slept (container destroyed), so recreate container
        info!(
            "Agent {} was slept, waking container with persistent volume and fresh tokens",
            agent_name
        );
        self.docker_manager
            .wake_container_with_tokens(
                &agent_name,
                wake_token,
                effective_principal.clone(),
                effective_principal_type.clone(),
                task.created_at,
            )
            .await?;

        // Update last_activity_at and clear idle_from/busy_from since agent is being woken (will set to idle later)
        sqlx::query(
            r#"UPDATE agents SET last_activity_at = NOW(), idle_from = NULL, busy_from = NULL WHERE name = ?"#,
        )
        .bind(&agent_name)
        .execute(&self.pool)
        .await?;

        info!("Container woken for agent {}", agent_name);

        // Send prompt if provided
        if let Some(prompt) = task.payload.get("prompt").and_then(|v| v.as_str()) {
            info!("Sending prompt to woken agent {}: {}", agent_name, prompt);

            // Get the principal name from the task
            let principal = effective_principal.clone();

            // Create response record in database for woken agent
            let response_id = uuid::Uuid::new_v4().to_string();
            let input_json = serde_json::json!({ "text": prompt });
            let output_json = serde_json::json!({ "text": "", "items": [] });
            sqlx::query(
                r#"
                INSERT INTO agent_responses (id, agent_name, created_by, status, input, output, created_at, updated_at)
                VALUES (?, ?, ?, 'pending', ?, ?, NOW(), NOW())
                "#,
            )
            .bind(&response_id)
            .bind(&agent_name)
            .bind(&principal)
            .bind(&input_json)
            .bind(&output_json)
            .execute(&self.pool)
            .await?;
            info!(
                "Prompt response {} created for woken agent {}",
                response_id, agent_name
            );
        }

        // Insert a chat marker indicating the agent has woken
        let response_id = uuid::Uuid::new_v4().to_string();
        let now_text = chrono::Utc::now().to_rfc3339();
        let reason = task
            .payload
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("user_wake");
        let note = if reason == "user_wake" {
            "User wake"
        } else {
            "Wake"
        };
        let output_json = serde_json::json!({
            "text": "",
            "items": [ { "type": "woke", "note": note, "reason": reason, "by": effective_principal, "at": now_text } ]
        });
        sqlx::query(
            r#"
            INSERT INTO agent_responses (id, agent_name, created_by, status, input, output, created_at, updated_at)
            VALUES (?, ?, ?, 'completed', ?, ?, ?, ?)
            "#,
        )
        .bind(&response_id)
        .bind(&agent_name)
        .bind(&principal)
        .bind(&serde_json::json!({"text":""}))
        .bind(&output_json)
        .bind(&task.created_at)
        .bind(&task.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn handle_create_response(&self, task: AgentTask) -> Result<()> {
        let agent_name = task.agent_name.clone();
        let principal = task.created_by.clone();

        info!("Handling create_response for agent {}", agent_name);

        // Parse payload
        let response_id = task
            .payload
            .get("response_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing response_id in payload"))?;
        let input = task
            .payload
            .get("input")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"text":""}));
        let wake_if_slept = task
            .payload
            .get("wake_if_slept")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Inspect agent state
        let state_opt: Option<(String,)> =
            sqlx::query_as(r#"SELECT state FROM agents WHERE name = ?"#)
                .bind(&agent_name)
                .fetch_optional(&self.pool)
                .await?;
        let state = state_opt.map(|t| t.0).unwrap_or_default();

        // Wake if needed
        if wake_if_slept && state == "slept" {
            info!(
                "Agent {} slept; waking prior to inserting response",
                agent_name
            );
            let wake_token = self
                .generate_agent_token(&principal, SubjectType::Subject, &agent_name)
                .map_err(|e| anyhow::anyhow!("Failed to generate wake agent token: {}", e))?;
            self.docker_manager
                .wake_container_with_tokens(
                    &agent_name,
                    wake_token,
                    principal.clone(),
                    "User".to_string(),
                    task.created_at,
                )
                .await?;
            sqlx::query(
                r#"UPDATE agents SET last_activity_at = NOW(), idle_from = NULL, busy_from = NULL WHERE name = ?"#,
            )
            .bind(&agent_name)
            .execute(&self.pool)
            .await?;

            // Insert a wake marker for implicit wake
            let marker_id = uuid::Uuid::new_v4().to_string();
            let now_text = chrono::Utc::now().to_rfc3339();
            // Ensure the wake marker sorts before the newly created response row by using a slightly earlier timestamp
            let marker_created_at = task
                .created_at
                .checked_sub_signed(chrono::Duration::milliseconds(1))
                .unwrap_or(task.created_at);
            let output_json = serde_json::json!({
                "text": "",
                "items": [ { "type": "woke", "note": "Incoming request", "reason": "implicit_wake", "by": principal, "at": now_text } ]
            });
            sqlx::query(
            r#"
            INSERT INTO agent_responses (id, agent_name, created_by, status, input, output, created_at, updated_at)
            VALUES (?, ?, ?, 'completed', ?, ?, ?, ?)
            "#,
        )
        .bind(&marker_id)
        .bind(&agent_name)
        .bind(&principal)
        .bind(&serde_json::json!({"text":""}))
        .bind(&output_json)
        .bind(&marker_created_at)
        .bind(&marker_created_at)
        .execute(&self.pool)
        .await?;
        }

        // If a response with this id already exists (e.g., pre-insert cancel), skip insertion
        if let Some((_existing_id, existing_status)) = sqlx::query_as::<_, (String, String)>(
            r#"SELECT id, status FROM agent_responses WHERE id = ?"#
        )
        .bind(&response_id)
        .fetch_optional(&self.pool)
        .await? {
            info!("Response {} already exists with status {}, skipping insert", response_id, existing_status);
            return Ok(());
        }

        // Insert response row
        // To avoid identical timestamps with the implicit wake marker (second-level precision
        // in MySQL DATETIME), create the response one second after the task's created_at.
        let output_json = serde_json::json!({ "text": "", "items": [] });
        let resp_created_at = task
            .created_at
            .checked_add_signed(chrono::Duration::seconds(1))
            .unwrap_or(task.created_at);
        sqlx::query(
            r#"
            INSERT INTO agent_responses (id, agent_name, created_by, status, input, output, created_at, updated_at)
            VALUES (?, ?, ?, 'pending', ?, ?, ?, ?)
            "#,
        )
        .bind(&response_id)
        .bind(&agent_name)
        .bind(&principal)
        .bind(&input)
        .bind(&output_json)
        .bind(&resp_created_at)
        .bind(&resp_created_at)
        .execute(&self.pool)
        .await?;
        info!("Inserted response {} for agent {}", response_id, agent_name);

        Ok(())
    }

    async fn handle_publish_agent(&self, task: AgentTask) -> Result<()> {
        let agent_name = &task.agent_name;
        info!("Publishing content for agent {}", agent_name);

        // Check if docker command is available
        match tokio::process::Command::new("which")
            .arg("docker")
            .output()
            .await
        {
            Ok(output) if output.status.success() => {
                let docker_path = String::from_utf8_lossy(&output.stdout);
                let docker_path = docker_path.trim();
                info!("Found docker at: {}", docker_path);
            }
            _ => {
                warn!("Could not find docker binary");
            }
        }

        let agent_container = format!("raworc_agent_{}", agent_name.to_ascii_lowercase());

        // First, create the content directory in the content container
        let public_dir = format!("/content/{}", agent_name);
        info!(
            "Executing: docker exec raworc_content mkdir -p {}",
            public_dir
        );

        let mkdir_output = tokio::process::Command::new("docker")
            .args(&["exec", "raworc_content", "mkdir", "-p", &public_dir])
            .output()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to execute mkdir command for agent {}: {}",
                    agent_name,
                    e
                )
            })?;

        if !mkdir_output.status.success() {
            let stderr = String::from_utf8_lossy(&mkdir_output.stderr);
            let stdout = String::from_utf8_lossy(&mkdir_output.stdout);
            return Err(anyhow::anyhow!(
                "Failed to create public directory for agent {}: stdout: {}, stderr: {}",
                agent_name,
                stdout,
                stderr
            ));
        }

        // Copy content files from agent container directly to server container's public directory
        // This uses docker cp to copy from agent container to filesystem, then from filesystem to server container
        let temp_dir = format!("/tmp/content_publish_{}", agent_name);

        // Create temp directory on filesystem
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| anyhow::anyhow!("Failed to create temp directory: {}", e))?;

        // Copy from agent container to filesystem temp
        let copy1_output = tokio::process::Command::new("docker")
            .args(&[
                "cp",
                &format!("{}:/agent/content/.", agent_container),
                &format!("{}/", temp_dir),
            ])
            .output()
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to execute copy command from agent container: {}", e)
            })?;

        if !copy1_output.status.success() {
            let _ = std::fs::remove_dir_all(&temp_dir);
            let stderr = String::from_utf8_lossy(&copy1_output.stderr);
            return Err(anyhow::anyhow!(
                "Failed to copy content from agent container: {}",
                stderr
            ));
        }

        // Copy from filesystem temp to content container
        let copy2_output = tokio::process::Command::new("docker")
            .args(&[
                "cp",
                &format!("{}//.", temp_dir),
                &format!("raworc_content:/content/{}/", agent_name),
            ])
            .output()
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to execute copy command to server container: {}", e)
            })?;

        // Clean up temp directory
        let _ = std::fs::remove_dir_all(&temp_dir);

        if !copy2_output.status.success() {
            let stderr = String::from_utf8_lossy(&copy2_output.stderr);
            return Err(anyhow::anyhow!(
                "Failed to copy content to server container: {}",
                stderr
            ));
        }

        info!(
            "Content published for agent {} to /content/{}/",
            agent_name, agent_name
        );
        Ok(())
    }

    async fn handle_unpublish_agent(&self, task: AgentTask) -> Result<()> {
        let agent_name = &task.agent_name;
        info!("Unpublishing content for agent {}", agent_name);

        // Remove content directory for this agent from the content container
        let public_path = format!("/content/{}", agent_name);
        info!(
            "Executing: docker exec raworc_content rm -rf {}",
            public_path
        );

        // Remove the published directory from content container
        let remove_output = tokio::process::Command::new("docker")
            .args(&["exec", "raworc_content", "rm", "-rf", &public_path])
            .output()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to execute rm command for agent {}: {}",
                    agent_name,
                    e
                )
            })?;

        if !remove_output.status.success() {
            let stderr = String::from_utf8_lossy(&remove_output.stderr);
            let stdout = String::from_utf8_lossy(&remove_output.stdout);
            return Err(anyhow::anyhow!(
                "Failed to remove public directory for agent {}: stdout: {}, stderr: {}",
                agent_name,
                stdout,
                stderr
            ));
        }

        info!("Content unpublished for agent {}", agent_name);
        Ok(())
    }

    /// Check health of all non-sleeping agents and mark failed containers as slept
    async fn check_agent_health(&self) -> Result<usize> {
        // Find all agents that are not sleeping (active agents)
        let active_agents: Vec<(String, String)> = sqlx::query_as(
            r#"
            SELECT name, state
            FROM agents
            WHERE state != 'slept'
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        if active_agents.is_empty() {
            return Ok(0);
        }

        info!("Checking health of {} active agents", active_agents.len());
        let mut recovered_count = 0;

        for (agent_name, current_state) in active_agents {
            // Check if container exists and is running
            match self.docker_manager.is_container_healthy(&agent_name).await {
                Ok(true) => {
                    // Container is healthy, no action needed
                    continue;
                }
                Ok(false) => {
                    // Container is unhealthy or doesn't exist
                    warn!(
                        "Agent {} container is unhealthy or missing, marking as slept for recovery",
                        agent_name
                    );

                    // Mark agent as slept so it can be woken up later
                    if let Err(e) =
                        sqlx::query(r#"UPDATE agents SET state = 'slept' WHERE name = ?"#)
                            .bind(&agent_name)
                            .execute(&self.pool)
                            .await
                    {
                        error!(
                            "Failed to mark unhealthy agent {} as slept: {}",
                            agent_name, e
                        );
                    } else {
                        info!(
                            "Agent {} marked as slept due to container failure (was: {})",
                            agent_name, current_state
                        );
                        recovered_count += 1;
                    }
                }
                Err(e) => {
                    // Health check failed, likely Docker connection issues
                    error!(
                        "Health check failed for agent {}: {}, will retry next cycle",
                        agent_name, e
                    );
                }
            }
        }

        if recovered_count > 0 {
            info!(
                "Marked {} agents as slept due to container failures",
                recovered_count
            );
        }

        Ok(recovered_count)
    }

    fn sanitize_relative_path(&self, p: &str) -> Result<String> {
        let p = p.trim();
        if p.is_empty() { return Ok(String::new()); }
        if p.starts_with('/') || p.contains('\0') { return Err(anyhow::anyhow!("invalid path")); }
        let mut parts = Vec::new();
        for seg in p.split('/') {
            if seg.is_empty() || seg == "." || seg == ".." {
                return Err(anyhow::anyhow!("invalid path"));
            }
            parts.push(seg);
        }
        Ok(parts.join("/"))
    }

    async fn task_update_result(&self, task_id: &str, mut payload: serde_json::Value, result: serde_json::Value) -> Result<()> {
        if let serde_json::Value::Object(ref mut map) = payload { map.insert("result".into(), result); }
        sqlx::query(
            r#"UPDATE agent_tasks SET payload = ?, status='completed', updated_at=NOW(), completed_at=NOW(), error=NULL WHERE id = ?"#
        )
        .bind(&payload)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn task_fail(&self, task_id: &str, msg: String) -> Result<()> {
        sqlx::query(
            r#"UPDATE agent_tasks SET status='failed', updated_at=NOW(), completed_at=NOW(), error=? WHERE id = ?"#
        )
        .bind(&msg)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn handle_file_read(&self, task: AgentTask) -> Result<()> {
        let path = task.payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let safe = self.sanitize_relative_path(path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        // Do not auto-wake for file APIs; require running container
        match self.docker_manager.is_container_healthy(&task.agent_name).await {
            Ok(true) => {}
            _ => { return self.task_fail(&task.id, "agent is sleeping".to_string()).await; }
        }
        let full_path = format!("/agent/{}", safe);
        // Get size and content type
        let (stat_code, stat_out, _stat_err) = self.docker_manager.exec_collect(&task.agent_name, vec!["/usr/bin/stat".into(), "-c".into(), "%s".into(), full_path.clone()]).await?;
        if stat_code != 0 { return self.task_fail(&task.id, "not found or invalid".to_string()).await; }
        let size: u64 = String::from_utf8_lossy(&stat_out).trim().parse().unwrap_or(0);
        // Cap at 25MB
        const MAX_BYTES: u64 = 25 * 1024 * 1024;
        if size > MAX_BYTES {
            return self
                .task_fail(&task.id, format!("file too large ({} bytes > 25MB)", size))
                .await;
        }
        let (code, stdout, stderr) = self.docker_manager.exec_collect(&task.agent_name, vec!["/bin/cat".into(), full_path.clone()]).await?;
        if code != 0 { return self.task_fail(&task.id, String::from_utf8_lossy(&stderr).to_string()).await; }
        let ct = guess_content_type(&safe);
        let content_b64 = base64::encode(&stdout);
        let result = serde_json::json!({
            "content_base64": content_b64,
            "content_type": ct,
            "size": size,
        });
        self.task_update_result(&task.id, task.payload.clone(), result).await
    }

    pub async fn handle_file_metadata(&self, task: AgentTask) -> Result<()> {
        let path = task.payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let safe = self.sanitize_relative_path(path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        match self.docker_manager.is_container_healthy(&task.agent_name).await {
            Ok(true) => {}
            _ => { return self.task_fail(&task.id, "agent is sleeping".to_string()).await; }
        }
        let full_path = format!("/agent/{}", safe);
        let fmt = "%F|%s|%a|%Y|%N";
        let (code, stdout, stderr) = self.docker_manager.exec_collect(&task.agent_name, vec!["/usr/bin/stat".into(), "-c".into(), fmt.into(), full_path.clone()]).await?;
        if code != 0 { return self.task_fail(&task.id, String::from_utf8_lossy(&stderr).to_string()).await; }
        let line = String::from_utf8_lossy(&stdout);
        let parts: Vec<&str> = line.trim().split('|').collect();
        if parts.len() < 4 { return self.task_fail(&task.id, "unexpected stat output".into()).await; }
        let kind_raw = parts[0].to_ascii_lowercase();
        let kind = if kind_raw.contains("regular") { "file" } else if kind_raw.contains("directory") { "dir" } else if kind_raw.contains("symbolic link") { "symlink" } else { &kind_raw };
        let size: u64 = parts[1].parse().unwrap_or(0);
        let mode = format!("{:0>4}", parts[2]);
        let mtime_epoch: i64 = parts[3].parse().unwrap_or(0);
        let mtime = chrono::Utc.timestamp_opt(mtime_epoch, 0).single().unwrap_or_else(|| chrono::Utc.timestamp_opt(0,0).unwrap());
        let mut obj = serde_json::json!({ "kind": kind, "size": size, "mode": mode, "mtime": mtime.to_rfc3339() });
        if parts.len() >= 5 && kind == "symlink" {
            if let Some(idx) = parts[4].find("->") { let target = parts[4][idx+2..].trim().trim_matches('\''); obj["link_target"] = serde_json::Value::String(target.to_string()); }
        }
        self.task_update_result(&task.id, task.payload.clone(), obj).await
    }

    pub async fn handle_file_list(&self, task: AgentTask) -> Result<()> {
        let path = task.payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let offset = task.payload.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);
        let limit = task.payload.get("limit").and_then(|v| v.as_u64()).unwrap_or(100).min(500);
        let safe = if path.is_empty() { String::new() } else { self.sanitize_relative_path(path).map_err(|e| anyhow::anyhow!(e.to_string()))? };
        match self.docker_manager.is_container_healthy(&task.agent_name).await {
            Ok(true) => {}
            _ => { return self.task_fail(&task.id, "agent is sleeping".to_string()).await; }
        }
        let base = if safe.is_empty() { "/agent".to_string() } else { format!("/agent/{}", safe) };

        // Print one record per line so parser can split by lines()
        let fmt = "%f|%y|%s|%m|%T@\n";
        let (code, stdout, stderr) = self.docker_manager.exec_collect(&task.agent_name, vec!["/usr/bin/find".into(), base.clone(), "-maxdepth".into(), "1".into(), "-mindepth".into(), "1".into(), "-printf".into(), fmt.into(), "-name".into(), "*".into()]).await?;
        if code != 0 { return self.task_fail(&task.id, String::from_utf8_lossy(&stderr).to_string()).await; }
        let mut entries: Vec<(String,String,u64,String,i64)> = Vec::new();
        for line in String::from_utf8_lossy(&stdout).lines() {
            let parts: Vec<&str> = line.trim().split('|').collect();
            if parts.len() < 5 { continue; }
            let name = parts[0].to_string(); if name == "." || name == ".." { continue; }
            let kind = match parts[1] { "f"=>"file", "d"=>"dir", "l"=>"symlink", other=>other } .to_string();
            let size: u64 = parts[2].parse().unwrap_or(0);
            let mode = format!("{:0>4}", parts[3]);
            let mtime_secs: i64 = parts[4].split('.').next().unwrap_or("0").parse().unwrap_or(0);
            entries.push((name, kind, size, mode, mtime_secs));
        }
        entries.sort_by(|a,b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        let total = entries.len() as u64;
        let slice_start = offset as usize;
        let slice_end = (offset+limit).min(total) as usize;
        let mut list: Vec<serde_json::Value> = Vec::new();
        if slice_start < entries.len() {
            for (name, kind, size, mode, mtime_secs) in entries[slice_start..slice_end].iter() {
                let mtime = chrono::Utc.timestamp_opt(*mtime_secs, 0).single().unwrap_or_else(|| chrono::Utc.timestamp_opt(0,0).unwrap());
                list.push(serde_json::json!({"name": name, "kind": kind, "size": size, "mode": mode, "mtime": mtime.to_rfc3339()}));
            }
        }
        let next_offset = if (offset+limit) < total { Some(offset+limit) } else { None };
        let result = serde_json::json!({ "entries": list, "offset": offset, "limit": limit, "next_offset": next_offset, "total": total });
        self.task_update_result(&task.id, task.payload.clone(), result).await
    }

    pub async fn handle_file_delete(&self, task: AgentTask) -> Result<()> {
        let path = task.payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let safe = self.sanitize_relative_path(path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        match self.docker_manager.is_container_healthy(&task.agent_name).await {
            Ok(true) => {}
            _ => { return self.task_fail(&task.id, "agent is sleeping".to_string()).await; }
        }
        let full_path = format!("/agent/{}", safe);
        // Ensure it's a regular file
        let (stat_code, stat_out, _stat_err) = self.docker_manager.exec_collect(&task.agent_name, vec!["/usr/bin/stat".into(), "-c".into(), "%F".into(), full_path.clone()]).await?;
        if stat_code != 0 { return self.task_fail(&task.id, "not found".to_string()).await; }
        let kind = String::from_utf8_lossy(&stat_out).to_ascii_lowercase();
        if !kind.contains("regular file") {
            return self.task_fail(&task.id, "Path is not a file".to_string()).await;
        }
        let (rm_code, _out, err) = self.docker_manager.exec_collect(&task.agent_name, vec!["/bin/rm".into(), "-f".into(), full_path.clone()]).await?;
        if rm_code != 0 { return self.task_fail(&task.id, String::from_utf8_lossy(&err).to_string()).await; }
        let result = serde_json::json!({ "deleted": true, "path": safe });
        self.task_update_result(&task.id, task.payload.clone(), result).await
    }
}

fn guess_content_type(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".html") || lower.ends_with(".htm") { "text/html; charset=utf-8" }
    else if lower.ends_with(".css") { "text/css; charset=utf-8" }
    else if lower.ends_with(".js") { "application/javascript" }
    else if lower.ends_with(".json") { "application/json" }
    else if lower.ends_with(".md") || lower.ends_with(".txt") || lower.ends_with(".rs") || lower.ends_with(".py") || lower.ends_with(".ts") || lower.ends_with(".sh") || lower.ends_with(".yml") || lower.ends_with(".yaml") || lower.ends_with(".toml") { "text/plain; charset=utf-8" }
    else if lower.ends_with(".svg") { "image/svg+xml" }
    else if lower.ends_with(".png") { "image/png" }
    else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") { "image/jpeg" }
    else if lower.ends_with(".gif") { "image/gif" }
    else { "application/octet-stream" }
}
