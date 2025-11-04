use anyhow::Result;
use bollard::Docker;
use chrono::{DateTime, TimeZone, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sqlx::{mysql::MySqlPoolOptions, MySql, Pool};
use std::convert::TryFrom;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{error, info, warn};

// Import constants from shared module
#[path = "../shared/models/constants.rs"]
pub mod constants;
pub use constants::SESSION_STATE_INIT;

// Using local Ollama via OLLAMA_HOST

#[path = "../shared/rbac.rs"]
pub mod rbac;
use rbac::{RbacClaims, SubjectType};

use super::docker_manager::DockerManager;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SessionRequest {
    id: String,
    request_type: String,
    session_name: String,
    created_by: String,
    payload: serde_json::Value,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    error: Option<String>,
}

pub struct SessionManager {
    pool: Pool<MySql>,
    docker_manager: DockerManager,
    jwt_secret: String,
}

impl SessionManager {
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
            "Session Manager started, polling for requests, auto-stop monitoring, and health checks..."
        );

        // Run frequent request polling; run heavier maintenance on a slower cadence
        let mut last_auto_stop = Instant::now() - Duration::from_secs(60);
        let mut last_health = Instant::now() - Duration::from_secs(60);
        let mut last_task_timeout = Instant::now() - Duration::from_secs(60);
        loop {
            // Process pending requests (fast path)
            let requests_processed = match self.process_pending_requests().await {
                Ok(processed) => processed,
                Err(e) => {
                    error!("Error processing requests: {}", e);
                    0
                }
            };

            // Process auto-stop every 10s
            let mut sessions_stopped = 0;
            if last_auto_stop.elapsed() >= Duration::from_secs(10) {
                sessions_stopped = match self.process_auto_stop().await {
                    Ok(count) => count,
                    Err(e) => {
                        error!("Error processing auto-stop: {}", e);
                        0
                    }
                };
                last_auto_stop = Instant::now();
            }

            // Check health every 10s
            let mut sessions_recovered = 0;
            if last_health.elapsed() >= Duration::from_secs(10) {
                sessions_recovered = match self.check_session_health().await {
                    Ok(recovered) => recovered,
                    Err(e) => {
                        error!("Error checking session health: {}", e);
                        0
                    }
                };
                last_health = Instant::now();
            }

            // Cancel per-task timeouts every 5s
            let mut tasks_cancelled = 0;
            if last_task_timeout.elapsed() >= Duration::from_secs(5) {
                tasks_cancelled = match self.process_task_timeouts().await {
                    Ok(count) => count,
                    Err(e) => {
                        error!("Error processing task timeouts: {}", e);
                        0
                    }
                };
                last_task_timeout = Instant::now();
            }

            // If no work was done, short sleep before next poll (improves responsiveness)
            if requests_processed == 0
                && sessions_stopped == 0
                && sessions_recovered == 0
                && tasks_cancelled == 0
            {
                sleep(Duration::from_millis(250)).await;
            }
        }
    }

    /// Ensure the session container is running and healthy; restart if needed and wait up to timeout_secs
    pub async fn ensure_session_running(
        &self,
        session_name: &str,
        timeout_secs: u64,
    ) -> Result<()> {
        // Quick healthy check
        match self.docker_manager.is_container_healthy(session_name).await {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => {
                tracing::warn!("health check error for {}: {}", session_name, e);
            }
        }

        // If DB says stopped or container absent, restart
        if let Some((state,)) =
            sqlx::query_as::<_, (String,)>(r#"SELECT state FROM sessions WHERE name = ?"#)
                .bind(session_name)
                .fetch_optional(&self.pool)
                .await?
        {
            if state.to_lowercase() == "stopped" {
                tracing::info!("Session {} is stopped; restarting container", session_name);
                let _ = self.docker_manager.restart_container(session_name).await?;
            }
        } else {
            // No row; nothing we can do
            tracing::warn!(
                "Session {} not found in DB during ensure_session_running",
                session_name
            );
        }

        // Wait for healthy
        let mut waited = 0u64;
        let step = 500u64; // ms
        while waited / 1000 < timeout_secs {
            if let Ok(true) = self.docker_manager.is_container_healthy(session_name).await {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(step)).await;
            waited += step;
        }
        Err(anyhow::anyhow!(
            "session {} not ready in {}s",
            session_name,
            timeout_secs
        ))
    }

    /// Proxy exec with stdout/stderr collection
    pub async fn exec_collect(
        &self,
        session_name: &str,
        cmd: Vec<String>,
    ) -> Result<(i32, Vec<u8>, Vec<u8>)> {
        self.docker_manager.exec_collect(session_name, cmd).await
    }

    // No external API key required for local Ollama

    /// Process sessions that need auto-stopping due to timeout
    async fn process_auto_stop(&self) -> Result<usize> {
        // Ensure all idle sessions have idle_from set
        let _ = sqlx::query(
            r#"
            UPDATE sessions
            SET idle_from = NOW()
            WHERE state = 'idle' AND idle_from IS NULL
            "#,
        )
        .execute(&self.pool)
        .await;

        // Ensure all busy sessions have busy_from set
        let _ = sqlx::query(
            r#"
            UPDATE sessions
            SET busy_from = NOW()
            WHERE state = 'busy' AND busy_from IS NULL
            "#,
        )
        .execute(&self.pool)
        .await;

        // Find sessions that need auto-stop due to idle timeout
        let sessions_to_stop: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT name
            FROM sessions
            WHERE state = 'idle'
              AND idle_from IS NOT NULL
              AND TIMESTAMPADD(SECOND, stop_timeout_seconds, idle_from) <= NOW()
            ORDER BY TIMESTAMPADD(SECOND, stop_timeout_seconds, idle_from) ASC
            LIMIT 50
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to find sessions to auto-stop: {}", e))?;

        let mut stopped_count = 0;

        for (session_name,) in sessions_to_stop {
            info!("Auto-stopping session {} due to timeout", session_name);

            // Create stop request for the session
            let request_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(r#"
                INSERT INTO session_requests (id, session_name, request_type, created_by, payload, status)
                VALUES (?, ?, 'stop_session', 'system', '{"reason": "auto_stop_timeout"}', 'pending')
                "#)
            .bind(&request_id)
            .bind(&session_name)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create auto-stop request for session {}: {}", session_name, e))?;

            info!(
                "Created auto-stop request {} for session {}",
                request_id, session_name
            );
            stopped_count += 1;
        }

        if stopped_count > 0 {
            info!("Scheduled {} sessions for auto-stop", stopped_count);
        }

        Ok(stopped_count)
    }

    async fn process_task_timeouts(&self) -> Result<usize> {
        let timed_out: Vec<(String, String, serde_json::Value, chrono::DateTime<Utc>)> =
            sqlx::query_as(
                r#"
                SELECT id, session_name, output, created_at
                FROM session_tasks
                WHERE timeout_at IS NOT NULL
                  AND timeout_at <= NOW()
                  AND status IN ('pending', 'processing')
                ORDER BY timeout_at ASC
                LIMIT 50
                "#,
            )
            .fetch_all(&self.pool)
            .await?;

        if timed_out.is_empty() {
            return Ok(0);
        }

        let mut cancelled = 0usize;
        for (task_id, session_name, output_json, created_at) in timed_out {
            let now = chrono::Utc::now();
            let now_text = now.to_rfc3339();
            let runtime_seconds = (now - created_at).num_seconds();
            let runtime_seconds = if runtime_seconds < 0 {
                0
            } else {
                runtime_seconds
            };

            let marker = serde_json::json!({
                "type": "cancelled",
                "reason": "task_timeout",
                "at": now_text,
                "runtime_seconds": runtime_seconds
            });

            let mut new_output = output_json.clone();
            let mut items = new_output
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_else(Vec::new);
            items.push(marker);

            if let serde_json::Value::Object(ref mut map) = new_output {
                map.insert("items".to_string(), serde_json::Value::Array(items));
                map.entry("text")
                    .or_insert_with(|| serde_json::Value::String(String::new()));
            } else {
                new_output = serde_json::json!({
                    "text": "",
                    "items": [
                        {
                            "type": "cancelled",
                            "reason": "task_timeout",
                            "at": now_text,
                            "runtime_seconds": runtime_seconds
                        }
                    ]
                });
            }

            let update = sqlx::query(
                r#"
                UPDATE session_tasks
                SET status = 'cancelled',
                    output = ?,
                    timeout_seconds = NULL,
                    timeout_at = NULL,
                    updated_at = NOW()
                WHERE id = ? AND status IN ('pending','processing')
                "#,
            )
            .bind(&new_output)
            .bind(&task_id)
            .execute(&self.pool)
            .await?;

            if update.rows_affected() == 0 {
                continue;
            }

            cancelled += 1;

            sqlx::query(
                r#"
                UPDATE sessions
                SET state = 'idle',
                    busy_from = NULL,
                    idle_from = NOW(),
                    last_activity_at = NOW()
                WHERE name = ? AND state = 'busy'
                "#,
            )
            .bind(&session_name)
            .execute(&self.pool)
            .await?;

            info!(
                "Cancelled task {} for session {} due to per-task timeout",
                task_id, session_name
            );
        }

        Ok(cancelled)
    }

    /// Generate a session-specific TSBX token for the given principal
    fn generate_session_token(
        &self,
        principal: &str,
        principal_type: SubjectType,
        session_name: &str,
    ) -> Result<String> {
        let exp = chrono::Utc::now() + chrono::Duration::hours(24);
        let claims = RbacClaims {
            sub: principal.to_string(), // Use original principal name for API server compatibility
            sub_type: principal_type,
            exp: exp.timestamp() as usize,
            iat: chrono::Utc::now().timestamp() as usize,
            iss: "tsbx-session-manager".to_string(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_ref()),
        )
        .map_err(|e| anyhow::anyhow!("Failed to generate session token: {}", e))?;

        info!(
            "Generated session token for principal: {} (session: {})",
            principal, session_name
        );
        Ok(token)
    }

    async fn process_pending_requests(&self) -> Result<usize> {
        let requests = self.fetch_pending_requests().await?;
        let mut processed = 0;

        for request in requests {
            match self.process_request(request).await {
                Ok(_) => processed += 1,
                Err(e) => error!("Failed to process request: {}", e),
            }
        }

        Ok(processed)
    }

    async fn fetch_pending_requests(&self) -> Result<Vec<SessionRequest>> {
        // MySQL doesn't support RETURNING, so we need to do this in two steps
        // First, get and lock the pending requests
        let request_ids: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT id
            FROM session_requests
            WHERE status = 'pending'
            ORDER BY created_at
            LIMIT 5
            FOR UPDATE
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        if request_ids.is_empty() {
            return Ok(vec![]);
        }

        // Mark requests as processing
        let ids: Vec<String> = request_ids.into_iter().map(|(id,)| id).collect();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "UPDATE session_requests SET status = 'processing', started_at = NOW(), updated_at = NOW() WHERE id IN ({placeholders})"
        );

        let mut query = sqlx::query(&query_str);
        for id in &ids {
            query = query.bind(id);
        }
        query.execute(&self.pool).await?;

        // Fetch the now-processing requests
        let query_str = format!("SELECT * FROM session_requests WHERE id IN ({placeholders})");
        let mut query = sqlx::query_as::<_, SessionRequest>(&query_str);
        for id in &ids {
            query = query.bind(id);
        }
        let requests = query.fetch_all(&self.pool).await?;

        Ok(requests)
    }

    async fn process_request(&self, request: SessionRequest) -> Result<()> {
        info!(
            "Processing request {} of type {}",
            request.id, request.request_type
        );

        let result = match request.request_type.as_str() {
            "start_session" => self.handle_start_session(request.clone()).await,
            "delete_session" => self.handle_delete_session(request.clone()).await,
            "execute_command" => self.handle_execute_command(request.clone()).await,
            "stop_session" => self.handle_stop_session(request.clone()).await,
            "restart_session" => self.handle_restart_session(request.clone()).await,
            "create_task" => self.handle_create_task(request.clone()).await,
            "file_read" => self.handle_file_read(request.clone()).await,
            "file_metadata" => self.handle_file_metadata(request.clone()).await,
            "file_list" => self.handle_file_list(request.clone()).await,
            "file_delete" => self.handle_file_delete(request.clone()).await,
            _ => {
                warn!("Unknown request type: {}", request.request_type);
                Err(anyhow::anyhow!("Unknown request type"))
            }
        };

        match result {
            Ok(_) => {
                self.mark_request_completed(&request.id).await?;
                info!("Request {} completed successfully", request.id);
            }
            Err(e) => {
                self.mark_request_failed(&request.id, &e.to_string())
                    .await?;
                error!("Request {} failed: {}", request.id, e);
            }
        }

        Ok(())
    }

    pub async fn handle_start_session(&self, request: SessionRequest) -> Result<()> {
        let session_name = request.session_name.clone();

        // Parse the payload to get session creation parameters
        let env = request
            .payload
            .get("env")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                    .collect::<std::collections::HashMap<String, String>>()
            })
            .unwrap_or_default();

        let instructions = request
            .payload
            .get("instructions")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let setup = request
            .payload
            .get("setup")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let prompt = request
            .payload
            .get("prompt")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Extract principal information for logging and token generation
        let principal = request
            .payload
            .get("principal")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let principal_type_str = request
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

        // Generate dynamic token for this session (for TaskSandbox auth)
        info!("Generating dynamic token for session {}", session_name);
        let session_token = self
            .generate_session_token(principal, principal_type, &session_name)
            .map_err(|e| anyhow::anyhow!("Failed to generate session token: {}", e))?;

        info!(
            "Generated dynamic tokens for session {} (principal: {})",
            session_name, principal
        );

        info!("Creating session {} for principal {} ({:?}) with {} env, instructions: {}, setup: {}, prompt: {}", 
              session_name, principal, principal_type, env.len(), instructions.is_some(), setup.is_some(), prompt.is_some());

        // Check if this is a cloned session from request payload
        let is_clone = request
            .payload
            .get("clone")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // For cloned sessions, extract prompt from request payload
        let clone_prompt = if is_clone {
            request
                .payload
                .get("prompt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        if is_clone {
            let parent_session_name = request
                .payload
                .get("parent_session_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing parent_session_name for clone"))?;

            let copy_code = request
                .payload
                .get("copy_code")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let copy_env = request
                .payload
                .get("copy_env")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let copy_content = request
                .payload
                .get("copy_content")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            // For cloned sessions, get principal info from clone request payload
            let clone_principal = request
                .payload
                .get("principal")
                .and_then(|v| v.as_str())
                .unwrap_or(principal);
            let clone_principal_type_str = request
                .payload
                .get("principal_type")
                .and_then(|v| v.as_str())
                .unwrap_or(principal_type_str);

            info!(
                "DEBUG: Clone request payload principal: {:?}, principal_type: {:?}",
                request.payload.get("principal"),
                request.payload.get("principal_type")
            );
            info!(
                "DEBUG: Using clone_principal: {}, clone_principal_type_str: {}",
                clone_principal, clone_principal_type_str
            );
            let clone_principal_type = match clone_principal_type_str {
                "Admin" => SubjectType::Admin,
                "User" => SubjectType::Subject,
                _ => SubjectType::Subject,
            };

            info!("Creating cloned session {} from parent {} (copy_code: {}, copy_env: {}, copy_content: {}) for principal {} ({})", 
                  session_name, parent_session_name, copy_code, copy_env, copy_content, clone_principal, clone_principal_type_str);

            // For cloned sessions, create container with selective volume copy from parent
            // Generate fresh token for cloned session
            let clone_token = self
                .generate_session_token(clone_principal, clone_principal_type, &session_name)
                .map_err(|e| anyhow::anyhow!("Failed to generate cloned session token: {}", e))?;

            self.docker_manager
                .create_container_with_selective_copy_and_tokens(
                    &session_name,
                    parent_session_name,
                    copy_code,
                    copy_env,
                    copy_content,
                    clone_token,
                    clone_principal.to_string(),
                    clone_principal_type_str.to_string(),
                    request.created_at,
                )
                .await?;
        } else {
            info!("Creating new session {}", session_name);

            // For regular sessions, create container with session parameters and generated tokens
            self.docker_manager
                .create_container_with_params_and_tokens(
                    &session_name,
                    env,
                    instructions,
                    setup,
                    session_token,
                    principal.to_string(),
                    principal_type_str.to_string(),
                    request.created_at,
                )
                .await?;
        }

        // Send prompt if provided (BEFORE setting state to IDLE)
        let prompt_to_send = prompt.or(clone_prompt);
        if let Some(prompt) = prompt_to_send {
            info!("Sending prompt to session {}: {}", session_name, prompt);

            // Create task record in database (pending)
            let task_id = uuid::Uuid::new_v4().to_string();
            let input_json =
                serde_json::json!({ "content": [ { "type": "text", "content": prompt } ] });
            let output_json = serde_json::json!({ "items": [] });
            sqlx::query(
                r#"
                INSERT INTO session_tasks (id, session_name, created_by, status, input, output, timeout_seconds, timeout_at, created_at, updated_at)
                VALUES (?, ?, ?, 'pending', ?, ?, ?, ?, NOW(), NOW())
                "#,
            )
            .bind(&task_id)
            .bind(&session_name)
            .bind(&principal)
            .bind(&input_json)
            .bind(&output_json)
            .bind(Option::<i32>::None)
            .bind(Option::<chrono::DateTime<Utc>>::None)
            .execute(&self.pool)
            .await?;
            info!(
                "Prompt task {} created for session {}",
                task_id, session_name
            );
        }

        // Set session state to INIT after container creation only if it hasn't changed yet.
        // This avoids overwriting an session that already set itself to IDLE.
        sqlx::query(r#"UPDATE sessions SET state = ?, last_activity_at = NOW() WHERE name = ? AND state = 'init'"#)
            .bind(SESSION_STATE_INIT)
            .bind(&session_name)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn handle_delete_session(&self, request: SessionRequest) -> Result<()> {
        let session_name = request.session_name;

        info!("Deleting container and volume for session {}", session_name);
        self.docker_manager.delete_container(&session_name).await?;

        // No need to update session state - DELETE endpoint performs hard delete of session row

        Ok(())
    }

    pub async fn handle_execute_command(&self, request: SessionRequest) -> Result<()> {
        let session_name = request.session_name;
        let command = request.payload["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing command in payload"))?;

        info!("Executing command in session {}: {}", session_name, command);
        let output = self
            .docker_manager
            .execute_command(&session_name, command)
            .await?;

        sqlx::query(
            r#"
            INSERT INTO command_results (id, session_name, command, output, created_at)
            VALUES (?, ?, ?, ?, NOW())
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(session_name)
        .bind(command)
        .bind(output)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn mark_request_completed(&self, request_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE session_requests
            SET status = 'completed',
                completed_at = NOW(),
                updated_at = NOW()
            WHERE id = ?
            "#,
        )
        .bind(request_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn mark_request_failed(&self, request_id: &str, error: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE session_requests
            SET status = 'failed',
                error = ?,
                completed_at = NOW(),
                updated_at = NOW()
            WHERE id = ?
            "#,
        )
        .bind(request_id)
        .bind(error)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn handle_stop_session(&self, request: SessionRequest) -> Result<()> {
        let session_name = request.session_name;
        // Optional delay before closing (in seconds), minimum 5 seconds
        let delay_secs = request
            .payload
            .get("delay_seconds")
            .and_then(|v| v.as_u64())
            .map(|d| if d < 5 { 5 } else { d })
            .unwrap_or(5);
        if delay_secs > 0 {
            info!(
                "Delaying close for session {} by {} seconds",
                session_name, delay_secs
            );
            sleep(Duration::from_secs(delay_secs)).await;
        }
        // Capture prior state and created_at for runtime measurement
        let session_row_opt: Option<(chrono::DateTime<Utc>, String)> =
            sqlx::query_as(r#"SELECT created_at, state FROM sessions WHERE name = ?"#)
                .bind(&session_name)
                .fetch_optional(&self.pool)
                .await?;
        let (session_created_at, prior_state) = session_row_opt
            .map(|(c, s)| (c, s))
            .unwrap_or((chrono::Utc::now(), String::new()));

        // Determine note: auto timeout vs user-triggered
        let auto =
            request.payload.get("reason").and_then(|v| v.as_str()) == Some("auto_stop_timeout");
        let reason = if auto {
            if prior_state.to_lowercase() == "busy" {
                "task_timeout"
            } else {
                "stop_timeout"
            }
        } else {
            "user"
        };
        let is_task_timeout = reason == "task_timeout";

        if is_task_timeout {
            info!(
                "Task timeout detected for session {} – cancelling task and returning to idle",
                session_name
            );
            sqlx::query(
                r#"UPDATE sessions SET state = 'idle', busy_from = NULL, idle_from = NOW(), last_activity_at = NOW() WHERE name = ?"#,
            )
            .bind(&session_name)
            .execute(&self.pool)
            .await?;
        } else {
            info!("Stopping container for session {}", session_name);

            // Close the Docker container but keep the persistent volume
            self.docker_manager.stop_container(&session_name).await?;

            // Update session state to stopped
            sqlx::query(r#"UPDATE sessions SET state = 'stopped' WHERE name = ?"#)
                .bind(&session_name)
                .execute(&self.pool)
                .await?;

            info!("Session {} state updated to stopped", session_name);
        }

        // Create a chat marker task to indicate the action taken
        let task_id = uuid::Uuid::new_v4().to_string();
        let created_by = request.created_by.clone();
        let now_text = chrono::Utc::now().to_rfc3339();
        let note = if auto {
            if reason == "task_timeout" {
                "Task timeout"
            } else {
                "Stop timeout"
            }
        } else {
            request
                .payload
                .get("note")
                .and_then(|v| v.as_str())
                .unwrap_or("User requested stop")
        };

        // Mark the latest in-progress task as cancelled (processing or pending) (applies to any close reason)
        if let Some((task_id, output_json)) = sqlx::query_as::<_, (String, serde_json::Value)>(
            r#"SELECT id, output FROM session_tasks WHERE session_name = ? AND status IN ('processing','pending') ORDER BY created_at DESC LIMIT 1"#,
        )
        .bind(&session_name)
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
            // Update task status to 'cancelled'
            let _ = sqlx::query(
                r#"UPDATE session_tasks SET status = 'cancelled', output = ?, updated_at = NOW() WHERE id = ?"#,
            )
            .bind(&new_output)
            .bind(&task_id)
            .execute(&self.pool)
            .await;
        } else {
            // If no task row exists yet (pre-insert race), try to find the latest create_task request and insert a cancelled task
            if let Some((request_id, created_by, payload)) = sqlx::query_as::<_, (String, String, serde_json::Value)>(
                r#"SELECT id, created_by, payload FROM session_requests WHERE session_name = ? AND request_type = 'create_task' AND status IN ('pending','processing') ORDER BY created_at DESC LIMIT 1"#
            )
            .bind(&session_name)
            .fetch_optional(&self.pool)
            .await
            .unwrap_or(None)
            {
                if let Some(task_id) = payload.get("task_id").and_then(|v| v.as_str()) {
                    let input = payload.get("input").cloned().unwrap_or_else(|| serde_json::json!({"text":""}));
                    let cancelled_item = serde_json::json!({"type":"cancelled","reason": reason, "at": now_text});
                    let output = serde_json::json!({"text":"","items":[cancelled_item]});
                    let _ = sqlx::query(
                        r#"INSERT INTO session_tasks (id, session_name, created_by, status, input, output, timeout_seconds, timeout_at, created_at, updated_at)
                            VALUES (?, ?, ?, 'cancelled', ?, ?, NULL, NULL, NOW(), NOW())
                            ON DUPLICATE KEY UPDATE status='cancelled', output=VALUES(output), timeout_seconds=VALUES(timeout_seconds), timeout_at=VALUES(timeout_at), updated_at=NOW()"#
                    )
                    .bind(task_id)
                    .bind(&session_name)
                    .bind(&created_by)
                    .bind(&input)
                    .bind(&output)
                    .execute(&self.pool)
                    .await;
                    let _ = sqlx::query(r#"UPDATE session_requests SET status='completed', updated_at=NOW(), completed_at=NOW(), error='cancelled' WHERE id = ?"#)
                        .bind(&request_id)
                        .execute(&self.pool)
                        .await;
                }
            }
        }
        // Determine runtime: time from last open marker (or session.created_at if none)
        let recent_rows: Vec<(chrono::DateTime<Utc>, serde_json::Value)> = sqlx::query_as(
            r#"SELECT created_at, output FROM session_tasks WHERE session_name = ? ORDER BY created_at DESC LIMIT 50"#
        )
        .bind(&session_name)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();
        let mut start_ts = session_created_at;
        for (row_created_at, output) in recent_rows {
            if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
                let mut found = false;
                for it in items {
                    if it.get("type").and_then(|v| v.as_str()) == Some("restarted") {
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

        let marker = if is_task_timeout {
            serde_json::json!({
                "type": "cancelled",
                "note": note,
                "reason": reason,
                "by": created_by,
                "at": now_text,
                "runtime_seconds": runtime_seconds
            })
        } else {
            serde_json::json!({
                "type": "stopped",
                "note": note,
                "reason": reason,
                "by": created_by,
                "delay_seconds": delay_secs,
                "at": now_text,
                "runtime_seconds": runtime_seconds
            })
        };
        let output_json = serde_json::json!({
            "text": "",
            "items": [ marker ]
        });

        sqlx::query(
            r#"
            INSERT INTO session_tasks (id, session_name, created_by, status, input, output, timeout_seconds, timeout_at, created_at, updated_at)
            VALUES (?, ?, ?, 'completed', ?, ?, NULL, NULL, NOW(), NOW())
            "#,
        )
        .bind(&task_id)
        .bind(&session_name)
        .bind(&created_by)
        .bind(&serde_json::json!({"text": ""}))
        .bind(&output_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn handle_restart_session(&self, request: SessionRequest) -> Result<()> {
        let session_name = request.session_name;
        // Prefer explicitly provided principal/principal_type from payload (owner),
        // fall back to request.created_by as a regular subject.
        let effective_principal = request
            .payload
            .get("principal")
            .and_then(|v| v.as_str())
            .unwrap_or(&request.created_by)
            .to_string();
        let effective_principal_type = request
            .payload
            .get("principal_type")
            .and_then(|v| v.as_str())
            .unwrap_or("User")
            .to_string();

        info!("Restarting container for session {}", session_name);

        // Generate fresh tokens for restarted session
        info!(
            "Generating fresh tokens for restarted session {}",
            session_name
        );
        let restart_token = self
            .generate_session_token(
                &effective_principal,
                match effective_principal_type.as_str() {
                    "Admin" => SubjectType::Admin,
                    _ => SubjectType::Subject,
                },
                &session_name,
            )
            .map_err(|e| anyhow::anyhow!("Failed to generate restart session token: {}", e))?;

        // All restarted sessions were stopped (container destroyed), so recreate container
        info!(
            "Session {} was stopped, restarting container with persistent volume and fresh tokens",
            session_name
        );
        self.docker_manager
            .restart_container_with_tokens(
                &session_name,
                restart_token,
                effective_principal.clone(),
                effective_principal_type.clone(),
                request.created_at,
            )
            .await?;

        // Update last_activity_at and clear idle_from/busy_from since session is being restarted (will set to idle later)
        sqlx::query(
            r#"UPDATE sessions SET last_activity_at = NOW(), idle_from = NULL, busy_from = NULL WHERE name = ?"#,
        )
        .bind(&session_name)
        .execute(&self.pool)
        .await?;

        info!("Container restarted for session {}", session_name);

        // Send prompt if provided
        if let Some(prompt) = request.payload.get("prompt").and_then(|v| v.as_str()) {
            info!(
                "Sending prompt to restarted session {}: {}",
                session_name, prompt
            );

            // Get the principal name from the request
            let principal = effective_principal.clone();

            // Create task record in database for restarted session
            let task_id = uuid::Uuid::new_v4().to_string();
            let input_json = serde_json::json!({ "text": prompt });
            let output_json = serde_json::json!({ "text": "", "items": [] });
            sqlx::query(
                r#"
                INSERT INTO session_tasks (id, session_name, created_by, status, input, output, timeout_seconds, timeout_at, created_at, updated_at)
                VALUES (?, ?, ?, 'pending', ?, ?, NULL, NULL, NOW(), NOW())
                "#,
            )
            .bind(&task_id)
            .bind(&session_name)
            .bind(&principal)
            .bind(&input_json)
            .bind(&output_json)
            .execute(&self.pool)
            .await?;
            info!(
                "Prompt task {} created for rerestarted session {}",
                task_id, session_name
            );
        }

        // Insert a chat marker indicating the session has restarted
        let task_id = uuid::Uuid::new_v4().to_string();
        let now_text = chrono::Utc::now().to_rfc3339();
        let reason = request
            .payload
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("user_restart");
        let note = if reason == "user_restart" {
            "User open"
        } else {
            "Open"
        };
        let output_json = serde_json::json!({
            "text": "",
            "items": [ { "type": "restarted", "note": note, "reason": reason, "by": effective_principal, "at": now_text } ]
        });
        sqlx::query(
            r#"
            INSERT INTO session_tasks (id, session_name, created_by, status, input, output, timeout_seconds, timeout_at, created_at, updated_at)
            VALUES (?, ?, ?, 'completed', ?, ?, NULL, NULL, ?, ?)
            "#,
        )
        .bind(&task_id)
        .bind(&session_name)
        .bind(&effective_principal)
        .bind(&serde_json::json!({"text":""}))
        .bind(&output_json)
        .bind(&request.created_at)
        .bind(&request.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn handle_create_task(&self, request: SessionRequest) -> Result<()> {
        let session_name = request.session_name.clone();
        let principal = request.created_by.clone();

        info!("Handling create_task for session {}", session_name);

        // Parse payload
        let task_id = request
            .payload
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing task_id in payload"))?;
        let input = request
            .payload
            .get("input")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"text":""}));
        let restart_if_stopped = request
            .payload
            .get("restart_if_stopped")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let timeout_seconds = request
            .payload
            .get("timeout_seconds")
            .and_then(|v| v.as_i64())
            .and_then(|v| i32::try_from(v).ok())
            .filter(|v| *v > 0);

        // Inspect session state
        let state_opt: Option<(String,)> =
            sqlx::query_as(r#"SELECT state FROM sessions WHERE name = ?"#)
                .bind(&session_name)
                .fetch_optional(&self.pool)
                .await?;
        let state = state_opt.map(|t| t.0).unwrap_or_default();

        // Restart if needed
        if restart_if_stopped && state == "stopped" {
            info!(
                "Session {} stopped; restarting prior to inserting task entry",
                session_name
            );
            let restart_token = self
                .generate_session_token(&principal, SubjectType::Subject, &session_name)
                .map_err(|e| anyhow::anyhow!("Failed to generate restart session token: {}", e))?;
            self.docker_manager
                .restart_container_with_tokens(
                    &session_name,
                    restart_token,
                    principal.clone(),
                    "User".to_string(),
                    request.created_at,
                )
                .await?;
            sqlx::query(
                r#"UPDATE sessions SET last_activity_at = NOW(), idle_from = NULL, busy_from = NULL WHERE name = ?"#,
            )
            .bind(&session_name)
            .execute(&self.pool)
            .await?;

            // Insert a restart marker for implicit restarts
            let marker_id = uuid::Uuid::new_v4().to_string();
            let now_text = chrono::Utc::now().to_rfc3339();
            // Ensure the restart marker sorts before the newly created task row by using a slightly earlier timestamp
            let marker_created_at = request
                .created_at
                .checked_sub_signed(chrono::Duration::milliseconds(1))
                .unwrap_or(request.created_at);
            let output_json = serde_json::json!({
                "text": "",
                "items": [ { "type": "restarted", "note": "Incoming request", "reason": "implicit_restart", "by": principal, "at": now_text } ]
            });
            sqlx::query(
                r#"
                INSERT INTO session_tasks (id, session_name, created_by, status, input, output, timeout_seconds, timeout_at, created_at, updated_at)
                VALUES (?, ?, ?, 'completed', ?, ?, NULL, NULL, ?, ?)
                "#,
            )
            .bind(&marker_id)
            .bind(&session_name)
            .bind(&principal)
            .bind(&serde_json::json!({"text":""}))
            .bind(&output_json)
            .bind(&marker_created_at)
            .bind(&marker_created_at)
            .execute(&self.pool)
            .await?;
        }

        // If a task with this id already exists (e.g., pre-insert cancel), skip insertion
        if let Some((_existing_id, existing_status)) = sqlx::query_as::<_, (String, String)>(
            r#"SELECT id, status FROM session_tasks WHERE id = ?"#,
        )
        .bind(&task_id)
        .fetch_optional(&self.pool)
        .await?
        {
            info!(
                "Task {} already exists with status {}, skipping insert",
                task_id, existing_status
            );
            return Ok(());
        }

        // Insert task row
        // To avoid identical timestamps with the implicit open marker (second-level precision
        // in MySQL DATETIME), create the task entry one second after the request's created_at.
        let output_json = serde_json::json!({ "text": "", "items": [] });
        let task_created_at = request
            .created_at
            .checked_add_signed(chrono::Duration::seconds(1))
            .unwrap_or(request.created_at);
        let timeout_at = timeout_seconds.and_then(|secs| {
            task_created_at.checked_add_signed(chrono::Duration::seconds(secs as i64))
        });

        sqlx::query(
            r#"
            INSERT INTO session_tasks (id, session_name, created_by, status, input, output, timeout_seconds, timeout_at, created_at, updated_at)
            VALUES (?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&task_id)
        .bind(&session_name)
        .bind(&principal)
        .bind(&input)
        .bind(&output_json)
        .bind(timeout_seconds)
        .bind(timeout_at)
        .bind(&task_created_at)
        .bind(&task_created_at)
        .execute(&self.pool)
        .await?;
        info!("Inserted task {} for session {}", task_id, session_name);

        Ok(())
    }

    /// Check health of all non-stopped sessions and mark failed containers as stopped
    async fn check_session_health(&self) -> Result<usize> {
        // Find all sessions that are not stopped (active sessions)
        let active_sessions: Vec<(String, String)> = sqlx::query_as(
            r#"
            SELECT name, state
            FROM sessions
            WHERE state != 'stopped'
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        if active_sessions.is_empty() {
            return Ok(0);
        }

        info!(
            "Checking health of {} active sessions",
            active_sessions.len()
        );
        let mut recovered_count = 0;

        for (session_name, current_state) in active_sessions {
            // Check if container exists and is running
            match self
                .docker_manager
                .is_container_healthy(&session_name)
                .await
            {
                Ok(true) => {
                    // Container is healthy, no action needed
                    continue;
                }
                Ok(false) => {
                    // Container is unhealthy or doesn't exist
                    warn!(
                        "Session {} container is unhealthy or missing, marking as stopped for recovery",
                        session_name
                    );

                    // Mark session as stopped so it can be restarted later
                    if let Err(e) =
                        sqlx::query(r#"UPDATE sessions SET state = 'stopped' WHERE name = ?"#)
                            .bind(&session_name)
                            .execute(&self.pool)
                            .await
                    {
                        error!(
                            "Failed to mark unhealthy session {} as stopped: {}",
                            session_name, e
                        );
                    } else {
                        info!(
                            "Session {} marked as stopped due to container failure (was: {})",
                            session_name, current_state
                        );
                        recovered_count += 1;
                    }
                }
                Err(e) => {
                    // Health check failed, likely Docker connection issues
                    error!(
                        "Health check failed for session {}: {}, will retry next cycle",
                        session_name, e
                    );
                }
            }
        }

        if recovered_count > 0 {
            info!(
                "Marked {} sessions as stopped due to container failures",
                recovered_count
            );
        }

        Ok(recovered_count)
    }

    fn sanitize_relative_path(&self, p: &str) -> Result<String> {
        let p = p.trim();
        if p.is_empty() {
            return Ok(String::new());
        }
        if p.starts_with('/') || p.contains('\0') {
            return Err(anyhow::anyhow!("invalid path"));
        }
        let mut parts = Vec::new();
        for seg in p.split('/') {
            if seg.is_empty() || seg == "." || seg == ".." {
                return Err(anyhow::anyhow!("invalid path"));
            }
            parts.push(seg);
        }
        Ok(parts.join("/"))
    }

    async fn complete_request_with_payload(
        &self,
        request_id: &str,
        mut payload: serde_json::Value,
        result: serde_json::Value,
    ) -> Result<()> {
        if let serde_json::Value::Object(ref mut map) = payload {
            map.insert("result".into(), result);
        }
        sqlx::query(
            r#"UPDATE session_requests SET payload = ?, status='completed', updated_at=NOW(), completed_at=NOW(), error=NULL WHERE id = ?"#
        )
        .bind(&payload)
        .bind(request_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn fail_request(&self, request_id: &str, msg: String) -> Result<()> {
        sqlx::query(
            r#"UPDATE session_requests SET status='failed', updated_at=NOW(), completed_at=NOW(), error=? WHERE id = ?"#
        )
        .bind(&msg)
        .bind(request_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn handle_file_read(&self, request: SessionRequest) -> Result<()> {
        let path = request
            .payload
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let safe = self
            .sanitize_relative_path(path)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        // Do not auto-open for file APIs; require running container
        match self
            .docker_manager
            .is_container_healthy(&request.session_name)
            .await
        {
            Ok(true) => {}
            _ => {
                return self
                    .fail_request(&request.id, "session is stopped".to_string())
                    .await;
            }
        }
        let full_path = format!("/session/{}", safe);
        // Get size and content type
        let (stat_code, stat_out, _stat_err) = self
            .docker_manager
            .exec_collect(
                &request.session_name,
                vec![
                    "/usr/bin/stat".into(),
                    "-c".into(),
                    "%s".into(),
                    full_path.clone(),
                ],
            )
            .await?;
        if stat_code != 0 {
            return self
                .fail_request(&request.id, "not found or invalid".to_string())
                .await;
        }
        let size: u64 = String::from_utf8_lossy(&stat_out)
            .trim()
            .parse()
            .unwrap_or(0);
        // Cap at 25MB
        const MAX_BYTES: u64 = 25 * 1024 * 1024;
        if size > MAX_BYTES {
            return self
                .fail_request(
                    &request.id,
                    format!("file too large ({} bytes > 25MB)", size),
                )
                .await;
        }
        let (code, stdout, stderr) = self
            .docker_manager
            .exec_collect(
                &request.session_name,
                vec!["/bin/cat".into(), full_path.clone()],
            )
            .await?;
        if code != 0 {
            return self
                .fail_request(&request.id, String::from_utf8_lossy(&stderr).to_string())
                .await;
        }
        let ct = guess_content_type(&safe);
        let content_b64 = base64::encode(&stdout);
        let result = serde_json::json!({
            "content_base64": content_b64,
            "content_type": ct,
            "size": size,
        });
        self.complete_request_with_payload(&request.id, request.payload.clone(), result)
            .await
    }

    pub async fn handle_file_metadata(&self, request: SessionRequest) -> Result<()> {
        let path = request
            .payload
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let safe = self
            .sanitize_relative_path(path)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        match self
            .docker_manager
            .is_container_healthy(&request.session_name)
            .await
        {
            Ok(true) => {}
            _ => {
                return self
                    .fail_request(&request.id, "session is stopped".to_string())
                    .await;
            }
        }
        let full_path = format!("/session/{}", safe);
        let fmt = "%F|%s|%a|%Y|%N";
        let (code, stdout, stderr) = self
            .docker_manager
            .exec_collect(
                &request.session_name,
                vec![
                    "/usr/bin/stat".into(),
                    "-c".into(),
                    fmt.into(),
                    full_path.clone(),
                ],
            )
            .await?;
        if code != 0 {
            return self
                .fail_request(&request.id, String::from_utf8_lossy(&stderr).to_string())
                .await;
        }
        let line = String::from_utf8_lossy(&stdout);
        let parts: Vec<&str> = line.trim().split('|').collect();
        if parts.len() < 4 {
            return self
                .fail_request(&request.id, "unexpected stat output".into())
                .await;
        }
        let kind_raw = parts[0].to_ascii_lowercase();
        let kind = if kind_raw.contains("regular") {
            "file"
        } else if kind_raw.contains("directory") {
            "dir"
        } else if kind_raw.contains("symbolic link") {
            "symlink"
        } else {
            &kind_raw
        };
        let size: u64 = parts[1].parse().unwrap_or(0);
        let mode = format!("{:0>4}", parts[2]);
        let mtime_epoch: i64 = parts[3].parse().unwrap_or(0);
        let mtime = chrono::Utc
            .timestamp_opt(mtime_epoch, 0)
            .single()
            .unwrap_or_else(|| chrono::Utc.timestamp_opt(0, 0).unwrap());
        let mut obj = serde_json::json!({ "kind": kind, "size": size, "mode": mode, "mtime": mtime.to_rfc3339() });
        if parts.len() >= 5 && kind == "symlink" {
            if let Some(idx) = parts[4].find("->") {
                let target = parts[4][idx + 2..].trim().trim_matches('\'');
                obj["link_target"] = serde_json::Value::String(target.to_string());
            }
        }
        self.complete_request_with_payload(&request.id, request.payload.clone(), obj)
            .await
    }

    pub async fn handle_file_list(&self, request: SessionRequest) -> Result<()> {
        let path = request
            .payload
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let offset = request
            .payload
            .get("offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let limit = request
            .payload
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(100)
            .min(500);
        let safe = if path.is_empty() {
            String::new()
        } else {
            self.sanitize_relative_path(path)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
        };
        match self
            .docker_manager
            .is_container_healthy(&request.session_name)
            .await
        {
            Ok(true) => {}
            _ => {
                return self
                    .fail_request(&request.id, "session is stopped".to_string())
                    .await;
            }
        }
        let base = if safe.is_empty() {
            "/session".to_string()
        } else {
            format!("/session/{}", safe)
        };

        // Print one record per line so parser can split by lines()
        let fmt = "%f|%y|%s|%m|%T@\n";
        let (code, stdout, stderr) = self
            .docker_manager
            .exec_collect(
                &request.session_name,
                vec![
                    "/usr/bin/find".into(),
                    base.clone(),
                    "-maxdepth".into(),
                    "1".into(),
                    "-mindepth".into(),
                    "1".into(),
                    "-printf".into(),
                    fmt.into(),
                    "-name".into(),
                    "*".into(),
                ],
            )
            .await?;
        if code != 0 {
            return self
                .fail_request(&request.id, String::from_utf8_lossy(&stderr).to_string())
                .await;
        }
        let mut entries: Vec<(String, String, u64, String, i64)> = Vec::new();
        for line in String::from_utf8_lossy(&stdout).lines() {
            let parts: Vec<&str> = line.trim().split('|').collect();
            if parts.len() < 5 {
                continue;
            }
            let name = parts[0].to_string();
            if name == "." || name == ".." {
                continue;
            }
            let kind = match parts[1] {
                "f" => "file",
                "d" => "dir",
                "l" => "symlink",
                other => other,
            }
            .to_string();
            let size: u64 = parts[2].parse().unwrap_or(0);
            let mode = format!("{:0>4}", parts[3]);
            let mtime_secs: i64 = parts[4]
                .split('.')
                .next()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0);
            entries.push((name, kind, size, mode, mtime_secs));
        }
        entries.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        let total = entries.len() as u64;
        let slice_start = offset as usize;
        let slice_end = (offset + limit).min(total) as usize;
        let mut list: Vec<serde_json::Value> = Vec::new();
        if slice_start < entries.len() {
            for (name, kind, size, mode, mtime_secs) in entries[slice_start..slice_end].iter() {
                let mtime = chrono::Utc
                    .timestamp_opt(*mtime_secs, 0)
                    .single()
                    .unwrap_or_else(|| chrono::Utc.timestamp_opt(0, 0).unwrap());
                list.push(serde_json::json!({"name": name, "kind": kind, "size": size, "mode": mode, "mtime": mtime.to_rfc3339()}));
            }
        }
        let next_offset = if (offset + limit) < total {
            Some(offset + limit)
        } else {
            None
        };
        let result = serde_json::json!({ "entries": list, "offset": offset, "limit": limit, "next_offset": next_offset, "total": total });
        self.complete_request_with_payload(&request.id, request.payload.clone(), result)
            .await
    }

    pub async fn handle_file_delete(&self, request: SessionRequest) -> Result<()> {
        let path = request
            .payload
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let safe = self
            .sanitize_relative_path(path)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        match self
            .docker_manager
            .is_container_healthy(&request.session_name)
            .await
        {
            Ok(true) => {}
            _ => {
                return self
                    .fail_request(&request.id, "session is stopped".to_string())
                    .await;
            }
        }
        let full_path = format!("/session/{}", safe);
        // Ensure it's a regular file
        let (stat_code, stat_out, _stat_err) = self
            .docker_manager
            .exec_collect(
                &request.session_name,
                vec![
                    "/usr/bin/stat".into(),
                    "-c".into(),
                    "%F".into(),
                    full_path.clone(),
                ],
            )
            .await?;
        if stat_code != 0 {
            return self
                .fail_request(&request.id, "not found".to_string())
                .await;
        }
        let kind = String::from_utf8_lossy(&stat_out).to_ascii_lowercase();
        if !kind.contains("regular file") {
            return self
                .fail_request(&request.id, "Path is not a file".to_string())
                .await;
        }
        let (rm_code, _out, err) = self
            .docker_manager
            .exec_collect(
                &request.session_name,
                vec!["/bin/rm".into(), "-f".into(), full_path.clone()],
            )
            .await?;
        if rm_code != 0 {
            return self
                .fail_request(&request.id, String::from_utf8_lossy(&err).to_string())
                .await;
        }
        let result = serde_json::json!({ "deleted": true, "path": safe });
        self.complete_request_with_payload(&request.id, request.payload.clone(), result)
            .await
    }
}

fn guess_content_type(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".html") || lower.ends_with(".htm") {
        "text/html; charset=utf-8"
    } else if lower.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if lower.ends_with(".js") {
        "application/javascript"
    } else if lower.ends_with(".json") {
        "application/json"
    } else if lower.ends_with(".md")
        || lower.ends_with(".txt")
        || lower.ends_with(".rs")
        || lower.ends_with(".py")
        || lower.ends_with(".ts")
        || lower.ends_with(".sh")
        || lower.ends_with(".yml")
        || lower.ends_with(".yaml")
        || lower.ends_with(".toml")
    {
        "text/plain; charset=utf-8"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "application/octet-stream"
    }
}
