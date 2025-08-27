use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, MySql};
use std::sync::Arc;
use tracing::{error, info, warn};

use super::space_builder::SpaceBuilder;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BuildTask {
    pub id: String,
    pub task_type: String,
    pub space: String,
    pub build_id: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub created_by: String,
}

pub struct BuildManager {
    pool: Arc<Pool<MySql>>,
}

impl BuildManager {
    pub fn new(pool: Arc<Pool<MySql>>) -> Self {
        Self { pool }
    }

    pub async fn poll_and_process_tasks(&self) -> Result<()> {
        // Get pending build tasks
        let tasks = self.get_pending_tasks().await?;
        
        for task in tasks {
            info!("Processing build task {} for space {}", task.id, task.space);
            
            // Mark task as processing
            if let Err(e) = self.update_task_status(&task.id, "processing", None).await {
                error!("Failed to mark task {} as processing: {}", task.id, e);
                continue;
            }

            // Process the task
            let result = self.process_build_task(&task).await;
            
            // Update task status based on result
            match result {
                Ok(_) => {
                    info!("Build task {} completed successfully", task.id);
                    if let Err(e) = self.update_task_status(&task.id, "completed", None).await {
                        error!("Failed to mark task {} as completed: {}", task.id, e);
                    }
                }
                Err(e) => {
                    error!("Build task {} failed: {}", task.id, e);
                    if let Err(update_err) = self.update_task_status(&task.id, "failed", Some(&e.to_string())).await {
                        error!("Failed to mark task {} as failed: {}", task.id, update_err);
                    }
                }
            }
        }
        
        Ok(())
    }

    async fn get_pending_tasks(&self) -> Result<Vec<BuildTask>> {
        let tasks = sqlx::query_as::<_, BuildTask>(
            r#"
            SELECT id, task_type, space, build_id, payload, status, 
                   created_at, updated_at, started_at, completed_at, error, created_by
            FROM build_tasks 
            WHERE status = 'pending'
            ORDER BY created_at ASC
            LIMIT 10
            "#
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(tasks)
    }

    async fn update_task_status(&self, task_id: &str, status: &str, error: Option<&str>) -> Result<()> {
        let now = Utc::now();
        
        if status == "processing" {
            sqlx::query("UPDATE build_tasks SET status = ?, updated_at = ?, started_at = ? WHERE id = ?")
                .bind(status)
                .bind(now)
                .bind(now)
                .bind(task_id)
                .execute(&*self.pool)
                .await?;
        } else if status == "completed" || status == "failed" {
            if let Some(error_msg) = error {
                sqlx::query("UPDATE build_tasks SET status = ?, updated_at = ?, completed_at = ?, error = ? WHERE id = ?")
                    .bind(status)
                    .bind(now)
                    .bind(now)
                    .bind(error_msg)
                    .bind(task_id)
                    .execute(&*self.pool)
                    .await?;
            } else {
                sqlx::query("UPDATE build_tasks SET status = ?, updated_at = ?, completed_at = ? WHERE id = ?")
                    .bind(status)
                    .bind(now)
                    .bind(now)
                    .bind(task_id)
                    .execute(&*self.pool)
                    .await?;
            }
        }
        
        Ok(())
    }

    async fn process_build_task(&self, task: &BuildTask) -> Result<()> {
        match task.task_type.as_str() {
            "space_build" => {
                // Extract build parameters from payload
                let force_rebuild = task.payload.get("force_rebuild")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                // Execute the space build
                SpaceBuilder::build_space(
                    &task.space,
                    &task.build_id,
                    force_rebuild,
                    self.pool.clone(),
                ).await?;

                Ok(())
            }
            _ => {
                warn!("Unknown build task type: {}", task.task_type);
                Err(anyhow::anyhow!("Unknown task type: {}", task.task_type))
            }
        }
    }
}