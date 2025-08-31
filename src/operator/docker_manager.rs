use anyhow::Result;
use bollard::{
    container::{Config, CreateContainerOptions, RemoveContainerOptions, LogsOptions},
    exec::{CreateExecOptions, StartExecResults},
    models::{HostConfig, Mount, MountTypeEnum},
    Docker,
};
use futures::StreamExt;
use sqlx::MySqlPool;
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

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
            host_image: std::env::var("HOST_IMAGE")
                .unwrap_or_else(|_| "raworc_host:latest".to_string()),
            cpu_limit: std::env::var("HOST_CPU_LIMIT")
                .unwrap_or_else(|_| "0.5".to_string())
                .parse()
                .unwrap_or(0.5),
            memory_limit: std::env::var("HOST_MEMORY_LIMIT")
                .unwrap_or_else(|_| "536870912".to_string())
                .parse()
                .unwrap_or(536870912),
        }
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

    // Initialize session directory structure with secrets, instructions, and setup
    async fn initialize_session_structure(&self, session_id: &str, secrets: &HashMap<String, String>, instructions: Option<&str>, setup: Option<&str>) -> Result<()> {
        info!("Initializing session structure for session {}", session_id);
        
        // Create base directories
        let init_script = "mkdir -p /session/{code,data,secrets}
chmod -R 755 /session
echo 'Session directories created'
";

        self.execute_command(session_id, init_script).await?;
        
        // Write secrets to /session/secrets/ folder
        for (key, value) in secrets {
            let write_secret_command = format!("echo '{}' > /session/secrets/{}", value, key);
            self.execute_command(session_id, &write_secret_command).await?;
        }
        
        // Write instructions if provided
        if let Some(instructions_content) = instructions {
            let escaped_instructions = instructions_content.replace("'", "'\"'\"'");
            let write_instructions_command = format!("echo '{}' > /session/code/instructions.md", escaped_instructions);
            self.execute_command(session_id, &write_instructions_command).await?;
        }
        
        // Write and make setup script executable if provided
        if let Some(setup_content) = setup {
            let escaped_setup = setup_content.replace("'", "'\"'\"'");
            let write_setup_command = format!("echo '{}' > /session/code/setup.sh && chmod +x /session/code/setup.sh", escaped_setup);
            self.execute_command(session_id, &write_setup_command).await?;
        }
        
        info!("Session structure initialized with {} secrets", secrets.len());
        
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
    
    async fn generate_operator_token(&self) -> Result<String> {
        // Generate a JWT token with operator role
        use jsonwebtoken::{encode, EncodingKey, Header};
        use serde::{Deserialize, Serialize};
        use chrono::{Duration, Utc};
        
        #[derive(Debug, Serialize, Deserialize)]
        struct Claims {
            sub: String,
            sub_type: String,
            exp: usize,
            iat: usize,
            iss: String,
        }
        
        let now = Utc::now();
        let claims = Claims {
            sub: "operator".to_string(),
            sub_type: "ServiceAccount".to_string(),
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

    pub async fn create_container_with_volume_copy(&self, session_id: &str, parent_session_id: &str) -> Result<String> {
        info!("Creating remix session {} with volume copy from {}", session_id, parent_session_id);
        
        // First create the container normally (this creates the empty target volume)
        let container_name = self.create_container(session_id).await?;
        
        // Then copy data from parent volume to new volume using Docker command
        let parent_volume = format!("raworc_session_data_{}", parent_session_id);
        let new_volume = format!("raworc_session_data_{}", session_id);
        
        info!("Copying volume data from {} to {}", parent_volume, new_volume);
        
        // Use bollard Docker API to create copy container
        let copy_container_name = format!("raworc_volume_copy_{}", session_id);
        
        let config = Config {
            image: Some(self.host_image.clone()),
            cmd: Some(vec![
                "bash".to_string(),
                "-c".to_string(),
                "cp -a /source/. /dest/ 2>/dev/null || echo 'No source data'; echo 'Copy completed'".to_string()
            ]),
            host_config: Some(HostConfig {
                mounts: Some(vec![
                    Mount {
                        typ: Some(MountTypeEnum::VOLUME),
                        source: Some(parent_volume.clone()),
                        target: Some("/source".to_string()),
                        read_only: Some(true),
                        ..Default::default()
                    },
                    Mount {
                        typ: Some(MountTypeEnum::VOLUME),
                        source: Some(new_volume.clone()),
                        target: Some("/dest".to_string()),
                        read_only: Some(false),
                        ..Default::default()
                    }
                ]),
                network_mode: Some("raworc_network".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        
        // Create copy container
        self.docker.create_container(
            Some(CreateContainerOptions {
                name: copy_container_name.clone(),
                ..Default::default()
            }),
            config,
        ).await?;
        
        // Start container
        self.docker.start_container::<String>(&copy_container_name, None).await?;
        
        // Wait for completion
        let mut wait_stream = self.docker.wait_container::<String>(&copy_container_name, None);
        while let Some(wait_result) = wait_stream.next().await {
            let exit_result = wait_result?;
            if exit_result.status_code == 0 {
                info!("Volume copy completed successfully");
            } else {
                warn!("Volume copy container exited with code {}", exit_result.status_code);
            }
            break;
        }
        
        // Get logs from copy container for debugging
        let logs = self.docker.logs::<String>(
            &copy_container_name,
            Some(LogsOptions {
                stdout: true,
                stderr: true,
                ..Default::default()
            })
        );
        
        let log_output = logs.map(|log| {
            match log {
                Ok(line) => String::from_utf8_lossy(&line.into_bytes()).to_string(),
                Err(_) => String::new(),
            }
        }).collect::<Vec<_>>().await.join("");
        
        info!("Volume copy logs: {}", log_output.trim());
        
        // Clean up copy container
        let _ = self.docker.remove_container(
            &copy_container_name,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        ).await;
        
        Ok(container_name)
    }

    pub async fn create_container_with_selective_copy(
        &self, 
        session_id: &str, 
        parent_session_id: &str,
        copy_data: bool,
        copy_code: bool
    ) -> Result<String> {
        info!("Creating remix session {} with selective copy from {} (data: {}, code: {})", 
              session_id, parent_session_id, copy_data, copy_code);
        
        // First create the session volume (without starting container)
        let session_volume = self.create_session_volume(session_id).await?;
        
        // Then copy specific directories from parent volume to new volume
        let parent_volume = format!("raworc_session_data_{}", parent_session_id);
        let new_volume = format!("raworc_session_data_{}", session_id);
        
        info!("Copying selective data from {} to {}", parent_volume, new_volume);
        
        // Build copy commands based on what should be copied
        let mut copy_commands = Vec::new();
        
        // Always create base directory structure with proper ownership
        copy_commands.push("sudo mkdir -p /dest/code /dest/data /dest/secrets && sudo chown -R host:host /dest".to_string());
        
        if copy_data {
            copy_commands.push("if [ -d /source/data ]; then cp -a /source/data/. /dest/data/ || echo 'No data to copy'; fi".to_string());
        }
        
        if copy_code {
            copy_commands.push("if [ -d /source/code ]; then cp -a /source/code/. /dest/code/ || echo 'No code to copy'; fi".to_string());
        }
        
        // Always copy secrets for remix sessions
        copy_commands.push("if [ -d /source/secrets ]; then cp -a /source/secrets/. /dest/secrets/ && echo 'SECRETS_COPIED:' && find /source/secrets -type f -exec bash -c 'echo \"SECRET:$(basename {})=$(cat {})\"' \\; || echo 'No secrets to copy'; fi".to_string());
        
        // Always copy README.md from root if it exists
        copy_commands.push("if [ -f /source/README.md ]; then cp /source/README.md /dest/ || echo 'No README to copy'; fi".to_string());
        
        copy_commands.push("echo 'Selective copy completed'".to_string());
        
        let copy_command = copy_commands.join(" && ");
        
        // Use bollard Docker API to create copy container
        let copy_container_name = format!("raworc_volume_copy_{}", session_id);
        
        let config = Config {
            image: Some(self.host_image.clone()),
            user: Some("host".to_string()),
            cmd: Some(vec![
                "bash".to_string(),
                "-c".to_string(),
                copy_command
            ]),
            host_config: Some(HostConfig {
                mounts: Some(vec![
                    Mount {
                        typ: Some(MountTypeEnum::VOLUME),
                        source: Some(parent_volume.clone()),
                        target: Some("/source".to_string()),
                        read_only: Some(true),
                        ..Default::default()
                    },
                    Mount {
                        typ: Some(MountTypeEnum::VOLUME),
                        source: Some(new_volume.clone()),
                        target: Some("/dest".to_string()),
                        read_only: Some(false),
                        ..Default::default()
                    }
                ]),
                network_mode: Some("raworc_network".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        
        // Create copy container
        self.docker.create_container(
            Some(CreateContainerOptions {
                name: copy_container_name.clone(),
                ..Default::default()
            }),
            config,
        ).await?;
        
        // Start container
        self.docker.start_container::<String>(&copy_container_name, None).await?;
        
        // Wait for completion
        let mut wait_stream = self.docker.wait_container::<String>(&copy_container_name, None);
        while let Some(wait_result) = wait_stream.next().await {
            let exit_result = wait_result?;
            if exit_result.status_code == 0 {
                info!("Selective volume copy completed successfully");
            } else {
                warn!("Selective volume copy container exited with code {}", exit_result.status_code);
            }
            break;
        }
        
        // Get logs from copy container for debugging
        let logs = self.docker.logs::<String>(
            &copy_container_name,
            Some(LogsOptions {
                stdout: true,
                stderr: true,
                ..Default::default()
            })
        );
        
        let log_output = logs.map(|log| {
            match log {
                Ok(line) => String::from_utf8_lossy(&line.into_bytes()).to_string(),
                Err(_) => String::new(),
            }
        }).collect::<Vec<_>>().await.join("");
        
        info!("Selective copy logs: {}", log_output.trim());
        
        // Parse secrets from copy output (always copied for remix sessions)
        let mut secrets = std::collections::HashMap::new();
        
        // Parse SECRET:key=value lines from the copy output
        for line in log_output.lines() {
            if line.starts_with("SECRET:") {
                if let Some(secret_part) = line.strip_prefix("SECRET:") {
                    if let Some((key, value)) = secret_part.split_once('=') {
                        secrets.insert(key.to_string(), value.to_string());
                        info!("Parsed secret from copy output: {}", key);
                    }
                }
            }
        }
        
        info!("Successfully parsed {} secrets from copy output", secrets.len());
        
        // Clean up copy container
        let _ = self.docker.remove_container(
            &copy_container_name,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        ).await;
        
        // Read instructions and setup if code was copied
        let instructions = if copy_code {
            self.read_file_from_volume(&new_volume, "code/instructions.md").await.ok()
        } else {
            None
        };
        
        let setup = if copy_code {
            self.read_file_from_volume(&new_volume, "code/setup.sh").await.ok()
        } else {
            None
        };
        
        info!("Creating container with {} secrets from copied volume", secrets.len());
        
        // Now create and start the container with the copied secrets as environment variables
        let container_name = self.create_container_internal(session_id, Some(secrets), instructions, setup).await?;
        
        Ok(container_name)
    }

    // Helper method to read secrets from a volume
    async fn read_secrets_from_volume(&self, volume_name: &str) -> Result<std::collections::HashMap<String, String>> {
        let mut secrets = std::collections::HashMap::new();
        
        // Create a temporary container to read secrets from the volume
        let read_container_name = format!("raworc_read_secrets_{}", Uuid::new_v4().to_string()[..8].to_string());
        
        let config = Config {
            image: Some(self.host_image.clone()),
            user: Some("host".to_string()),
            cmd: Some(vec![
                "bash".to_string(),
                "-c".to_string(),
                "if [ -d /volume/secrets ]; then find /volume/secrets -type f -exec basename {} \\; 2>/dev/null || true; fi".to_string()
            ]),
            host_config: Some(HostConfig {
                mounts: Some(vec![
                    Mount {
                        typ: Some(MountTypeEnum::VOLUME),
                        source: Some(volume_name.to_string()),
                        target: Some("/volume".to_string()),
                        read_only: Some(true),
                        ..Default::default()
                    }
                ]),
                network_mode: Some("raworc_network".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        
        // Create and start container
        self.docker.create_container(
            Some(CreateContainerOptions {
                name: read_container_name.clone(),
                ..Default::default()
            }),
            config,
        ).await?;
        
        self.docker.start_container::<String>(&read_container_name, None).await?;
        
        // Wait for completion
        let mut wait_stream = self.docker.wait_container::<String>(&read_container_name, None);
        while let Some(wait_result) = wait_stream.next().await {
            let _exit_result = wait_result?;
            break;
        }
        
        // Get output (list of secret file names)
        let logs = self.docker.logs::<String>(
            &read_container_name,
            Some(LogsOptions {
                stdout: true,
                stderr: false,
                ..Default::default()
            })
        );
        
        let secret_files = logs.map(|log| {
            match log {
                Ok(line) => String::from_utf8_lossy(&line.into_bytes()).trim().to_string(),
                Err(_) => String::new(),
            }
        }).collect::<Vec<_>>().await.join("").lines().map(|s| s.to_string()).collect::<Vec<_>>();
        
        // Clean up the read container
        let _ = self.docker.remove_container(
            &read_container_name,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        ).await;
        
        // Now read each secret file content
        for secret_file in secret_files {
            if !secret_file.is_empty() {
                if let Ok(value) = self.read_file_from_volume(volume_name, &format!("secrets/{}", secret_file)).await {
                    secrets.insert(secret_file, value);
                }
            }
        }
        
        Ok(secrets)
    }
    
    // Helper method to read a file from a volume
    async fn read_file_from_volume(&self, volume_name: &str, file_path: &str) -> Result<String> {
        let read_container_name = format!("raworc_read_file_{}", Uuid::new_v4().to_string()[..8].to_string());
        
        let config = Config {
            image: Some(self.host_image.clone()),
            user: Some("host".to_string()),
            cmd: Some(vec![
                "bash".to_string(),
                "-c".to_string(),
                format!("if [ -f /volume/{} ]; then cat /volume/{}; fi", file_path, file_path)
            ]),
            host_config: Some(HostConfig {
                mounts: Some(vec![
                    Mount {
                        typ: Some(MountTypeEnum::VOLUME),
                        source: Some(volume_name.to_string()),
                        target: Some("/volume".to_string()),
                        read_only: Some(true),
                        ..Default::default()
                    }
                ]),
                network_mode: Some("raworc_network".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        
        // Create and start container
        self.docker.create_container(
            Some(CreateContainerOptions {
                name: read_container_name.clone(),
                ..Default::default()
            }),
            config,
        ).await?;
        
        self.docker.start_container::<String>(&read_container_name, None).await?;
        
        // Wait for completion
        let mut wait_stream = self.docker.wait_container::<String>(&read_container_name, None);
        while let Some(wait_result) = wait_stream.next().await {
            let _exit_result = wait_result?;
            break;
        }
        
        // Get file content
        let logs = self.docker.logs::<String>(
            &read_container_name,
            Some(LogsOptions {
                stdout: true,
                stderr: false,
                ..Default::default()
            })
        );
        
        let content = logs.map(|log| {
            match log {
                Ok(line) => String::from_utf8_lossy(&line.into_bytes()).to_string(),
                Err(_) => String::new(),
            }
        }).collect::<Vec<_>>().await.join("");
        
        // Clean up the read container
        let _ = self.docker.remove_container(
            &read_container_name,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        ).await;
        
        if content.trim().is_empty() {
            Err(anyhow::anyhow!("File {} not found or empty", file_path))
        } else {
            Ok(content.trim().to_string())
        }
    }

    pub async fn create_container_with_params(
        &self, 
        session_id: &str, 
        secrets: std::collections::HashMap<String, String>,
        instructions: Option<String>,
        setup: Option<String>
    ) -> Result<String> {
        let container_name = self.create_container_internal(session_id, Some(secrets), instructions, setup).await?;
        Ok(container_name)
    }

    pub async fn create_container(&self, session_id: &str) -> Result<String> {
        let container_name = self.create_container_internal(session_id, None, None, None).await?;
        Ok(container_name)
    }

    async fn create_container_internal(
        &self, 
        session_id: &str, 
        secrets: Option<std::collections::HashMap<String, String>>,
        instructions: Option<String>,
        setup: Option<String>
    ) -> Result<String> {
        let container_name = format!("raworc_session_{session_id}");
        
        // Use host image directly for all sessions
        let container_image = self.host_image.clone();
        info!("Creating container {} with host image {}", container_name, container_image);
        
        info!("Creating container for session {}", session_id);

        // Create or get existing session volume
        let session_volume = match self.get_session_volume(session_id).await? {
            Some(existing) => existing,
            None => self.create_session_volume(session_id).await?,
        };

        let mut labels = HashMap::new();
        labels.insert("raworc.session".to_string(), session_id.to_string());
        labels.insert("raworc.managed".to_string(), "true".to_string());
        labels.insert("raworc.volume".to_string(), session_volume.clone());
        
        // Generate operator token
        let operator_token = self.generate_operator_token().await?;

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

        // Set environment variables for the session structure
        let mut env = vec![
            format!("RAWORC_API_URL=http://raworc_server:9000"),
            format!("RAWORC_SESSION_ID={}", session_id),
            format!("RAWORC_API_KEY={}", operator_token),
            format!("RAWORC_SESSION_DIR=/session"),
        ];
        
        // Add secrets as environment variables
        if let Some(secrets_map) = &secrets {
            for (key, value) in secrets_map {
                env.push(format!("{}={}", key, value));
                info!("Adding secret {} as environment variable for session {}", key, session_id);
            }
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
            let empty_secrets = HashMap::new();
            let secrets_ref = secrets.as_ref().unwrap_or(&empty_secrets);
            self.initialize_session_structure(session_id, secrets_ref, instructions.as_deref(), setup.as_deref()).await?;
        }

        // Update database with container ID
        sqlx::query("UPDATE sessions SET container_id = ? WHERE id = ?")
            .bind(&container.id)
            .bind(session_id)
            .execute(&self.db_pool)
            .await?;

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