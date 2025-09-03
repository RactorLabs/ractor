use anyhow::Result;
use bollard::Docker;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{mysql::MySqlPoolOptions, Pool, MySql};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};
use jsonwebtoken::{encode, EncodingKey, Header};

// Import constants from shared module
#[path = "../shared/models/constants.rs"]
pub mod constants;
pub use constants::SESSION_STATE_INIT;

// Import shared modules
#[path = "../shared/anthropic.rs"]
pub mod anthropic;
use anthropic::AnthropicKeyManager;

#[path = "../shared/rbac.rs"]
pub mod rbac;
use rbac::{RbacClaims, SubjectType};

use super::docker_manager::DockerManager;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SessionTask {
    id: String,
    task_type: String,
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
    key_manager: AnthropicKeyManager,
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
        
        let key_manager = AnthropicKeyManager::new()
            .map_err(|e| anyhow::anyhow!("Failed to initialize Anthropic key manager: {}", e))?;
        
        let jwt_secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "default-secret-change-in-production".to_string());

        Ok(Self {
            pool,
            docker_manager,
            key_manager,
            jwt_secret,
        })
    }

    pub async fn run(&self) -> Result<()> {
        info!("Session Manager started, polling for tasks and auto-close monitoring...");

        loop {
            // Process pending tasks
            let tasks_processed = match self.process_pending_tasks().await {
                Ok(processed) => processed,
                Err(e) => {
                    error!("Error processing tasks: {}", e);
                    0
                }
            };

            // Process auto-close monitoring
            let sessions_closed = match self.process_auto_close().await {
                Ok(closed) => closed,
                Err(e) => {
                    error!("Error processing auto-close: {}", e);
                    0
                }
            };

            // If no work was done, sleep before next iteration
            if tasks_processed == 0 && sessions_closed == 0 {
                sleep(Duration::from_secs(10)).await;
            }
        }
    }

    /// Generate a session-specific API key for Anthropic
    async fn generate_session_api_key(&self, session_name: &str) -> Result<String> {
        self.key_manager.generate_session_api_key(session_name).await
    }

    /// Process sessions that need auto-closing due to timeout
    async fn process_auto_close(&self) -> Result<usize> {
        // Find idle sessions that have passed their auto_close_at time
        let sessions_to_close: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT name
            FROM sessions
            WHERE auto_close_at <= NOW() 
              AND auto_close_at IS NOT NULL
              AND state = 'idle'
              AND state != 'deleted'
            ORDER BY auto_close_at ASC
            LIMIT 50
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to find sessions to auto-close: {}", e))?;
        
        let mut closed_count = 0;
        
        for (session_name,) in sessions_to_close {
            info!("Auto-closing session {} due to timeout", session_name);
            
            // Create close task for the session
            let task_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(r#"
                INSERT INTO session_tasks (id, session_name, task_type, created_by, payload, status)
                VALUES (?, ?, 'close_session', 'system', '{"reason": "auto_close_timeout"}', 'pending')
                "#)
            .bind(&task_id)
            .bind(&session_name)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create auto-close task for session {}: {}", session_name, e))?;
            
            info!("Created auto-close task {} for session {}", task_id, session_name);
            closed_count += 1;
        }
        
        if closed_count > 0 {
            info!("Scheduled {} sessions for auto-close", closed_count);
        }
        
        Ok(closed_count)
    }

    /// Generate a session-specific RAWORC token for the given principal
    fn generate_session_token(&self, principal: &str, principal_type: SubjectType, session_name: &str) -> Result<String> {
        let exp = chrono::Utc::now() + chrono::Duration::hours(24);
        let claims = RbacClaims {
            sub: principal.to_string(), // Use original principal name for API server compatibility
            sub_type: principal_type,
            exp: exp.timestamp() as usize,
            iat: chrono::Utc::now().timestamp() as usize,
            iss: "raworc-session-manager".to_string(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_ref()),
        ).map_err(|e| anyhow::anyhow!("Failed to generate session token: {}", e))?;

        info!("Generated session token for principal: {} (session: {})", principal, session_name);
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

    async fn fetch_pending_tasks(&self) -> Result<Vec<SessionTask>> {
        // MySQL doesn't support RETURNING, so we need to do this in two steps
        // First, get and lock the pending tasks
        let task_ids: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT id
            FROM session_tasks
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
            "UPDATE session_tasks SET status = 'processing', started_at = NOW(), updated_at = NOW() WHERE id IN ({placeholders})"
        );
        
        let mut query = sqlx::query(&query_str);
        for id in &ids {
            query = query.bind(id);
        }
        query.execute(&self.pool).await?;

        // Fetch the updated tasks
        let query_str = format!(
            "SELECT * FROM session_tasks WHERE id IN ({placeholders})"
        );
        let mut query = sqlx::query_as::<_, SessionTask>(&query_str);
        for id in &ids {
            query = query.bind(id);
        }
        let tasks = query.fetch_all(&self.pool).await?;

        Ok(tasks)
    }

    async fn process_task(&self, task: SessionTask) -> Result<()> {
        info!("Processing task {} of type {}", task.id, task.task_type);

        let result = match task.task_type.as_str() {
            "create_session" => self.handle_create_session(task.clone()).await,
            "destroy_session" => self.handle_destroy_session(task.clone()).await,
            "execute_command" => self.handle_execute_command(task.clone()).await,
            "close_session" => self.handle_close_session(task.clone()).await,
            "restore_session" => self.handle_restore_session(task.clone()).await,
            "publish_session" => self.handle_publish_session(task.clone()).await,
            "unpublish_session" => self.handle_unpublish_session(task.clone()).await,
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

    pub async fn handle_create_session(&self, task: SessionTask) -> Result<()> {
        let session_name = task.session_name.clone();
        
        // Parse the payload to get session creation parameters
        let mut secrets = task.payload.get("secrets")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                   .collect::<std::collections::HashMap<String, String>>()
            })
            .unwrap_or_default();
            
        let instructions = task.payload.get("instructions")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
            
        let setup = task.payload.get("setup")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let prompt = task.payload.get("prompt")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Extract principal information for logging and token generation
        let principal = task.payload.get("principal")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let principal_type_str = task.payload.get("principal_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        
        // Parse principal type for token generation
        let principal_type = match principal_type_str {
            "Operator" => SubjectType::Operator,
            "User" => SubjectType::Subject,
            _ => SubjectType::Subject,
        };

        // Generate dynamic tokens for this session
        info!("Generating dynamic tokens for session {}", session_name);
        let session_api_key = self.generate_session_api_key(&session_name).await
            .map_err(|e| anyhow::anyhow!("Failed to generate session API key: {}", e))?;
        let session_token = self.generate_session_token(principal, principal_type, &session_name)
            .map_err(|e| anyhow::anyhow!("Failed to generate session token: {}", e))?;
        
        info!("Generated dynamic tokens for session {} (principal: {})", session_name, principal);
        
        info!("Creating session {} for principal {} ({:?}) with {} secrets, instructions: {}, setup: {}, prompt: {}", 
              session_name, principal, principal_type, secrets.len(), instructions.is_some(), setup.is_some(), prompt.is_some());
        
        // Check if this is a remix session from task payload
        let is_remix = task.payload.get("remix")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        // For remix sessions, extract prompt from task payload 
        let remix_prompt = if is_remix {
            task.payload.get("prompt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };
        
        if is_remix {
            let parent_session_name = task.payload.get("parent_session_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing parent_session_name for remix"))?;
                
            let copy_data = task.payload.get("copy_data")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let copy_code = task.payload.get("copy_code")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let copy_secrets = task.payload.get("copy_secrets")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let copy_canvas = task.payload.get("copy_canvas")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
                
            // For remix sessions, get principal info from remix task payload
            let remix_principal = task.payload.get("principal")
                .and_then(|v| v.as_str())
                .unwrap_or(principal);
            let remix_principal_type_str = task.payload.get("principal_type")
                .and_then(|v| v.as_str())
                .unwrap_or(principal_type_str);
                
            info!("DEBUG: Remix task payload principal: {:?}, principal_type: {:?}", 
                  task.payload.get("principal"), task.payload.get("principal_type"));
            info!("DEBUG: Using remix_principal: {}, remix_principal_type_str: {}", 
                  remix_principal, remix_principal_type_str);
            let remix_principal_type = match remix_principal_type_str {
                "Operator" => SubjectType::Operator,
                "User" => SubjectType::Subject,
                _ => SubjectType::Subject,
            };
                
            info!("Creating remix session {} from parent {} (copy_data: {}, copy_code: {}, copy_secrets: {}, copy_canvas: {}) for principal {} ({})", 
                  session_name, parent_session_name, copy_data, copy_code, copy_secrets, copy_canvas, remix_principal, remix_principal_type_str);
            
            // For remix sessions, create container with selective volume copy from parent
            // Generate fresh tokens for remix session
            let remix_api_key = self.generate_session_api_key(&session_name).await
                .map_err(|e| anyhow::anyhow!("Failed to generate remix session API key: {}", e))?;
            let remix_token = self.generate_session_token(remix_principal, remix_principal_type, &session_name)
                .map_err(|e| anyhow::anyhow!("Failed to generate remix session token: {}", e))?;
            
            self.docker_manager.create_container_with_selective_copy_and_tokens(
                &session_name, 
                parent_session_name, 
                copy_data, 
                copy_code,
                copy_secrets,
                copy_canvas,
                remix_api_key,
                remix_token,
                remix_principal.to_string(),
                remix_principal_type_str.to_string(),
                task.created_at
            ).await?;
        } else {
            info!("Creating new session {}", session_name);
            
            // For regular sessions, create container with session parameters and generated tokens
            self.docker_manager.create_container_with_params_and_tokens(
                &session_name, 
                secrets, 
                instructions, 
                setup,
                session_api_key,
                session_token,
                principal.to_string(),
                principal_type_str.to_string(),
                task.created_at
            ).await?;
        }
        
        // Send prompt if provided (BEFORE setting state to IDLE)
        let prompt_to_send = prompt.or(remix_prompt);
        if let Some(prompt) = prompt_to_send {
            info!("Sending prompt to session {}: {}", session_name, prompt);
            
            // Create message record in database
            let message_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(r#"
                INSERT INTO session_messages (id, session_name, created_by, content, role, created_at)
                VALUES (?, ?, ?, ?, 'user', NOW())
                "#)
            .bind(&message_id)
            .bind(&session_name)
            .bind(&principal)
            .bind(&prompt)
            .execute(&self.pool)
            .await?;
            
            info!("Prompt message {} created for session {}", message_id, session_name);
        }
        
        // Set session state to INIT after container creation (host will set to IDLE when ready)
        sqlx::query(r#"UPDATE sessions SET state = ?, last_activity_at = NOW() WHERE name = ?"#)
        .bind(SESSION_STATE_INIT)
        .bind(&session_name)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn handle_destroy_session(&self, task: SessionTask) -> Result<()> {
        let session_name = task.session_name;
        
        info!("Deleting container and volume for session {}", session_name);
        self.docker_manager.delete_container(&session_name).await?;
        
        // No need to update session state - DELETE endpoint already soft-deletes the session
        
        Ok(())
    }

    pub async fn handle_execute_command(&self, task: SessionTask) -> Result<()> {
        let session_name = task.session_name;
        let command = task.payload["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing command in payload"))?;
        
        info!("Executing command in session {}: {}", session_name, command);
        let output = self.docker_manager.execute_command(&session_name, command).await?;
        
        sqlx::query(r#"
            INSERT INTO command_results (id, session_name, command, output, created_at)
            VALUES (?, ?, ?, ?, NOW())
            "#
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(session_name)
        .bind(command)
        .bind(output)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    async fn mark_task_completed(&self, task_id: &str) -> Result<()> {
        sqlx::query(r#"
            UPDATE session_tasks
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
        sqlx::query(r#"
            UPDATE session_tasks
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


    pub async fn handle_close_session(&self, task: SessionTask) -> Result<()> {
        let session_name = task.session_name;
        
        info!("Closing container for session {}", session_name);
        
        // Close the Docker container but keep the persistent volume
        self.docker_manager.close_container(&session_name).await?;
        
        // Update session state to closed
        sqlx::query(r#"UPDATE sessions SET state = 'closed' WHERE name = ?"#)
            .bind(&session_name)
            .execute(&self.pool)
            .await?;
        
        info!("Session {} state updated to closed", session_name);
        
        Ok(())
    }

    pub async fn handle_restore_session(&self, task: SessionTask) -> Result<()> {
        let session_name = task.session_name;
        let principal = task.created_by.clone();
        
        info!("Restoring container for session {}", session_name);
        
        // Generate fresh tokens for restored session
        info!("Generating fresh tokens for restored session {}", session_name);
        let restore_api_key = self.generate_session_api_key(&session_name).await
            .map_err(|e| anyhow::anyhow!("Failed to generate restore session API key: {}", e))?;
        let restore_token = self.generate_session_token(&principal, SubjectType::Subject, &session_name)
            .map_err(|e| anyhow::anyhow!("Failed to generate restore session token: {}", e))?;
        
        // All restored sessions were closed (container destroyed), so recreate container
        info!("Session {} was closed, restoring container with persistent volume and fresh tokens", session_name);
        self.docker_manager.restore_container_with_tokens(&session_name, restore_api_key, restore_token, principal.clone(), "User".to_string(), task.created_at).await?;
        
        // Update last_activity_at and clear auto_close_at since session is being restored
        sqlx::query(r#"UPDATE sessions SET last_activity_at = NOW(), auto_close_at = NULL WHERE name = ?"#)
            .bind(&session_name)
            .execute(&self.pool)
            .await?;
        
        info!("Container restored for session {}", session_name);
        
        // Send prompt if provided
        if let Some(prompt) = task.payload.get("prompt").and_then(|v| v.as_str()) {
            info!("Sending prompt to restored session {}: {}", session_name, prompt);
            
            // Get the principal name from the task
            let principal = task.created_by;
            
            // Create message record in database
            let message_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(r#"
                INSERT INTO session_messages (id, session_name, created_by, content, role, created_at)
                VALUES (?, ?, ?, ?, 'user', NOW())
                "#)
            .bind(&message_id)
            .bind(&session_name)
            .bind(&principal)
            .bind(prompt)
            .execute(&self.pool)
            .await?;
            
            info!("Prompt message {} created for restored session {}", message_id, session_name);
        }
        
        Ok(())
    }

    async fn handle_publish_session(&self, task: SessionTask) -> Result<()> {
        let session_name = &task.session_name;
        info!("Publishing canvas for session {}", session_name);
        
        // Check if docker command is available
        match tokio::process::Command::new("which").arg("docker").output().await {
            Ok(output) if output.status.success() => {
                let docker_path = String::from_utf8_lossy(&output.stdout);
                let docker_path = docker_path.trim();
                info!("Found docker at: {}", docker_path);
            }
            _ => {
                warn!("Could not find docker binary");
            }
        }
        
        let session_container = format!("raworc_session_{}", session_name);
        
        // First, create the public directory in the server container
        let public_dir = format!("/public/{}", session_name);
        info!("Executing: docker exec raworc_server mkdir -p {}", public_dir);
        
        let mkdir_output = tokio::process::Command::new("docker")
            .args(&["exec", "raworc_server", "mkdir", "-p", &public_dir])
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to execute mkdir command for session {}: {}", session_name, e))?;
        
        if !mkdir_output.status.success() {
            let stderr = String::from_utf8_lossy(&mkdir_output.stderr);
            let stdout = String::from_utf8_lossy(&mkdir_output.stdout);
            return Err(anyhow::anyhow!("Failed to create public directory for session {}: stdout: {}, stderr: {}", 
                session_name, stdout, stderr));
        }
        
        // Copy canvas files from session container directly to server container's public directory
        // This uses docker cp to copy from session container to host, then from host to server container
        let temp_dir = format!("/tmp/canvas_publish_{}", session_name);
        
        // Create temp directory on host
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| anyhow::anyhow!("Failed to create temp directory: {}", e))?;
        
        // Copy from session container to host temp
        let copy1_output = tokio::process::Command::new("docker")
            .args(&["cp", &format!("{}:/session/canvas/.", session_container), &format!("{}/", temp_dir)])
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to execute copy command from session container: {}", e))?;
            
        if !copy1_output.status.success() {
            let _ = std::fs::remove_dir_all(&temp_dir);
            let stderr = String::from_utf8_lossy(&copy1_output.stderr);
            return Err(anyhow::anyhow!("Failed to copy canvas from session container: {}", stderr));
        }
        
        // Copy from host temp to server container
        let copy2_output = tokio::process::Command::new("docker")
            .args(&["cp", &format!("{}//.", temp_dir), &format!("raworc_server:/public/{}/", session_name)])
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to execute copy command to server container: {}", e))?;
            
        // Clean up temp directory
        let _ = std::fs::remove_dir_all(&temp_dir);
        
        if !copy2_output.status.success() {
            let stderr = String::from_utf8_lossy(&copy2_output.stderr);
            return Err(anyhow::anyhow!("Failed to copy canvas to server container: {}", stderr));
        }
        
        info!("Canvas published for session {} to /public/{}/", session_name, session_name);
        Ok(())
    }
    
    async fn handle_unpublish_session(&self, task: SessionTask) -> Result<()> {
        let session_name = &task.session_name;
        info!("Unpublishing canvas for session {}", session_name);
        
        // Remove public directory for this session
        let public_path = format!("/public/{}", session_name);
        
        // Remove the published directory
        let remove_cmd = format!("rm -rf {}", public_path);
        tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&remove_cmd)
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to remove published canvas for session {}: {}", session_name, e))?;
        
        info!("Canvas unpublished for session {}", session_name);
        Ok(())
    }
}