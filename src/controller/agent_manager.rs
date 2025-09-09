use anyhow::Result;
use bollard::Docker;
use chrono::{DateTime, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sqlx::{mysql::MySqlPoolOptions, MySql, Pool};
use std::time::Duration;
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
            let agents_slept = match self.process_auto_sleep().await {
                Ok(slept) => slept,
                Err(e) => {
                    error!("Error processing auto-sleep: {}", e);
                    0
                }
            };

            // Check health of active agents
            let agents_recovered = match self.check_agent_health().await {
                Ok(recovered) => recovered,
                Err(e) => {
                    error!("Error checking agent health: {}", e);
                    0
                }
            };

            // If no work was done, sleep before next iteration
            if tasks_processed == 0 && agents_slept == 0 && agents_recovered == 0 {
                sleep(Duration::from_secs(10)).await;
            }
        }
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
        let mut secrets = task
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
            "Operator" => SubjectType::Operator,
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
                "Operator" => SubjectType::Operator,
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

            // Create message record in database
            let message_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                r#"
                INSERT INTO agent_messages (id, agent_name, created_by, content, role, created_at)
                VALUES (?, ?, ?, ?, 'user', NOW())
                "#,
            )
            .bind(&message_id)
            .bind(&agent_name)
            .bind(&principal)
            .bind(&prompt)
            .execute(&self.pool)
            .await?;

            info!(
                "Prompt message {} created for agent {}",
                message_id, agent_name
            );
        }

        // Mark agent idle after container creation; agent will toggle busy/idle as it works
        sqlx::query(
            r#"
            UPDATE agents 
            SET state = 'idle', last_activity_at = NOW(), idle_from = NOW(), busy_from = NULL
            WHERE name = ?
            "#,
        )
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

        info!("Sleeping container for agent {}", agent_name);

        // Sleep the Docker container but keep the persistent volume
        self.docker_manager.sleep_container(&agent_name).await?;

        // Update agent state to slept
        sqlx::query(r#"UPDATE agents SET state = 'slept' WHERE name = ?"#)
            .bind(&agent_name)
            .execute(&self.pool)
            .await?;

        info!("Agent {} state updated to slept", agent_name);

        Ok(())
    }

    pub async fn handle_wake_agent(&self, task: AgentTask) -> Result<()> {
        let agent_name = task.agent_name;
        let principal = task.created_by.clone();

        info!("Waking container for agent {}", agent_name);

        // Generate fresh tokens for woken agent
        info!("Generating fresh tokens for woken agent {}", agent_name);
        let wake_token = self
            .generate_agent_token(&principal, SubjectType::Subject, &agent_name)
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
                principal.clone(),
                "User".to_string(),
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
            let principal = task.created_by;

            // Create message record in database
            let message_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                r#"
                INSERT INTO agent_messages (id, agent_name, created_by, content, role, created_at)
                VALUES (?, ?, ?, ?, 'user', NOW())
                "#,
            )
            .bind(&message_id)
            .bind(&agent_name)
            .bind(&principal)
            .bind(prompt)
            .execute(&self.pool)
            .await?;

            info!(
                "Prompt message {} created for woken agent {}",
                message_id, agent_name
            );
        }

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

        let agent_container = format!("raworc_agent_{}", agent_name);

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
}
