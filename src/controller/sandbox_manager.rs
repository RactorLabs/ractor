use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
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
pub use constants::SANDBOX_STATE_INITIALIZING;

// Using external inference service via TSBX_INFERENCE_URL

#[path = "../shared/rbac.rs"]
pub mod rbac;
use rbac::{RbacClaims, SubjectType};

use super::docker_manager::DockerManager;
use super::shared_task::{extract_output_items, TaskType};

#[path = "../shared/models/sandbox.rs"]
pub mod sandbox_model;
use sandbox_model::Sandbox;

#[path = "../shared/models/snapshot.rs"]
pub mod snapshot_model;
use snapshot_model::{CreateSnapshotRequest, Snapshot};

#[path = "../shared/models/state_helpers.rs"]
pub mod state_helpers;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SandboxRequest {
    id: String,
    request_type: String,
    sandbox_id: String,
    created_by: String,
    payload: serde_json::Value,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    error: Option<String>,
}

pub struct SandboxManager {
    pool: Pool<MySql>,
    docker_manager: DockerManager,
    jwt_secret: String,
}

impl SandboxManager {
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
            let mut sandboxes_stopped = 0;
            if last_auto_stop.elapsed() >= Duration::from_secs(10) {
                sandboxes_stopped = match self.process_auto_stop().await {
                    Ok(count) => count,
                    Err(e) => {
                        error!("Error processing auto-stop: {}", e);
                        0
                    }
                };
                last_auto_stop = Instant::now();
            }

            // Check health every 10s
            let mut sandboxes_recovered = 0;
            if last_health.elapsed() >= Duration::from_secs(10) {
                sandboxes_recovered = match self.check_sandbox_health().await {
                    Ok(recovered) => recovered,
                    Err(e) => {
                        error!("Error checking sandbox health: {}", e);
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
                && sandboxes_stopped == 0
                && sandboxes_recovered == 0
                && tasks_cancelled == 0
            {
                sleep(Duration::from_millis(250)).await;
            }
        }
    }

    /// Ensure the sandbox container is running and healthy; wait up to timeout_secs
    pub async fn ensure_sandbox_running(&self, sandbox_id: &str, timeout_secs: u64) -> Result<()> {
        // Quick healthy check
        match self.docker_manager.is_container_healthy(sandbox_id).await {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => {
                tracing::warn!("health check error for {}: {}", sandbox_id, e);
            }
        }

        // Check if sandbox exists in DB
        if let Some((state,)) =
            sqlx::query_as::<_, (String,)>(r#"SELECT state FROM sandboxes WHERE id = ?"#)
                .bind(sandbox_id)
                .fetch_optional(&self.pool)
                .await?
        {
            if state.eq_ignore_ascii_case("terminated")
                || state.eq_ignore_ascii_case("terminating")
                || state.eq_ignore_ascii_case("deleted")
            {
                return Err(anyhow::anyhow!(
                    "Sandbox {} is terminated and cannot be used",
                    sandbox_id
                ));
            }
        } else {
            // No row; nothing we can do
            return Err(anyhow::anyhow!("Sandbox {} not found in DB", sandbox_id));
        }

        // Wait for healthy
        let mut waited = 0u64;
        let step = 500u64; // ms
        while waited / 1000 < timeout_secs {
            if let Ok(true) = self.docker_manager.is_container_healthy(sandbox_id).await {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(step)).await;
            waited += step;
        }
        Err(anyhow::anyhow!(
            "sandbox {} not ready in {}s",
            sandbox_id,
            timeout_secs
        ))
    }

    /// Proxy exec with stdout/stderr collection
    pub async fn exec_collect(
        &self,
        sandbox_id: &str,
        cmd: Vec<String>,
    ) -> Result<(i32, Vec<u8>, Vec<u8>)> {
        self.docker_manager.exec_collect(sandbox_id, cmd).await
    }
    /// Process sandboxes that need auto-stopping due to timeout
    async fn process_auto_stop(&self) -> Result<usize> {
        // Ensure all idle sandboxes have idle_from set
        let _ = sqlx::query(
            r#"
            UPDATE sandboxes
            SET idle_from = NOW()
            WHERE state = 'idle' AND idle_from IS NULL
            "#,
        )
        .execute(&self.pool)
        .await;

        // Ensure all busy sandboxes have busy_from set
        let _ = sqlx::query(
            r#"
            UPDATE sandboxes
            SET busy_from = NOW()
            WHERE state = 'busy' AND busy_from IS NULL
            "#,
        )
        .execute(&self.pool)
        .await;

        // Find sandboxes that need auto-stop due to idle timeout
        let sandboxes_to_stop: Vec<(String, String)> = sqlx::query_as(
            r#"
            SELECT id,
                   state
            FROM sandboxes
            WHERE (
                    state = 'idle'
                    AND idle_from IS NOT NULL
                    AND TIMESTAMPADD(SECOND, idle_timeout_seconds, idle_from) <= NOW()
                  )
               OR (
                    state = 'initializing'
                    AND TIMESTAMPADD(SECOND, idle_timeout_seconds, created_at) <= NOW()
                  )
            ORDER BY TIMESTAMPADD(
                     SECOND,
                     idle_timeout_seconds,
                     CASE
                         WHEN state = 'idle' THEN idle_from
                         ELSE created_at
                     END
                 ) ASC
            LIMIT 50
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to find sandboxes to auto-stop: {}", e))?;

        let mut stopped_count = 0;

        for (sandbox_id, state) in sandboxes_to_stop {
            let reason = if state == "initializing" {
                "startup timeout"
            } else {
                "idle timeout"
            };
            info!("Auto-stopping sandbox {} due to {}", sandbox_id, reason);

            // Create stop request for the sandbox
            let request_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(r#"
                INSERT INTO sandbox_requests (id, sandbox_id, request_type, created_by, payload, status)
                VALUES (?, ?, 'terminate_sandbox', 'system', '{"reason": "idle_timeout"}', 'pending')
                "#)
            .bind(&request_id)
            .bind(&sandbox_id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create auto-stop request for sandbox {}: {}", sandbox_id, e))?;

            info!(
                "Created auto-stop request {} for sandbox {}",
                request_id, sandbox_id
            );
            stopped_count += 1;
        }

        if stopped_count > 0 {
            info!("Scheduled {} sandboxes for auto-stop", stopped_count);
        }

        Ok(stopped_count)
    }

    async fn process_task_timeouts(&self) -> Result<usize> {
        let timed_out: Vec<(
            String,
            String,
            serde_json::Value,
            serde_json::Value,
            chrono::DateTime<Utc>,
        )> = sqlx::query_as(
            r#"
                SELECT id, sandbox_id, steps, output, created_at
                FROM sandbox_tasks
                WHERE timeout_at IS NOT NULL
                  AND timeout_at <= NOW()
                  AND status IN ('queued', 'processing')
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
        for (task_id, sandbox_id, steps_json, output_json, created_at) in timed_out {
            let now = chrono::Utc::now();
            let now_text = now.to_rfc3339();
            let runtime_seconds = (now - created_at).num_seconds();
            let runtime_seconds = if runtime_seconds < 0 {
                0
            } else {
                runtime_seconds
            };

            let marker_step = serde_json::json!({
                "type": "cancelled",
                "reason": "task_timeout",
                "at": now_text,
                "runtime_seconds": runtime_seconds
            });

            let mut steps_vec = steps_json.as_array().cloned().unwrap_or_else(Vec::new);
            steps_vec.push(marker_step);
            let updated_steps = serde_json::Value::Array(steps_vec);

            let mut output_items = extract_output_items(&output_json);
            output_items.push(serde_json::json!({
                "type": "text",
                "content": "Task timed out"
            }));
            let updated_output = serde_json::Value::Array(output_items);

            let update = sqlx::query(
                r#"
                UPDATE sandbox_tasks
                SET status = 'cancelled',
                    output = ?,
                    steps = ?,
                    timeout_seconds = NULL,
                    timeout_at = NULL,
                    updated_at = NOW()
                WHERE id = ? AND status IN ('queued','processing')
                "#,
            )
            .bind(&updated_output)
            .bind(&updated_steps)
            .bind(&task_id)
            .execute(&self.pool)
            .await?;

            if update.rows_affected() == 0 {
                continue;
            }

            cancelled += 1;

            sqlx::query(
                r#"
                UPDATE sandboxes
                SET state = 'idle',
                    busy_from = NULL,
                    idle_from = NOW(),
                    last_activity_at = NOW()
                WHERE id = ? AND state = 'busy'
                "#,
            )
            .bind(&sandbox_id)
            .execute(&self.pool)
            .await?;

            info!(
                "Cancelled task {} for sandbox {} due to per-task timeout",
                task_id, sandbox_id
            );
        }

        Ok(cancelled)
    }

    /// Generate a sandbox-specific TSBX token for the given principal
    fn generate_sandbox_token(
        &self,
        principal: &str,
        principal_type: SubjectType,
        sandbox_id: &str,
    ) -> Result<String> {
        let exp = chrono::Utc::now() + chrono::Duration::hours(24);
        let claims = RbacClaims {
            sub: principal.to_string(), // Use original principal name for API server compatibility
            sub_type: principal_type,
            exp: exp.timestamp() as usize,
            iat: chrono::Utc::now().timestamp() as usize,
            iss: "tsbx-sandbox-manager".to_string(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_ref()),
        )
        .map_err(|e| anyhow::anyhow!("Failed to generate sandbox token: {}", e))?;

        info!(
            "Generated sandbox token for principal: {} (sandbox ID: {})",
            principal, sandbox_id
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

    async fn fetch_pending_requests(&self) -> Result<Vec<SandboxRequest>> {
        // MySQL doesn't support RETURNING, so we need to do this in two steps
        // First, get and lock the pending requests
        let request_ids: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT id
            FROM sandbox_requests
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
            "UPDATE sandbox_requests SET status = 'processing', started_at = NOW(), updated_at = NOW() WHERE id IN ({placeholders})"
        );

        let mut query = sqlx::query(&query_str);
        for id in &ids {
            query = query.bind(id);
        }
        query.execute(&self.pool).await?;

        // Fetch the now-processing requests
        let query_str = format!("SELECT * FROM sandbox_requests WHERE id IN ({placeholders})");
        let mut query = sqlx::query_as::<_, SandboxRequest>(&query_str);
        for id in &ids {
            query = query.bind(id);
        }
        let requests = query.fetch_all(&self.pool).await?;

        Ok(requests)
    }

    async fn process_request(&self, request: SandboxRequest) -> Result<()> {
        info!(
            "Processing request {} of type {}",
            request.id, request.request_type
        );

        let result = match request.request_type.as_str() {
            "create_sandbox" => self.handle_start_sandbox(request.clone()).await,
            "terminate_sandbox" => self.handle_terminate_sandbox_request(request.clone()).await,
            "create_snapshot" => self.handle_create_snapshot(request.clone()).await,
            "execute_command" => self.handle_execute_command(request.clone()).await,
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

    pub async fn handle_start_sandbox(&self, request: SandboxRequest) -> Result<()> {
        // Look up the sandbox from the database using sandbox_id
        let sandbox = sqlx::query_as::<_, Sandbox>(
            "SELECT id, created_by, state, description, snapshot_id, created_at, last_activity_at,
             metadata, tags, inference_model, nl_task_enabled, idle_timeout_seconds, idle_from, busy_from,
             tokens_prompt, tokens_completion, tool_count,
             runtime_seconds, tasks_completed
             FROM sandboxes WHERE id = ?",
        )
        .bind(&request.sandbox_id)
        .fetch_one(&self.pool)
        .await?;

        // Parse the payload to get sandbox creation parameters
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

        let startup_task = request
            .payload
            .get("startup_task")
            .or_else(|| request.payload.get("prompt"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let inference_api_key = request
            .payload
            .get("inference_api_key")
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

        // Generate dynamic token for this sandbox (for TSBX auth)
        info!("Generating dynamic token for sandbox ID {}", &sandbox.id);
        let sandbox_token = self
            .generate_sandbox_token(principal, principal_type, &sandbox.id)
            .map_err(|e| anyhow::anyhow!("Failed to generate sandbox token: {}", e))?;

        info!(
            "Generated dynamic tokens for sandbox ID {} (principal: {})",
            &sandbox.id, principal
        );

        info!("Creating sandbox ID {} for principal {} ({:?}) with {} env, instructions: {}, setup: {}, startup_task: {}",
              &sandbox.id, principal, principal_type, env.len(), instructions.is_some(), setup.is_some(), startup_task.is_some());

        info!("Creating new sandbox {}", &sandbox.id);

        // Create container with sandbox parameters and generated tokens
        self.docker_manager
            .create_container_with_params_and_tokens(
                &sandbox.id,
                env,
                instructions,
                setup,
                sandbox_token,
                principal.to_string(),
                principal_type_str.to_string(),
                request.created_at,
                sandbox.inference_model.clone(),
                inference_api_key,
            )
            .await?;

        // Check if we need to restore from a snapshot
        if let Some(snapshot_id) = request.payload.get("snapshot_id").and_then(|v| v.as_str()) {
            info!(
                "Restoring snapshot {} to sandbox {}",
                snapshot_id, &sandbox.id
            );

            // Wait a moment for container to be fully ready
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            match self
                .docker_manager
                .restore_snapshot(&sandbox.id, snapshot_id)
                .await
            {
                Ok(_) => {
                    info!(
                        "Snapshot {} successfully restored to sandbox {}",
                        snapshot_id, &sandbox.id
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to restore snapshot {} to sandbox {}: {}",
                        snapshot_id, &sandbox.id, e
                    );
                    // Don't fail the entire creation - continue without the snapshot
                }
            }
        }

        // Send startup task if provided (BEFORE setting state to IDLE)
        if let Some(startup_task) = startup_task {
            info!(
                "Submitting startup task to sandbox ID {}: {}",
                &sandbox.id, startup_task
            );

            // Create task record in database (pending)
            let task_id = uuid::Uuid::new_v4().to_string();
            let input_json =
                serde_json::json!({ "content": [ { "type": "text", "content": startup_task } ] });
            let output_json = serde_json::json!([]);
            let steps_json = serde_json::json!([]);
            sqlx::query(
                r#"
                INSERT INTO sandbox_tasks (id, sandbox_id, created_by, status, task_type, input, output, steps, timeout_seconds, timeout_at, created_at, updated_at)
                VALUES (?, ?, ?, 'queued', 'NL', ?, ?, ?, ?, ?, NOW(), NOW())
                "#,
            )
            .bind(&task_id)
            .bind(&sandbox.id)
            .bind(&principal)
            .bind(&input_json)
            .bind(&output_json)
            .bind(&steps_json)
            .bind(Option::<i32>::None)
            .bind(Option::<chrono::DateTime<Utc>>::None)
            .execute(&self.pool)
            .await?;
            info!(
                "Startup task {} created for sandbox ID {}",
                task_id, &sandbox.id
            );
        }

        // Set sandbox state to INIT after container creation only if it hasn't changed yet.
        // This avoids overwriting a sandbox that already set itself to IDLE.
        sqlx::query(r#"UPDATE sandboxes SET state = ?, last_activity_at = NOW() WHERE id = ? AND state = 'initializing'"#)
            .bind(SANDBOX_STATE_INITIALIZING)
            .bind(&sandbox.id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn handle_execute_command(&self, request: SandboxRequest) -> Result<()> {
        let command = request.payload["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing command in payload"))?;

        info!(
            "Executing command in sandbox ID {}: {}",
            &request.sandbox_id, command
        );
        let _output = self
            .docker_manager
            .execute_command(&request.sandbox_id, command)
            .await?;

        // Note: command_results table does not exist in schema
        // If command result tracking is needed, add migration to create:
        // CREATE TABLE command_results (
        //   id CHAR(36) PRIMARY KEY,
        //   sandbox_id CHAR(36) NOT NULL,
        //   command TEXT NOT NULL,
        //   output TEXT,
        //   created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
        //   CONSTRAINT fk_command_results_sandbox FOREIGN KEY (sandbox_id) REFERENCES sandboxes(id) ON DELETE CASCADE
        // )

        Ok(())
    }

    async fn mark_request_completed(&self, request_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sandbox_requests
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
            UPDATE sandbox_requests
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

    pub async fn handle_terminate_sandbox_request(&self, request: SandboxRequest) -> Result<()> {
        // Look up the sandbox from the database using sandbox_id
        let sandbox = sqlx::query_as::<_, Sandbox>(
            "SELECT id, created_by, state, description, snapshot_id, created_at, last_activity_at,
             metadata, tags, inference_model, nl_task_enabled, idle_timeout_seconds, idle_from, busy_from,
             tokens_prompt, tokens_completion, tool_count,
             runtime_seconds, tasks_completed
             FROM sandboxes WHERE id = ?",
        )
        .bind(&request.sandbox_id)
        .fetch_one(&self.pool)
        .await?;

        // Optional delay before closing (in seconds), minimum 5 seconds
        let delay_secs = request
            .payload
            .get("delay_seconds")
            .and_then(|v| v.as_u64())
            .map(|d| if d < 5 { 5 } else { d })
            .unwrap_or(5);
        if delay_secs > 0 {
            info!(
                "Delaying close for sandbox ID {} by {} seconds",
                &sandbox.id, delay_secs
            );
            sleep(Duration::from_secs(delay_secs)).await;
        }
        // Capture prior state and created_at for runtime measurement
        let sandbox_row_opt: Option<(chrono::DateTime<Utc>, String)> =
            sqlx::query_as(r#"SELECT created_at, state FROM sandboxes WHERE id = ?"#)
                .bind(&sandbox.id)
                .fetch_optional(&self.pool)
                .await?;
        let (_created_at, prior_state) = sandbox_row_opt
            .map(|(c, s)| (c, s))
            .unwrap_or((chrono::Utc::now(), String::new()));

        // Determine note: auto timeout vs user-triggered
        let auto = request.payload.get("reason").and_then(|v| v.as_str()) == Some("idle_timeout");
        let reason = if auto {
            if prior_state.to_lowercase() == "busy" {
                "task_timeout"
            } else {
                "idle_timeout"
            }
        } else {
            "user"
        };
        let is_task_timeout = reason == "task_timeout";

        if is_task_timeout {
            info!(
                "Task timeout detected for sandbox ID {} – cancelling task and returning to idle",
                &sandbox.id
            );
            sqlx::query(
                r#"UPDATE sandboxes SET state = 'idle', busy_from = NULL, idle_from = NOW(), last_activity_at = NOW() WHERE id = ?"#,
            )
            .bind(&sandbox.id)
            .execute(&self.pool)
            .await?;
        } else {
            info!("Stopping sandbox {}", &sandbox.id);

            // Create snapshot before stopping (graceful failure - don't block stop)
            let snapshot_id = uuid::Uuid::new_v4().to_string();
            match self
                .docker_manager
                .create_snapshot(&sandbox.id, &snapshot_id)
                .await
            {
                Ok(_) => {
                    info!(
                        "Snapshot {} created for sandbox {}",
                        snapshot_id, &sandbox.id
                    );

                    // Record snapshot in database
                    let snapshot_req = CreateSnapshotRequest {
                        metadata: serde_json::json!({
                            "trigger": "sandbox_stop",
                            "stopped_at": chrono::Utc::now().to_rfc3339()
                        }),
                    };

                    match Snapshot::create_with_id(
                        &self.pool,
                        &sandbox.id,
                        "termination",
                        snapshot_req,
                        Some(snapshot_id.clone()),
                    )
                    .await
                    {
                        Ok(_) => info!("Snapshot {} recorded in database", snapshot_id),
                        Err(e) => warn!(
                            "Failed to record snapshot {} in database: {}",
                            snapshot_id, e
                        ),
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to create snapshot for sandbox {}: {}",
                        &sandbox.id, e
                    );
                }
            }

            // Close the Docker container (no individual volumes to keep)
            self.docker_manager.stop_container(&sandbox.id).await?;

            // Update sandbox state to terminated (sandbox no longer active)
            sqlx::query(r#"UPDATE sandboxes SET state = 'terminated' WHERE id = ?"#)
                .bind(&sandbox.id)
                .execute(&self.pool)
                .await?;

            info!("Sandbox {} state updated to terminated", &sandbox.id);
        }

        let now_text = chrono::Utc::now().to_rfc3339();
        let _note = if auto {
            if reason == "task_timeout" {
                "Task timeout"
            } else {
                "Idle timeout"
            }
        } else {
            request
                .payload
                .get("note")
                .and_then(|v| v.as_str())
                .unwrap_or("User requested terminate")
        };

        // Mark the latest in-progress task as cancelled (processing or pending) (applies to any close reason)
        if let Some((task_id, steps_json, output_json)) =
            sqlx::query_as::<_, (String, serde_json::Value, serde_json::Value)>(
                r#"SELECT id, steps, output FROM sandbox_tasks WHERE sandbox_id = ? AND status IN ('processing','queued') ORDER BY created_at DESC LIMIT 1"#,
            )
            .bind(&sandbox.id)
            .fetch_optional(&self.pool)
            .await?
        {
            let mut steps_array = steps_json
                .as_array()
                .cloned()
                .unwrap_or_else(Vec::new);
            steps_array.push(serde_json::json!({
                "type": "cancelled",
                "reason": reason,
                "at": now_text,
            }));

            let updated_steps = serde_json::Value::Array(steps_array);
            let mut output_items = extract_output_items(&output_json);
            output_items.push(serde_json::json!({
                "type": "text",
                "content": format!("Task cancelled ({})", reason)
            }));
            let updated_output = serde_json::Value::Array(output_items);
            // Update task status to 'cancelled'
            let _ = sqlx::query(
                r#"UPDATE sandbox_tasks SET status = 'cancelled', output = ?, steps = ?, updated_at = NOW() WHERE id = ?"#,
            )
            .bind(&updated_output)
            .bind(&updated_steps)
            .bind(&task_id)
            .execute(&self.pool)
            .await;
        }

        let _ = sqlx::query(
            r#"UPDATE sandbox_requests
               SET status='completed', updated_at=NOW(), completed_at=NOW(), error='cancelled'
               WHERE sandbox_id = ? AND request_type = 'create_task' AND status IN ('pending','processing')"#,
        )
        .bind(&sandbox.id)
        .execute(&self.pool)
        .await;

        // Determine runtime: time from last open marker (or sandbox.created_at if none)
        let recent_rows: Vec<(chrono::DateTime<Utc>, serde_json::Value)> = sqlx::query_as(
            r#"SELECT created_at, steps FROM sandbox_tasks WHERE sandbox_id = ? ORDER BY created_at DESC LIMIT 50"#
        )
        .bind(&sandbox.id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();
        let mut start_ts = sandbox.created_at;
        for (row_created_at, output) in recent_rows {
            if let Some(items) = output.as_array() {
                if items
                    .iter()
                    .any(|it| it.get("type").and_then(|v| v.as_str()) == Some("restarted"))
                {
                    start_ts = row_created_at;
                    break;
                }
            }
        }
        let now = chrono::Utc::now();
        let mut runtime_seconds = (now - start_ts).num_seconds();
        if runtime_seconds < 0 {
            runtime_seconds = 0;
        }

        info!(
            "Sandbox {} terminated (reason: {}, runtime {}s)",
            sandbox.id, reason, runtime_seconds
        );

        Ok(())
    }

    pub async fn handle_create_snapshot(&self, request: SandboxRequest) -> Result<()> {
        let sandbox = sqlx::query_as::<_, Sandbox>(
            "SELECT id, created_by, state, description, snapshot_id, created_at, last_activity_at,
             metadata, tags, inference_model, nl_task_enabled, idle_timeout_seconds, idle_from, busy_from,
             tokens_prompt, tokens_completion, tool_count,
             runtime_seconds, tasks_completed
             FROM sandboxes WHERE id = ?",
        )
        .bind(&request.sandbox_id)
        .fetch_one(&self.pool)
        .await?;

        if sandbox.state.eq_ignore_ascii_case("terminated")
            || sandbox.state.eq_ignore_ascii_case("terminating")
            || sandbox.state.eq_ignore_ascii_case("initializing")
            || sandbox.state.eq_ignore_ascii_case("deleted")
        {
            return self
                .fail_request(&request.id, "sandbox not available".to_string())
                .await;
        }

        let snapshot_id = request
            .payload
            .get("snapshot_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing snapshot_id in payload"))?
            .to_string();

        let metadata = request
            .payload
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        let trigger_type = request
            .payload
            .get("trigger_type")
            .and_then(|v| v.as_str())
            .unwrap_or("manual")
            .to_string();

        match self.docker_manager.is_container_healthy(&sandbox.id).await {
            Ok(true) => {}
            _ => {
                return self
                    .fail_request(&request.id, "sandbox not available".to_string())
                    .await;
            }
        }

        if let Err(e) = self
            .docker_manager
            .create_snapshot(&sandbox.id, &snapshot_id)
            .await
        {
            return self
                .fail_request(
                    &request.id,
                    format!("failed to create snapshot {}: {}", snapshot_id, e),
                )
                .await;
        }

        let snapshot_req = CreateSnapshotRequest { metadata };
        let snapshot = Snapshot::create_with_id(
            &self.pool,
            &sandbox.id,
            &trigger_type,
            snapshot_req,
            Some(snapshot_id.clone()),
        )
        .await?;

        let result = serde_json::json!({
            "snapshot": snapshot,
        });

        self.complete_request_with_payload(&request.id, request.payload.clone(), result)
            .await
    }

    pub async fn handle_create_task(&self, request: SandboxRequest) -> Result<()> {
        // Look up the sandbox from the database using sandbox_id
        let sandbox = sqlx::query_as::<_, Sandbox>(
            "SELECT id, created_by, state, description, snapshot_id, created_at, last_activity_at,
             metadata, tags, inference_model, nl_task_enabled, idle_timeout_seconds, idle_from, busy_from,
             tokens_prompt, tokens_completion, tool_count,
             runtime_seconds, tasks_completed
             FROM sandboxes WHERE id = ?",
        )
        .bind(&request.sandbox_id)
        .fetch_one(&self.pool)
        .await?;

        let principal = request.created_by.clone();

        info!("Handling create_task for sandbox ID {}", &sandbox.id);

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
        let task_type = request
            .payload
            .get("task_type")
            .and_then(|v| v.as_str())
            .map(TaskType::from_db_value)
            .unwrap_or(TaskType::NL);

        if task_type == TaskType::NL && !sandbox.nl_task_enabled {
            warn!(
                "Sandbox {} cannot accept NL tasks because no inference key was provided",
                sandbox.id
            );
            anyhow::bail!("NL tasks disabled for sandbox without inference key");
        }

        let timeout_seconds = request
            .payload
            .get("timeout_seconds")
            .and_then(|v| v.as_i64())
            .and_then(|v| i32::try_from(v).ok())
            .filter(|v| *v > 0);

        // If a task with this id already exists (e.g., pre-insert cancel), skip insertion
        if let Some((_existing_id, existing_status)) = sqlx::query_as::<_, (String, String)>(
            r#"SELECT id, status FROM sandbox_tasks WHERE id = ?"#,
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
        let output_json = serde_json::json!([]);
        let steps_json = serde_json::json!([]);
        let task_created_at = request
            .created_at
            .checked_add_signed(chrono::Duration::seconds(1))
            .unwrap_or(request.created_at);
        let timeout_at = timeout_seconds.and_then(|secs| {
            task_created_at.checked_add_signed(chrono::Duration::seconds(secs as i64))
        });

        sqlx::query(
            r#"
            INSERT INTO sandbox_tasks (id, sandbox_id, created_by, status, task_type, input, output, steps, timeout_seconds, timeout_at, created_at, updated_at)
            VALUES (?, ?, ?, 'queued', ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&task_id)
        .bind(&sandbox.id)
        .bind(&principal)
        .bind(task_type.as_str())
        .bind(&input)
        .bind(&output_json)
        .bind(&steps_json)
        .bind(timeout_seconds)
        .bind(timeout_at)
        .bind(&task_created_at)
        .bind(&task_created_at)
        .execute(&self.pool)
        .await?;
        info!("Inserted task {} for sandbox ID {}", task_id, &sandbox.id);

        Ok(())
    }

    /// Check health of all non-terminated sandboxes and mark failed containers as terminated
    async fn check_sandbox_health(&self) -> Result<usize> {
        // Find all sandboxes that are not terminated (active sandboxes)
        let active_sandboxes: Vec<(String, String)> = sqlx::query_as(
            r#"
            SELECT id, state
            FROM sandboxes
            WHERE state NOT IN ('terminated', 'terminating', 'deleted', 'initializing')
            ORDER BY id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        if active_sandboxes.is_empty() {
            return Ok(0);
        }

        info!(
            "Checking health of {} active sandboxes",
            active_sandboxes.len()
        );
        let mut recovered_count = 0;

        for (sandbox_id, current_state) in active_sandboxes {
            // Check if container exists and is running
            match self.docker_manager.is_container_healthy(&sandbox_id).await {
                Ok(true) => {
                    // Container is healthy, no action needed
                    continue;
                }
                Ok(false) => {
                    // Container is unhealthy or doesn't exist
                    warn!(
                        "Sandbox ID {} container is unhealthy or missing, marking as terminated for recovery",
                        sandbox_id
                    );

                    // Mark sandbox as terminated so it can be restarted later
                    if let Err(e) =
                        sqlx::query(r#"UPDATE sandboxes SET state = 'terminated' WHERE id = ?"#)
                            .bind(&sandbox_id)
                            .execute(&self.pool)
                            .await
                    {
                        error!(
                            "Failed to mark unhealthy sandbox ID {} as terminated: {}",
                            sandbox_id, e
                        );
                    } else {
                        info!(
                            "Sandbox ID {} marked as terminated due to container failure (was: {})",
                            sandbox_id, current_state
                        );
                        recovered_count += 1;
                    }
                }
                Err(e) => {
                    // Health check failed, likely Docker connection issues
                    error!(
                        "Health check failed for sandbox ID {}: {}, will retry next cycle",
                        sandbox_id, e
                    );
                }
            }
        }

        if recovered_count > 0 {
            info!(
                "Marked {} sandboxes as terminated due to container failures",
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
            r#"UPDATE sandbox_requests SET payload = ?, status='completed', updated_at=NOW(), completed_at=NOW(), error=NULL WHERE id = ?"#
        )
        .bind(&payload)
        .bind(request_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn fail_request(&self, request_id: &str, msg: String) -> Result<()> {
        sqlx::query(
            r#"UPDATE sandbox_requests SET status='failed', updated_at=NOW(), completed_at=NOW(), error=? WHERE id = ?"#
        )
        .bind(&msg)
        .bind(request_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn handle_file_read(&self, request: SandboxRequest) -> Result<()> {
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
            .is_container_healthy(&request.sandbox_id)
            .await
        {
            Ok(true) => {}
            _ => {
                return self
                    .fail_request(&request.id, "sandbox not available".to_string())
                    .await;
            }
        }
        let full_path = format!("/sandbox/{}", safe);
        // Get size and content type
        let (stat_code, stat_out, _stat_err) = self
            .docker_manager
            .exec_collect(
                &request.sandbox_id,
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
                &request.sandbox_id,
                vec!["/bin/cat".into(), full_path.clone()],
            )
            .await?;
        if code != 0 {
            return self
                .fail_request(&request.id, String::from_utf8_lossy(&stderr).to_string())
                .await;
        }
        let ct = guess_content_type(&safe);
        let content_b64 = BASE64_STANDARD.encode(&stdout);
        let result = serde_json::json!({
            "content_base64": content_b64,
            "content_type": ct,
            "size": size,
        });
        self.complete_request_with_payload(&request.id, request.payload.clone(), result)
            .await
    }

    pub async fn handle_file_metadata(&self, request: SandboxRequest) -> Result<()> {
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
            .is_container_healthy(&request.sandbox_id)
            .await
        {
            Ok(true) => {}
            _ => {
                return self
                    .fail_request(&request.id, "sandbox not available".to_string())
                    .await;
            }
        }
        let full_path = format!("/sandbox/{}", safe);
        let fmt = "%F|%s|%a|%Y|%N";
        let (code, stdout, stderr) = self
            .docker_manager
            .exec_collect(
                &request.sandbox_id,
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

    pub async fn handle_file_list(&self, request: SandboxRequest) -> Result<()> {
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
            .is_container_healthy(&request.sandbox_id)
            .await
        {
            Ok(true) => {}
            _ => {
                return self
                    .fail_request(&request.id, "sandbox not available".to_string())
                    .await;
            }
        }
        let base = if safe.is_empty() {
            "/sandbox".to_string()
        } else {
            format!("/sandbox/{}", safe)
        };

        // Print one record per line so parser can split by lines()
        let fmt = "%f|%y|%s|%m|%T@\n";
        let (code, stdout, stderr) = self
            .docker_manager
            .exec_collect(
                &request.sandbox_id,
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

    pub async fn handle_file_delete(&self, request: SandboxRequest) -> Result<()> {
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
            .is_container_healthy(&request.sandbox_id)
            .await
        {
            Ok(true) => {}
            _ => {
                return self
                    .fail_request(&request.id, "sandbox not available".to_string())
                    .await;
            }
        }
        let full_path = format!("/sandbox/{}", safe);
        // Ensure it's a regular file
        let (stat_code, stat_out, _stat_err) = self
            .docker_manager
            .exec_collect(
                &request.sandbox_id,
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
                &request.sandbox_id,
                vec!["/bin/rm".into(), "-f".into(), full_path.clone()],
            )
            .await?;
        if rm_code != 0 {
            return self
                .fail_request(&request.id, String::from_utf8_lossy(&err).to_string())
                .await;
        }
        let result = serde_json::json!({ "terminated": true, "path": safe });
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
