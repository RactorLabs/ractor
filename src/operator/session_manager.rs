use anyhow::Result;
use bollard::Docker;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{mysql::MySqlPoolOptions, Pool, MySql};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

// Import constants from shared module
#[path = "../shared/models/constants.rs"]
pub mod constants;
pub use constants::SESSION_STATE_IDLE;

use super::docker_manager::DockerManager;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SessionTask {
    id: String,
    task_type: String,
    session_id: String,
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
}

impl SessionManager {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        let docker = Docker::connect_with_socket_defaults()?;
        let docker_manager = DockerManager::new(docker, pool.clone());

        Ok(Self {
            pool,
            docker_manager,
        })
    }

    pub async fn run(&self) -> Result<()> {
        info!("Session Manager started, polling for tasks...");

        loop {
            match self.process_pending_tasks().await {
                Ok(processed) => {
                    if processed == 0 {
                        sleep(Duration::from_secs(2)).await;
                    }
                }
                Err(e) => {
                    error!("Error processing tasks: {}", e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
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
        let session_id = task.session_id.clone();
        
        // Parse the payload to get session creation parameters
        let secrets = task.payload.get("secrets")
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
        
        info!("Creating session {} with {} secrets, instructions: {}, setup: {}", 
              session_id, secrets.len(), instructions.is_some(), setup.is_some());
        
        // Check if this is a remix session from task payload
        let is_remix = task.payload.get("remix")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        if is_remix {
            let parent_session_id = task.payload.get("parent_session_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing parent_session_id for remix"))?;
                
            let copy_data = task.payload.get("copy_data")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let copy_code = task.payload.get("copy_code")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let copy_secrets = task.payload.get("copy_secrets")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            
            info!("Creating remix session {} from parent {} (copy_data: {}, copy_code: {}, copy_secrets: {})", 
                  session_id, parent_session_id, copy_data, copy_code, copy_secrets);
            
            // For remix sessions, create container with selective volume copy from parent
            self.docker_manager.create_container_with_selective_copy(
                &session_id, 
                parent_session_id, 
                copy_data, 
                copy_code, 
                copy_secrets
            ).await?;
        } else {
            info!("Creating new session {}", session_id);
            
            // For regular sessions, create container with session parameters
            self.docker_manager.create_container_with_params(&session_id, secrets, instructions, setup).await?;
        }
        
        sqlx::query(r#"UPDATE sessions SET state = ?, last_activity_at = NOW() WHERE id = ?"#)
        .bind(SESSION_STATE_IDLE)
        .bind(&session_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn handle_destroy_session(&self, task: SessionTask) -> Result<()> {
        let session_id = task.session_id;
        
        info!("Deleting container and volume for session {}", session_id);
        self.docker_manager.delete_container(&session_id).await?;
        
        // No need to update session state - DELETE endpoint already soft-deletes the session
        
        Ok(())
    }

    pub async fn handle_execute_command(&self, task: SessionTask) -> Result<()> {
        let session_id = task.session_id;
        let command = task.payload["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing command in payload"))?;
        
        info!("Executing command in session {}: {}", session_id, command);
        let output = self.docker_manager.execute_command(&session_id, command).await?;
        
        sqlx::query(r#"
            INSERT INTO command_results (id, session_id, command, output, created_at)
            VALUES (?, ?, ?, ?, NOW())
            "#
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(session_id)
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
        let session_id = task.session_id;
        
        info!("Closing container for session {}", session_id);
        
        // Close the Docker container but keep the persistent volume
        self.docker_manager.close_container(&session_id).await?;
        
        Ok(())
    }

    pub async fn handle_restore_session(&self, task: SessionTask) -> Result<()> {
        let session_id = task.session_id;
        
        info!("Restoring container for session {}", session_id);
        
        // All restored sessions were closed (container destroyed), so recreate container
        info!("Session {} was closed, creating new container with persistent volume", session_id);
        self.docker_manager.create_container(&session_id).await?;
        
        // Update last_activity_at to track when session was restored
        sqlx::query(r#"UPDATE sessions SET last_activity_at = NOW() WHERE id = ?"#)
            .bind(&session_id)
            .execute(&self.pool)
            .await?;
        
        info!("Container restored for session {}", session_id);
        
        Ok(())
    }
}