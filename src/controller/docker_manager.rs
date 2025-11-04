use anyhow::Result;
use bollard::{
    container::{Config, CreateContainerOptions, LogsOptions, RemoveContainerOptions},
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
    session_image: String,
    cpu_limit: f64,
    memory_limit: i64,
    db_pool: MySqlPool,
}

fn render_env_file(env: &HashMap<String, String>) -> String {
    let mut lines = String::from(
        "# TaskSandbox session environment\n# Managed by TaskSandbox controller; do not modify without explicit approval.\n",
    );
    let mut entries: Vec<_> = env.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    for (key, value) in entries {
        lines.push_str(&format!("{}={}\n", key, value));
    }
    lines
}

fn parse_env_content(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}

impl DockerManager {
    pub fn new(docker: Docker, db_pool: MySqlPool) -> Self {
        Self {
            docker,
            db_pool,
            session_image: std::env::var("SESSION_IMAGE")
                .unwrap_or_else(|_| "tsbx_session:latest".to_string()),
            cpu_limit: std::env::var("SESSION_CPU_LIMIT")
                .unwrap_or_else(|_| "0.5".to_string())
                .parse()
                .unwrap_or(0.5),
            memory_limit: std::env::var("SESSION_MEMORY_LIMIT")
                .unwrap_or_else(|_| "536870912".to_string())
                .parse()
                .unwrap_or(536870912),
        }
    }

    // NEW: Create session volume with explicit naming
    async fn create_session_volume(&self, session_id: &str) -> Result<String> {
        let volume_name = format!("tsbx_session_data_{}", session_id);

        let mut labels = HashMap::new();
        labels.insert("tsbx.session_id".to_string(), session_id.to_string());
        labels.insert("tsbx.type".to_string(), "session_volume".to_string());
        labels.insert(
            "tsbx.created_at".to_string(),
            chrono::Utc::now().to_rfc3339(),
        );

        let volume_config = bollard::volume::CreateVolumeOptions {
            name: volume_name.clone(),
            driver: "local".to_string(),
            driver_opts: HashMap::new(),
            labels,
        };

        self.docker.create_volume(volume_config).await?;
        info!("Created session volume: {}", volume_name);

        Ok(volume_name)
    }

    // Get session volume name (derived from session ID)
    fn get_session_volume_name(&self, session_id: &str) -> String {
        format!("tsbx_session_data_{}", session_id)
    }

    // Get session container name (derived from session ID)
    fn get_session_container_name(&self, session_id: &str) -> String {
        format!("tsbx_session_{}", session_id)
    }

    // Check if session volume exists
    async fn session_volume_exists(&self, session_id: &str) -> Result<bool> {
        let volume_name = self.get_session_volume_name(session_id);
        match self.docker.inspect_volume(&volume_name).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    // Initialize session directory structure with env, instructions, and setup
    async fn initialize_session_structure(
        &self,
        session_id: &str,
        env: &HashMap<String, String>,
        instructions: Option<&str>,
        setup: Option<&str>,
    ) -> Result<()> {
        info!(
            "Initializing session structure for session {}",
            session_id
        );

        // Create base directories with proper ownership
        // Use sudo to ensure proper ownership since volume may be root-owned initially
        let init_script = "sudo mkdir -p /session/logs
sudo touch /session/.env
sudo chown session:session /session/.env
sudo chmod 600 /session/.env
sudo chown -R session:session /session
sudo chmod -R 755 /session
echo 'Session directories created (.env, logs)'
";

        self.execute_command(session_id, init_script).await?;

        // Write env values to /session/.env
        let env_content = render_env_file(env);
        let write_env_script = format!(
            "cat <<'EOF_ENV' | sudo tee /session/.env >/dev/null\n{}EOF_ENV\nsudo chown session:session /session/.env\nsudo chmod 600 /session/.env\n",
            env_content
        );
        self.execute_command(session_id, &write_env_script)
            .await?;

        // Write instructions if provided
        if let Some(instructions_content) = instructions {
            let escaped_instructions = instructions_content.replace("'", "'\"'\"'");
            let write_instructions_command = format!(
                "echo '{}' > /session/instructions.md",
                escaped_instructions
            );
            self.execute_command(session_id, &write_instructions_command)
                .await?;
        }

        // Write and make setup script executable if provided
        if let Some(setup_content) = setup {
            let escaped_setup = setup_content.replace("'", "'\"'\"'");
            let write_setup_command = format!(
                "echo '{}' > /session/setup.sh && chmod +x /session/setup.sh",
                escaped_setup
            );
            self.execute_command(session_id, &write_setup_command)
                .await?;
        }

        info!(
            "Session structure initialized with {} env entries",
            env.len()
        );

        Ok(())
    }

    // NEW: Check if session is initialized
    async fn session_initialized(&self, _volume_name: &str) -> Result<bool> {
        // Skip initialization check for now - always initialize
        Ok(false)
    }

    // NEW: Cleanup session volume
    pub async fn cleanup_session_volume(&self, session_id: &str) -> Result<()> {
        let expected_volume_name = format!("tsbx_session_data_{}", session_id);

        match self.docker.remove_volume(&expected_volume_name, None).await {
            Ok(_) => {
                info!("Removed session volume: {}", expected_volume_name);
            }
            Err(e) => warn!(
                "Failed to remove session volume {}: {}",
                expected_volume_name, e
            ),
        }

        Ok(())
    }

    pub async fn create_container_with_volume_copy(
        &self,
        session_id: &str,
        parent_session_id: &str,
    ) -> Result<String> {
        info!(
            "Creating cloned session {} with volume copy from {}",
            session_id, parent_session_id
        );

        // First create the container normally (this creates the empty target volume)
        let container_name = self.create_container(session_id).await?;

        // Then copy data from parent volume to new volume using Docker command
        let parent_volume = format!("tsbx_session_data_{}", parent_session_id);
        let new_volume = format!("tsbx_session_data_{}", session_id);

        info!(
            "Copying volume data from {} to {}",
            parent_volume, new_volume
        );

        // Use bollard Docker API to create copy container
        let copy_container_name = format!("tsbx_volume_copy_{}", session_id);

        let config = Config {
            image: Some(self.session_image.clone()),
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
                network_mode: Some("tsbx_network".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        // Create copy container
        self.docker
            .create_container(
                Some(CreateContainerOptions {
                    name: copy_container_name.clone(),
                    ..Default::default()
                }),
                config,
            )
            .await?;

        // Start container
        self.docker
            .start_container::<String>(&copy_container_name, None)
            .await?;

        // Wait for completion
        let mut wait_stream = self
            .docker
            .wait_container::<String>(&copy_container_name, None);
        while let Some(wait_result) = wait_stream.next().await {
            let exit_result = wait_result?;
            if exit_result.status_code == 0 {
                info!("Volume copy completed successfully");
            } else {
                warn!(
                    "Volume copy container exited with code {}",
                    exit_result.status_code
                );
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
            }),
        );

        let log_output = logs
            .map(|log| match log {
                Ok(line) => String::from_utf8_lossy(&line.into_bytes()).to_string(),
                Err(_) => String::new(),
            })
            .collect::<Vec<_>>()
            .await
            .join("");

        info!("Volume copy logs: {}", log_output.trim());

        // Clean up copy container
        let _ = self
            .docker
            .remove_container(
                &copy_container_name,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await;

        Ok(container_name)
    }

    pub async fn create_container_with_full_copy(
        &self,
        session_id: &str,
        parent_session_id: &str,
    ) -> Result<String> {
        info!("Creating cloned session {} with full copy from {}",
              session_id, parent_session_id);

        // First create the session volume (without starting container)
        let session_volume = self.create_session_volume(session_id).await?;

        // Then copy specific directories from parent volume to new volume
        let parent_volume = format!("tsbx_session_data_{}", parent_session_id);
        let new_volume = format!("tsbx_session_data_{}", session_id);

        info!(
            "Copying full volume data from {} to {}",
            parent_volume, new_volume
        );

        // Copy entire /session directory from parent
        let copy_command = "cp -a /source/. /dest/ 2>/dev/null || echo 'No source data'; echo 'Full volume copy completed'".to_string();

        // Use bollard Docker API to create copy container
        let copy_container_name = format!("tsbx_volume_copy_{}", session_id);

        let config = Config {
            image: Some(self.session_image.clone()),
            user: Some("root".to_string()),
            cmd: Some(vec!["bash".to_string(), "-c".to_string(), copy_command]),
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
                    },
                ]),
                network_mode: Some("tsbx_network".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        // Create copy container
        self.docker
            .create_container(
                Some(CreateContainerOptions {
                    name: copy_container_name.clone(),
                    ..Default::default()
                }),
                config,
            )
            .await?;

        // Start container
        self.docker
            .start_container::<String>(&copy_container_name, None)
            .await?;

        // Wait for completion
        let mut wait_stream = self
            .docker
            .wait_container::<String>(&copy_container_name, None);
        while let Some(wait_result) = wait_stream.next().await {
            let exit_result = wait_result?;
            if exit_result.status_code == 0 {
                info!("Full volume copy completed successfully");
            } else {
                warn!(
                    "Full volume copy container exited with code {}",
                    exit_result.status_code
                );
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
            }),
        );

        let log_output = logs
            .map(|log| match log {
                Ok(line) => String::from_utf8_lossy(&line.into_bytes()).to_string(),
                Err(_) => String::new(),
            })
            .collect::<Vec<_>>()
            .await
            .join("");

        info!("Full copy logs: {}", log_output.trim());

        // Read env from copied volume
        let env = match self.read_env_from_volume(&new_volume).await {
            Ok(map) => map,
            Err(e) => {
                warn!(
                    "Failed to read env file from copied volume {}: {}",
                    new_volume, e
                );
                HashMap::new()
            }
        };

        info!("Environment entries in cloned session: {}", env.len());

        // Clean up copy container
        let _ = self
            .docker
            .remove_container(
                &copy_container_name,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await;

        // Read instructions and setup from cloned volume
        let instructions = self
            .read_file_from_volume(&new_volume, "instructions.md")
            .await
            .ok();

        let setup = self
            .read_file_from_volume(&new_volume, "setup.sh")
            .await
            .ok();

        info!(
            "Creating container with {} env from copied volume",
            env.len()
        );

        // Now create and start the container with the copied env as environment variables
        let container_name = self
            .create_container_internal(session_id, Some(env), instructions, setup)
            .await?;

        Ok(container_name)
    }

    pub async fn create_container_with_full_copy_and_tokens(
        &self,
        session_id: &str,
        parent_session_id: &str,
        tsbx_token: String,
        principal: String,
        principal_type: String,
        request_created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<String> {
        info!(
            "Creating cloned session {} with full copy from {} and fresh tokens",
            session_id, parent_session_id
        );

        // First create the session volume (without starting container)
        let session_volume = self.create_session_volume(session_id).await?;

        // Then copy specific directories from parent volume to new volume
        let parent_volume = format!("tsbx_session_data_{}", parent_session_id);
        let new_volume = format!("tsbx_session_data_{}", session_id);

        info!(
            "Copying full volume data from {} to {}",
            parent_volume, new_volume
        );

        // Copy entire /session directory from parent
        let copy_command = "cp -a /source/. /dest/ 2>/dev/null || echo 'No source data'; echo 'Full volume copy completed'".to_string();

        // Use bollard Docker API to create copy container
        let copy_container_name = format!("tsbx_volume_copy_{}", session_id);

        let config = Config {
            image: Some(self.session_image.clone()),
            user: Some("root".to_string()),
            cmd: Some(vec!["bash".to_string(), "-c".to_string(), copy_command]),
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
                    },
                ]),
                network_mode: Some("tsbx_network".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        // Create copy container
        self.docker
            .create_container(
                Some(CreateContainerOptions {
                    name: copy_container_name.clone(),
                    ..Default::default()
                }),
                config,
            )
            .await?;

        // Start container
        self.docker
            .start_container::<String>(&copy_container_name, None)
            .await?;

        // Wait for completion
        let mut wait_stream = self
            .docker
            .wait_container::<String>(&copy_container_name, None);
        while let Some(wait_result) = wait_stream.next().await {
            let exit_result = wait_result?;
            if exit_result.status_code == 0 {
                info!("Full volume copy completed successfully");
            } else {
                warn!(
                    "Full volume copy container exited with code {}",
                    exit_result.status_code
                );
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
            }),
        );

        let log_output = logs
            .map(|log| match log {
                Ok(line) => String::from_utf8_lossy(&line.into_bytes()).to_string(),
                Err(_) => String::new(),
            })
            .collect::<Vec<_>>()
            .await
            .join("");

        info!("Full volume copy logs: {}", log_output.trim());

        // Always read env from copied volume
        let env = match self.read_env_from_volume(&new_volume).await {
            Ok(map) => map,
            Err(e) => {
                warn!(
                    "Failed to read env file from copied volume {}: {}",
                    new_volume, e
                );
                HashMap::new()
            }
        };

        info!(
            "Successfully collected {} user env entries from copy",
            env.len()
        );

        // Clean up copy container
        let _ = self
            .docker
            .remove_container(
                &copy_container_name,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await;

        // Read instructions and setup from cloned volume
        let instructions = self
            .read_file_from_volume(&new_volume, "instructions.md")
            .await
            .ok();

        let setup = self
            .read_file_from_volume(&new_volume, "setup.sh")
            .await
            .ok();

        info!(
            "Creating cloned container with {} user env and fresh system tokens",
            env.len()
        );

        // Now create and start the container with user env + generated system tokens
        let container_name = self
            .create_container_internal_with_tokens(
                session_id,
                Some(env),
                instructions,
                setup,
                tsbx_token,
                principal,
                principal_type,
                Some(request_created_at),
            )
            .await?;

        Ok(container_name)
    }

    // Helper method to read env from a volume
    async fn read_env_from_volume(
        &self,
        volume_name: &str,
    ) -> Result<std::collections::HashMap<String, String>> {
        let content = self.read_file_from_volume(volume_name, ".env").await?;
        Ok(parse_env_content(&content))
    }

    // Helper method to read a file from a volume
    async fn read_file_from_volume(&self, volume_name: &str, file_path: &str) -> Result<String> {
        let read_container_name = format!(
            "tsbx_read_file_{}",
            Uuid::new_v4().to_string()[..8].to_string()
        );

        let config = Config {
            image: Some(self.session_image.clone()),
            user: Some("root".to_string()),
            cmd: Some(vec![
                "bash".to_string(),
                "-c".to_string(),
                format!(
                    "if [ -f /volume/{} ]; then cat /volume/{}; fi",
                    file_path, file_path
                ),
            ]),
            host_config: Some(HostConfig {
                mounts: Some(vec![Mount {
                    typ: Some(MountTypeEnum::VOLUME),
                    source: Some(volume_name.to_string()),
                    target: Some("/volume".to_string()),
                    read_only: Some(true),
                    ..Default::default()
                }]),
                network_mode: Some("tsbx_network".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        // Create and start container
        self.docker
            .create_container(
                Some(CreateContainerOptions {
                    name: read_container_name.clone(),
                    ..Default::default()
                }),
                config,
            )
            .await?;

        self.docker
            .start_container::<String>(&read_container_name, None)
            .await?;

        // Wait for completion
        let mut wait_stream = self
            .docker
            .wait_container::<String>(&read_container_name, None);
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
            }),
        );

        let content = logs
            .map(|log| match log {
                Ok(line) => String::from_utf8_lossy(&line.into_bytes()).to_string(),
                Err(_) => String::new(),
            })
            .collect::<Vec<_>>()
            .await
            .join("");

        // Clean up the read container
        let _ = self
            .docker
            .remove_container(
                &read_container_name,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await;

        if content.trim().is_empty() {
            Err(anyhow::anyhow!("File {} not found or empty", file_path))
        } else {
            Ok(content.trim().to_string())
        }
    }

    pub async fn create_container_with_params(
        &self,
        session_id: &str,
        env: std::collections::HashMap<String, String>,
        instructions: Option<String>,
        setup: Option<String>,
    ) -> Result<String> {
        let container_name = self
            .create_container_internal(session_id, Some(env), instructions, setup)
            .await?;
        Ok(container_name)
    }

    pub async fn create_container_with_params_and_tokens(
        &self,
        session_id: &str,
        env: std::collections::HashMap<String, String>,
        instructions: Option<String>,
        setup: Option<String>,
        tsbx_token: String,
        principal: String,
        principal_type: String,
        request_created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<String> {
        let container_name = self
            .create_container_internal_with_tokens(
                session_id,
                Some(env),
                instructions,
                setup,
                tsbx_token,
                principal,
                principal_type,
                Some(request_created_at),
            )
            .await?;
        Ok(container_name)
    }

    pub async fn create_container(&self, session_id: &str) -> Result<String> {
        let container_name = self
            .create_container_internal(session_id, None, None, None)
            .await?;
        Ok(container_name)
    }

    pub async fn restart_container(&self, session_id: &str) -> Result<String> {
        // Read existing env from the volume
        let volume_name = format!("tsbx_session_data_{}", session_id);
        info!(
            "Restarting container for session {} - reading env from volume {}",
            session_id, volume_name
        );

        let env = match self.read_env_from_volume(&volume_name).await {
            Ok(s) => {
                info!(
                    "Found {} env in volume for session {}",
                    s.len(),
                    session_id
                );
                for key in s.keys() {
                    info!("  - Env key: {}", key);
                }
                Some(s)
            }
            Err(e) => {
                warn!(
                    "Could not read env from volume for session {}: {}",
                    session_id, e
                );
                None
            }
        };

        // Read existing instructions and setup from volume
        let instructions = self
            .read_file_from_volume(&volume_name, "instructions.md")
            .await
            .ok();
        let setup = self
            .read_file_from_volume(&volume_name, "setup.sh")
            .await
            .ok();

        let container_name = self
            .create_container_internal(session_id, env, instructions, setup)
            .await?;
        Ok(container_name)
    }

    pub async fn restart_container_with_tokens(
        &self,
        session_id: &str,
        tsbx_token: String,
        principal: String,
        principal_type: String,
        request_created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<String> {
        // Read existing user env from the volume (but generate fresh system tokens)
        let volume_name = format!("tsbx_session_data_{}", session_id);
        info!(
            "Restarting container for session {} with fresh tokens",
            session_id
        );

        let env = match self.read_env_from_volume(&volume_name).await {
            Ok(s) => {
                info!(
                    "Found {} user env in volume for session {}",
                    s.len(),
                    session_id
                );
                Some(s)
            }
            Err(e) => {
                warn!(
                    "Could not read env from volume for session {}: {}",
                    session_id, e
                );
                None
            }
        };

        // Read existing instructions and setup from volume
        let instructions = self
            .read_file_from_volume(&volume_name, "instructions.md")
            .await
            .ok();
        let setup = self
            .read_file_from_volume(&volume_name, "setup.sh")
            .await
            .ok();

        let container_name = self
            .create_container_internal_with_tokens(
                session_id,
                env,
                instructions,
                setup,
                tsbx_token,
                principal,
                principal_type,
                Some(request_created_at),
            )
            .await?;
        Ok(container_name)
    }

    async fn create_container_internal(
        &self,
        session_id: &str,
        env_map: Option<std::collections::HashMap<String, String>>,
        instructions: Option<String>,
        setup: Option<String>,
    ) -> Result<String> {
        let container_name = format!("tsbx_session_{}", session_id);

        // No content port mapping; preview server is removed.

        // Use session image directly for all sessions
        let container_image = self.session_image.clone();
        info!(
            "Creating container {} with session image {},",
            container_name, container_image
        );

        info!("Creating container for session {}", session_id);

        // Create or get existing session volume
        let session_volume = if self.session_volume_exists(session_id).await? {
            self.get_session_volume_name(session_id)
        } else {
            self.create_session_volume(session_id).await?
        };

        let mut labels = HashMap::new();
        labels.insert("tsbx.session_id".to_string(), session_id.to_string());
        labels.insert("tsbx.managed".to_string(), "true".to_string());
        labels.insert("tsbx.volume".to_string(), session_volume.clone());

        // Get user token from env (added automatically by session manager)
        let user_token = env_map
            .as_ref()
            .and_then(|s| s.get("TSBX_TOKEN"))
            .cloned()
            .unwrap_or_else(|| {
                warn!("No TSBX_TOKEN found in env, Host authentication may fail");
                "missing-token".to_string()
            });

        // Configure volume mounts
        let mounts = vec![bollard::models::Mount {
            typ: Some(bollard::models::MountTypeEnum::VOLUME),
            source: Some(session_volume.clone()),
            target: Some("/session".to_string()),
            read_only: Some(false),
            ..Default::default()
        }];

        // No port bindings or exposed ports needed.

        // Set environment variables for the session structure
        let mut env_vars = vec![
            format!("TSBX_API_URL=http://tsbx_api:9000"),
            format!("SESSION_ID={}", session_id),
            format!("TSBX_SESSION_DIR=/session"),
        ];

        // Propagate host branding and URL to sessions (provided by start script)
        let host_name =
            std::env::var("TSBX_HOST_NAME").unwrap_or_else(|_| "TaskSandbox".to_string());
        let host_url =
            std::env::var("TSBX_HOST_URL").expect("TSBX_HOST_URL must be set by the start script");
        env_vars.push(format!("TSBX_HOST_NAME={}", host_name));
        env_vars.push(format!("TSBX_HOST_URL={}", host_url));

        // Configure Ollama host for model inference (required; no default)
        let ollama_host = std::env::var("OLLAMA_HOST").map_err(|_| {
            anyhow::anyhow!("Controller requires OLLAMA_HOST to be set (e.g., http://ollama:11434)")
        })?;
        env_vars.push(format!("OLLAMA_HOST={}", ollama_host));
        let ollama_model =
            std::env::var("TSBX_DEFAULT_MODEL").unwrap_or_else(|_| "gpt-oss:20b".to_string());
        env_vars.push(format!("TSBX_DEFAULT_MODEL={}", ollama_model));
        let ollama_timeout =
            std::env::var("OLLAMA_TIMEOUT_SECS").unwrap_or_else(|_| "600".to_string());
        env_vars.push(format!("OLLAMA_TIMEOUT_SECS={}", ollama_timeout));
        // Propagate timeout for model calls; default 600s if unspecified
        let ollama_timeout =
            std::env::var("OLLAMA_TIMEOUT_SECS").unwrap_or_else(|_| "600".to_string());
        env_vars.push(format!("OLLAMA_TIMEOUT_SECS={}", ollama_timeout));

        // No web_search tool; do not propagate BRAVE_API_KEY

        // Add hint about setup script availability to avoid unnecessary waiting
        if setup.is_some() {
            env_vars.push("TSBX_HAS_SETUP=true".to_string());
        }

        // Add principal information as environment variables
        if let Some(env_map) = &env_map {
            // Extract principal info from TSBX_TOKEN if available
            if let Some(_token) = env_map.get("TSBX_TOKEN") {
                // Set environment variables for Host principal logging
                env_vars.push(format!(
                    "TSBX_PRINCIPAL={}",
                    env_map
                        .get("TSBX_PRINCIPAL")
                        .unwrap_or(&"unknown".to_string())
                ));
                env_vars.push(format!(
                    "TSBX_PRINCIPAL_TYPE={}",
                    env_map
                        .get("TSBX_PRINCIPAL_TYPE")
                        .unwrap_or(&"unknown".to_string())
                ));
            }

            // Add user env as environment variables, but do NOT override
            // system-managed values like TSBX_TOKEN or OLLAMA_HOST.
            for (key, value) in env_map {
                if key == "TSBX_TOKEN" || key == "OLLAMA_HOST" {
                    info!(
                        "Skipping user-provided {} - using system-managed value instead for session {}",
                        key, session_id
                    );
                    continue;
                }
                env_vars.push(format!("{}={}", key, value));
                if key != "TSBX_PRINCIPAL" && key != "TSBX_PRINCIPAL_TYPE" {
                    info!(
                        "Adding env entry {} as environment variable for session {}",
                        key, session_id
                    );
                }
            }
        }

        // Set the command with required arguments
        let cmd = vec![
            "tsbx-session".to_string(),
            "--api-url".to_string(),
            "http://tsbx_api:9000".to_string(),
            "--session-id".to_string(),
            session_id.to_string(),
        ];

        let config = Config {
            image: Some(container_image),
            hostname: Some(format!(
                "session-{}",
                &session_id[..session_id.len().min(8)]
            )),
            labels: Some(labels),
            env: Some(env_vars),
            cmd: Some(cmd),
            working_dir: Some("/session".to_string()), // User starts in their session
            exposed_ports: None,
            host_config: Some(bollard::models::HostConfig {
                cpu_quota: Some((self.cpu_limit * 100000.0) as i64),
                cpu_period: Some(100000),
                memory: Some(self.memory_limit),
                memory_swap: Some(self.memory_limit),
                network_mode: Some("tsbx_network".to_string()),
                mounts: Some(mounts),
                port_bindings: None,
                extra_hosts: Some(vec!["host.docker.internal:host-gateway".to_string()]),
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

        // Initialize session structure after starting container so host can execute setup script
        if !self.session_initialized(&session_volume).await? {
            let empty_env: HashMap<String, String> = HashMap::new();
            let env_ref = env_map.as_ref().unwrap_or(&empty_env);
            self.initialize_session_structure(
                session_id,
                env_ref,
                instructions.as_deref(),
                setup.as_deref(),
            )
            .await?;
        }

        info!(
            "Container {} created with session volume {}",
            container_name, session_volume
        );
        Ok(container.id)
    }

    async fn create_container_internal_with_tokens(
        &self,
        session_id: &str,
        env_map_opt: Option<std::collections::HashMap<String, String>>,
        instructions: Option<String>,
        setup: Option<String>,
        tsbx_token: String,
        principal: String,
        principal_type: String,
        request_created_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<String> {
        let container_name = format!("tsbx_session_{}", session_id);

        // No content port mapping; preview server is removed.

        // Use session image directly for all sessions
        let container_image = self.session_image.clone();
        info!(
            "Creating container {} with session image and generated tokens",
            container_name
        );

        info!(
            "Creating container for session {} with fresh tokens",
            session_id
        );

        // Create or get existing session volume
        let session_volume = if self.session_volume_exists(session_id).await? {
            self.get_session_volume_name(session_id)
        } else {
            self.create_session_volume(session_id).await?
        };

        let mut labels = HashMap::new();
        labels.insert("tsbx.session_id".to_string(), session_id.to_string());
        labels.insert("tsbx.managed".to_string(), "true".to_string());
        labels.insert("tsbx.volume".to_string(), session_volume.clone());

        // Configure volume mounts
        let mounts = vec![bollard::models::Mount {
            typ: Some(bollard::models::MountTypeEnum::VOLUME),
            source: Some(session_volume.clone()),
            target: Some("/session".to_string()),
            read_only: Some(false),
            ..Default::default()
        }];

        // No port bindings or exposed ports needed.

        // Set environment variables for the session structure
        let mut env_vars = vec![
            format!("TSBX_API_URL=http://tsbx_api:9000"),
            format!("SESSION_ID={}", session_id),
            format!("TSBX_SESSION_DIR=/session"),
            // Set the generated system tokens directly as environment variables
            format!("TSBX_TOKEN={}", tsbx_token),
            format!("TSBX_PRINCIPAL={}", principal),
            format!("TSBX_PRINCIPAL_TYPE={}", principal_type),
        ];

        // Propagate host branding and URL to sessions (provided by start script)
        let host_name =
            std::env::var("TSBX_HOST_NAME").unwrap_or_else(|_| "TaskSandbox".to_string());
        let host_url =
            std::env::var("TSBX_HOST_URL").expect("TSBX_HOST_URL must be set by the start script");
        env_vars.push(format!("TSBX_HOST_NAME={}", host_name));
        env_vars.push(format!("TSBX_HOST_URL={}", host_url));

        // Configure Ollama host for model inference (required; no default)
        let ollama_host = std::env::var("OLLAMA_HOST").map_err(|_| {
            anyhow::anyhow!("Controller requires OLLAMA_HOST to be set (e.g., http://ollama:11434)")
        })?;
        env_vars.push(format!("OLLAMA_HOST={}", ollama_host));
        let ollama_model =
            std::env::var("TSBX_DEFAULT_MODEL").unwrap_or_else(|_| "gpt-oss:20b".to_string());
        env_vars.push(format!("TSBX_DEFAULT_MODEL={}", ollama_model));

        // No web_search tool; do not propagate BRAVE_API_KEY

        // Add hint about setup script availability to avoid unnecessary waiting
        if setup.is_some() {
            env_vars.push("TSBX_HAS_SETUP=true".to_string());
        }

        // Add request creation timestamp for message processing
        if let Some(timestamp) = request_created_at {
            env_vars.push(format!(
                "TSBX_REQUEST_CREATED_AT={}",
                timestamp.to_rfc3339()
            ));
        }

        info!("Set TSBX_TOKEN and OLLAMA_HOST as environment variables");

        // Add user env as environment variables (but NOT TSBX_TOKEN or OLLAMA_HOST)
        if let Some(env_map) = &env_map_opt {
            for (key, value) in env_map {
                // Skip if user provided their own TSBX_TOKEN or OLLAMA_HOST - we use system-managed values
                if key == "TSBX_TOKEN" || key == "OLLAMA_HOST" {
                    info!(
                        "Skipping user-provided {} - using system-managed value instead",
                        key
                    );
                    continue;
                }
                env_vars.push(format!("{}={}", key, value));
                info!(
                    "Adding user env entry {} as environment variable for session {}",
                    key, session_id
                );
            }
        }

        // Set the command with required arguments
        let cmd = vec![
            "tsbx-session".to_string(),
            "--api-url".to_string(),
            "http://tsbx_api:9000".to_string(),
            "--session-id".to_string(),
            session_id.to_string(),
        ];

        let config = Config {
            image: Some(container_image),
            hostname: Some(format!(
                "session-{}",
                &session_id[..session_id.len().min(8)]
            )),
            labels: Some(labels),
            env: Some(env_vars),
            cmd: Some(cmd),
            working_dir: Some("/session".to_string()), // User starts in their session
            exposed_ports: None,
            host_config: Some(bollard::models::HostConfig {
                cpu_quota: Some((self.cpu_limit * 100000.0) as i64),
                cpu_period: Some(100000),
                memory: Some(self.memory_limit),
                memory_swap: Some(self.memory_limit),
                network_mode: Some("tsbx_network".to_string()),
                mounts: Some(mounts),
                port_bindings: None,
                extra_hosts: Some(vec!["host.docker.internal:host-gateway".to_string()]),
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

        // Initialize session structure after starting container so host can execute setup script
        // Only initialize with user env (system tokens are already in environment)
        if !self.session_initialized(&session_volume).await? {
            let empty_env: HashMap<String, String> = HashMap::new();
            let env_ref = env_map_opt.as_ref().unwrap_or(&empty_env);
            self.initialize_session_structure(
                session_id,
                env_ref,
                instructions.as_deref(),
                setup.as_deref(),
            )
            .await?;
        }

        info!(
            "Container {} created with session volume {} and fresh system tokens",
            container_name, session_volume
        );
        Ok(container.id)
    }

    // Stop container but retain persistent volume (for session pause/stop)
    pub async fn stop_container(&self, session_id: &str) -> Result<()> {
        let container_name = format!("tsbx_session_{}", session_id);

        info!("Stopping container {}", container_name);

        let options = RemoveContainerOptions {
            force: true,
            ..Default::default()
        };

        match self
            .docker
            .remove_container(&container_name, Some(options))
            .await
        {
            Ok(_) => {
                info!(
                    "Container {} stopped, persistent volume retained",
                    container_name
                );
                Ok(())
            }
            Err(e) => {
                if e.to_string().contains("404") || e.to_string().contains("No such container") {
                    warn!(
                        "Container {} already removed or doesn't exist, treating as success",
                        container_name
                    );
                    Ok(())
                } else {
                    error!("Failed to stop container {}: {}", container_name, e);
                    Err(anyhow::anyhow!("Failed to stop container: {}", e))
                }
            }
        }
    }

    // Delete container and remove persistent volume (for session deletion)
    pub async fn delete_container(&self, session_id: &str) -> Result<()> {
        let container_name = format!("tsbx_session_{}", session_id);

        info!("Deleting container {}", container_name);

        let options = RemoveContainerOptions {
            force: true,
            ..Default::default()
        };

        match self
            .docker
            .remove_container(&container_name, Some(options))
            .await
        {
            Ok(_) => {
                info!("Container {} deleted", container_name);

                // Cleanup the session volume
                if let Err(e) = self.cleanup_session_volume(session_id).await {
                    warn!(
                        "Failed to cleanup session volume for {}: {}",
                        session_id, e
                    );
                }

                Ok(())
            }
            Err(e) => {
                if e.to_string().contains("404") || e.to_string().contains("No such container") {
                    warn!("Container {} already removed or doesn't exist, proceeding with volume cleanup", container_name);

                    // Still try to cleanup the session volume
                    if let Err(e) = self.cleanup_session_volume(session_id).await {
                        warn!(
                            "Failed to cleanup session volume for {}: {}",
                            session_id, e
                        );
                    }

                    Ok(())
                } else {
                    error!("Failed to delete container {}: {}", container_name, e);
                    Err(anyhow::anyhow!("Failed to delete container: {}", e))
                }
            }
        }
    }

    // Removed legacy destroy_container (deprecated). Use stop_container or delete_container.

    pub async fn execute_command(&self, session_id: &str, command: &str) -> Result<String> {
        let container_name = format!("tsbx_session_{}", session_id);

        info!(
            "Executing command in container {}: {}",
            container_name, command
        );

        let exec_config = CreateExecOptions {
            cmd: Some(vec!["/bin/bash", "-c", command]),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        let exec = self
            .docker
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

    // Execute a command and collect stdout/stderr bytes with exit code
    pub async fn exec_collect(
        &self,
        session_id: &str,
        cmd: Vec<String>,
    ) -> Result<(i32, Vec<u8>, Vec<u8>)> {
        let container_name = format!("tsbx_session_{}", session_id);
        let exec_config = CreateExecOptions {
            cmd: Some(cmd),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        let exec = self
            .docker
            .create_exec(&container_name, exec_config)
            .await?;
        let mut out_buf: Vec<u8> = Vec::new();
        let mut err_buf: Vec<u8> = Vec::new();
        if let StartExecResults::Attached { mut output, .. } =
            self.docker.start_exec(&exec.id, None).await?
        {
            while let Some(Ok(frame)) = output.next().await {
                use bollard::container::LogOutput;
                match frame {
                    LogOutput::StdOut { message } => out_buf.extend_from_slice(&message),
                    LogOutput::StdErr { message } => err_buf.extend_from_slice(&message),
                    LogOutput::Console { message } => out_buf.extend_from_slice(&message),
                    other => {
                        let bytes = other.into_bytes();
                        out_buf.extend_from_slice(&bytes);
                    }
                }
            }
        }
        // Inspect for exit code
        let inspect = self.docker.inspect_exec(&exec.id).await?;
        let code_i64 = inspect.exit_code.unwrap_or(0);
        let code = i32::try_from(code_i64).unwrap_or_else(|_| if code_i64 == 0 { 0 } else { 1 });
        Ok((code, out_buf, err_buf))
    }

    /// Check if a session container exists and is running healthily
    pub async fn is_container_healthy(&self, session_id: &str) -> Result<bool> {
        let container_name = format!("tsbx_session_{}", session_id);

        // First check if container exists
        match self.docker.inspect_container(&container_name, None).await {
            Ok(container_info) => {
                // Container exists, check if it's running
                if let Some(state) = container_info.state {
                    if let Some(running) = state.running {
                        if running {
                            // Container is running, do a basic health check by trying to exec a command
                            return self.ping_container(&container_name).await;
                        } else {
                            // Container exists but is not running
                            info!(
                                "Session {} container exists but is not running",
                                session_id
                            );
                            return Ok(false);
                        }
                    }
                }
                // Container state is unclear, assume unhealthy
                warn!("Session {} container state is unclear", session_id);
                Ok(false)
            }
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            }) => {
                // Container doesn't exist
                info!("Session {} container does not exist", session_id);
                Ok(false)
            }
            Err(e) => {
                // Other Docker API error
                error!(
                    "Failed to inspect session {} container: {}",
                    session_id, e
                );
                Err(anyhow::anyhow!(
                    "Docker API error for session {}: {}",
                    session_id,
                    e
                ))
            }
        }
    }

    /// Ping a container to verify it's responding
    async fn ping_container(&self, container_name: &str) -> Result<bool> {
        let exec_config = CreateExecOptions {
            cmd: Some(vec!["echo", "ping"]),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        match self.docker.create_exec(container_name, exec_config).await {
            Ok(exec_info) => {
                match self.docker.start_exec(&exec_info.id, None).await {
                    Ok(StartExecResults::Attached { .. }) => {
                        // Command executed successfully
                        Ok(true)
                    }
                    Ok(StartExecResults::Detached) => {
                        // Command was detached, assume success
                        Ok(true)
                    }
                    Err(e) => {
                        warn!("Failed to ping container {}: {}", container_name, e);
                        Ok(false)
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Failed to create exec for container {}: {}",
                    container_name, e
                );
                Ok(false)
            }
        }
    }
}
