use anyhow::Result;
use bollard::{
    container::{Config, CreateContainerOptions, RemoveContainerOptions},
    exec::{CreateExecOptions, StartExecResults},
    Docker,
};
use futures::StreamExt;
use sqlx::MySqlPool;
use std::collections::HashMap;
use tracing::{error, info, warn};

pub struct DockerManager {
    docker: Docker,
    host_image: String,
    cpu_limit: f64,
    memory_limit: i64,
    db_pool: MySqlPool,
}

impl DockerManager {
    pub fn new(docker: Docker, db_pool: MySqlPool) -> Self {
        Self {
            docker,
            db_pool,
            host_image: std::env::var("HOST_AGENT_IMAGE")
                .unwrap_or_else(|_| "raworc_host:latest".to_string()),
            cpu_limit: std::env::var("HOST_AGENT_CPU_LIMIT")
                .unwrap_or_else(|_| "0.5".to_string())
                .parse()
                .unwrap_or(0.5),
            memory_limit: std::env::var("HOST_AGENT_MEMORY_LIMIT")
                .unwrap_or_else(|_| "536870912".to_string())
                .parse()
                .unwrap_or(536870912),
        }
    }
    
    async fn get_space_secrets(&self, space: &str) -> Result<HashMap<String, String>> {
        let mut secrets = HashMap::new();
        
        // Fetch secrets from database
        let rows = sqlx::query_as::<_, (String, String)>(
            r#"
            SELECT key_name, encrypted_value 
            FROM space_secrets 
            WHERE space = ?
            "#
        )
        .bind(space)
        .fetch_all(&self.db_pool)
        .await?;
        
        for (key_name, encrypted_value) in rows {
            // In production, you would decrypt the value here
            // For now, we'll use the value as-is
            secrets.insert(key_name, encrypted_value);
        }
        
        Ok(secrets)
    }

    // NEW: Create session volume with explicit naming
    async fn create_session_volume(&self, session_id: &str) -> Result<String> {
        let volume_name = format!("raworc_session_data_{}", session_id);
        
        let mut labels = HashMap::new();
        labels.insert("raworc.session_id".to_string(), session_id.to_string());
        labels.insert("raworc.type".to_string(), "host_session_volume".to_string());
        labels.insert("raworc.created_at".to_string(), chrono::Utc::now().to_rfc3339());
        
        let volume_config = bollard::volume::CreateVolumeOptions {
            name: volume_name.clone(),
            driver: "local".to_string(),
            driver_opts: HashMap::new(),
            labels,
        };
        
        self.docker.create_volume(volume_config).await?;
        info!("Created session volume: {}", volume_name);
        
        // Update database with volume ID
        self.update_session_volume(session_id, &volume_name).await?;
        
        Ok(volume_name)
    }

    // NEW: Update session with volume ID
    async fn update_session_volume(&self, session_id: &str, volume_name: &str) -> Result<()> {
        sqlx::query("UPDATE sessions SET persistent_volume_id = ? WHERE id = ?")
            .bind(volume_name)
            .bind(session_id)
            .execute(&self.db_pool)
            .await?;
        Ok(())
    }

    // NEW: Get session volume with new naming
    async fn get_session_volume(&self, session_id: &str) -> Result<Option<String>> {
        // First check database
        let result = sqlx::query_as::<_, (Option<String>,)>(
            "SELECT persistent_volume_id FROM sessions WHERE id = ?"
        )
        .bind(session_id)
        .fetch_optional(&self.db_pool)
        .await?;
        
        if let Some((Some(volume_name),)) = result {
            return Ok(Some(volume_name));
        }
        
        // Fallback: check if volume exists with expected name
        let expected_volume_name = format!("raworc_session_data_{}", session_id);
        match self.docker.inspect_volume(&expected_volume_name).await {
            Ok(_) => {
                // Volume exists but not in database, update database
                self.update_session_volume(session_id, &expected_volume_name).await?;
                Ok(Some(expected_volume_name))
            }
            Err(_) => Ok(None),
        }
    }

    // NEW: Initialize session directory structure
    async fn initialize_session_structure(&self, session_id: &str) -> Result<()> {
        info!("Initializing session structure for session {}", session_id);
        
        let init_script = "mkdir -p /session/{space,agents,cache,state,tmp}
mkdir -p /session/cache/{cargo,pip,npm,git}

cd /session
echo '# Welcome to your Raworc session!' > README.md
echo 'This is your persistent space. Files here will be saved across session restarts.' >> README.md
echo '' >> README.md
echo '## Directory Structure:' >> README.md
echo '- /session/ - Your main working directory (current directory)' >> README.md
echo '- /session/agents/ - Custom agents and their data' >> README.md
echo '- /session/cache/ - Build caches for tools' >> README.md
echo '- /session/state/ - Session state and tasks' >> README.md

echo '{}' > /session/state/agent_metadata.json
echo '{\"initialized\": true, \"version\": \"1.0\", \"structure\": \"session_v1\"}' > /session/state/session.state

chmod -R 755 /session

echo 'Session structure initialized'
";

        self.execute_command(session_id, init_script).await?;
        info!("Session structure initialized");
        
        Ok(())
    }

    // NEW: Check if session is initialized
    async fn session_initialized(&self, _volume_name: &str) -> Result<bool> {
        // Skip initialization check for now - always initialize
        Ok(false)
    }


    // NEW: Cleanup session volume
    pub async fn cleanup_session_volume(&self, session_id: &str) -> Result<()> {
        let expected_volume_name = format!("raworc_session_data_{}", session_id);
        
        match self.docker.remove_volume(&expected_volume_name, None).await {
            Ok(_) => {
                info!("Removed session volume: {}", expected_volume_name);
                
                // Clear from database
                sqlx::query("UPDATE sessions SET persistent_volume_id = NULL WHERE id = ?")
                    .bind(session_id)
                    .execute(&self.db_pool)
                    .await?;
            }
            Err(e) => warn!("Failed to remove session volume {}: {}", expected_volume_name, e),
        }
        
        Ok(())
    }
    
    async fn generate_operator_token(&self, space: &str) -> Result<String> {
        // Generate a JWT token with operator role for the space
        // This would use the same JWT logic as the auth system
        use jsonwebtoken::{encode, EncodingKey, Header};
        use serde::{Deserialize, Serialize};
        use chrono::{Duration, Utc};
        
        #[derive(Debug, Serialize, Deserialize)]
        struct Claims {
            sub: String,
            sub_type: String,
            space: Option<String>,
            exp: usize,
            iat: usize,
            iss: String,
        }
        
        let now = Utc::now();
        let claims = Claims {
            sub: "operator".to_string(),
            sub_type: "ServiceAccount".to_string(),
            space: Some(space.to_string()),
            exp: (now + Duration::days(365)).timestamp() as usize,
            iat: now.timestamp() as usize,
            iss: "raworc-operator".to_string(),
        };
        
        let secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "development-secret-key".to_string());
        
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_ref()),
        )?;
        
        Ok(token)
    }

    pub async fn create_container(&self, session_id: &str) -> Result<String> {
        let container_name = format!("raworc_session_{session_id}");
        
        // Fetch session details and space image
        let session_row = sqlx::query_as::<_, (String, Option<String>)>(
            r#"
            SELECT s.space,
                   wb.image_tag
            FROM sessions s
            LEFT JOIN space_builds wb ON s.space = wb.space 
                AND wb.status = 'completed'
            WHERE s.id = ?
            ORDER BY wb.started_at DESC
            LIMIT 1
            "#
        )
        .bind(session_id)
        .fetch_optional(&self.db_pool)
        .await?;
        
        let (space, container_image) = match session_row {
            Some(row) => {
                let space = row.0;
                let image = row.1.unwrap_or_else(|| {
                    warn!("No space image found for {}, falling back to host image", space);
                    self.host_image.clone()
                });
                (space, image)
            }
            None => {
                warn!("Session {} not found, using default space and host image", session_id);
                ("default".to_string(), self.host_image.clone())
            }
        };
        
        info!("Creating container {} with image {}", container_name, container_image);
        
        info!("Creating container for session {} in space {}", session_id, space);

        // Create or get existing session volume
        let session_volume = match self.get_session_volume(session_id).await? {
            Some(existing) => existing,
            None => self.create_session_volume(session_id).await?,
        };

        let mut labels = HashMap::new();
        labels.insert("raworc.session".to_string(), session_id.to_string());
        labels.insert("raworc.space".to_string(), space.clone());
        labels.insert("raworc.managed".to_string(), "true".to_string());
        labels.insert("raworc.volume".to_string(), session_volume.clone());
        
        // Generate operator token for this space
        let operator_token = self.generate_operator_token(&space).await?;
        
        // Get space secrets
        let secrets = self.get_space_secrets(&space).await?;

        // Configure volume mounts
        let mounts = vec![
            bollard::models::Mount {
                typ: Some(bollard::models::MountTypeEnum::VOLUME),
                source: Some(session_volume.clone()),
                target: Some("/session".to_string()),
                read_only: Some(false),
                ..Default::default()
            }
        ];

        // Set environment variables for the clean directory structure
        let mut env = vec![
            format!("RAWORC_API_URL=http://raworc_server:9000"),
            format!("RAWORC_SESSION_ID={}", session_id),
            format!("RAWORC_SPACE_ID={}", space),
            format!("RAWORC_API_TOKEN={}", operator_token),
            
            // Clean, simple paths
            format!("RAWORC_SESSION_DIR=/session"),
            format!("RAWORC_AGENTS_DIR=/session/agents"),
            format!("RAWORC_STATE_DIR=/session/state"),
            format!("RAWORC_CACHE_DIR=/session/cache"),
            
            // Build tool cache configuration
            format!("CARGO_HOME=/session/cache/cargo"),
            format!("PIP_CACHE_DIR=/session/cache/pip"),
            format!("NPM_CONFIG_CACHE=/session/cache/npm"),
        ];
        
        // Add space secrets as environment variables
        for (key, value) in secrets {
            env.push(format!("{}={}", key, value));
        }

        // Set the command with required arguments
        let cmd = vec![
            "raworc-host".to_string(),
            "--api-url".to_string(),
            "http://raworc_server:9000".to_string(),
            "--session-id".to_string(),
            session_id.to_string(),
            "--api-key".to_string(),
            operator_token.clone(),
        ];

        let config = Config {
            image: Some(container_image),
            hostname: Some(format!("session-{}", &session_id.to_string()[..8])),
            labels: Some(labels),
            env: Some(env),
            cmd: Some(cmd),
            working_dir: Some("/session".to_string()), // User starts in their session
            host_config: Some(bollard::models::HostConfig {
                cpu_quota: Some((self.cpu_limit * 100000.0) as i64),
                cpu_period: Some(100000),
                memory: Some(self.memory_limit),
                memory_swap: Some(self.memory_limit),
                network_mode: Some("raworc_network".to_string()),
                mounts: Some(mounts),
                ..Default::default()
            }),
            ..Default::default()
        };

        let options = CreateContainerOptions {
            name: container_name.clone(),
            ..Default::default()
        };

        let container = self.docker.create_container(Some(options), config).await?;
        
        self.docker
            .start_container::<String>(&container.id, None)
            .await?;

        // Initialize session structure if needed
        if !self.session_initialized(&session_volume).await? {
            self.initialize_session_structure(session_id).await?;
        }

        info!("Container {} created with session volume {}", container_name, session_volume);
        Ok(container.id)
    }

    // Close container but retain persistent volume (for session pause/close)
    pub async fn close_container(&self, session_id: &str) -> Result<()> {
        let container_name = format!("raworc_session_{session_id}");
        
        info!("Closing container {}", container_name);

        let options = RemoveContainerOptions {
            force: true,
            ..Default::default()
        };

        match self.docker.remove_container(&container_name, Some(options)).await {
            Ok(_) => {
                info!("Container {} closed, persistent volume retained", container_name);
                Ok(())
            }
            Err(e) => {
                error!("Failed to close container {}: {}", container_name, e);
                Err(anyhow::anyhow!("Failed to close container: {}", e))
            }
        }
    }

    // Delete container and remove persistent volume (for session deletion)
    pub async fn delete_container(&self, session_id: &str) -> Result<()> {
        let container_name = format!("raworc_session_{session_id}");
        
        info!("Deleting container {}", container_name);

        let options = RemoveContainerOptions {
            force: true,
            ..Default::default()
        };

        match self.docker.remove_container(&container_name, Some(options)).await {
            Ok(_) => {
                info!("Container {} deleted", container_name);
                
                // Cleanup the session volume
                if let Err(e) = self.cleanup_session_volume(session_id).await {
                    warn!("Failed to cleanup session volume for {}: {}", session_id, e);
                }
                
                Ok(())
            }
            Err(e) => {
                error!("Failed to delete container {}: {}", container_name, e);
                Err(anyhow::anyhow!("Failed to delete container: {}", e))
            }
        }
    }

    // Legacy method - kept for backward compatibility, but deprecated
    // Use close_container or delete_container instead
    pub async fn destroy_container(&self, session_id: &str) -> Result<()> {
        warn!("destroy_container is deprecated, use close_container or delete_container instead");
        self.delete_container(session_id).await
    }

    pub async fn execute_command(&self, session_id: &str, command: &str) -> Result<String> {
        let container_name = format!("raworc_session_{session_id}");
        
        info!("Executing command in container {}: {}", container_name, command);

        let exec_config = CreateExecOptions {
            cmd: Some(vec!["/bin/bash", "-c", command]),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        let exec = self.docker
            .create_exec(&container_name, exec_config)
            .await?;

        let mut output_str = String::new();
        
        if let StartExecResults::Attached { mut output, .. } = 
            self.docker.start_exec(&exec.id, None).await? 
        {
            while let Some(Ok(msg)) = output.next().await {
                output_str.push_str(&msg.to_string());
            }
        }

        Ok(output_str)
    }
}