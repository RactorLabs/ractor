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
        "# Ractor session environment\n# Managed by Ractor controller; do not modify without explicit approval.\n",
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
                .unwrap_or_else(|_| "ractor_session:latest".to_string()),
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
    async fn create_session_volume(&self, session_name: &str) -> Result<String> {
        let volume_name = format!("ractor_session_data_{}", session_name.to_ascii_lowercase());

        let mut labels = HashMap::new();
        labels.insert("ractor.session_name".to_string(), session_name.to_string());
        labels.insert("ractor.type".to_string(), "session_volume".to_string());
        labels.insert(
            "ractor.created_at".to_string(),
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

    // Get session volume name (derived from session name). Docker volume names must be lowercase.
    fn get_session_volume_name(&self, session_name: &str) -> String {
        format!("ractor_session_data_{}", session_name.to_ascii_lowercase())
    }

    // Get session container name (derived from session name). Docker container names must be lowercase.
    fn get_session_container_name(&self, session_name: &str) -> String {
        format!("ractor_session_{}", session_name.to_ascii_lowercase())
    }

    // Check if session volume exists
    async fn session_volume_exists(&self, session_name: &str) -> Result<bool> {
        let volume_name = self.get_session_volume_name(session_name);
        match self.docker.inspect_volume(&volume_name).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    // Initialize session directory structure with env, instructions, and setup
    async fn initialize_session_structure(
        &self,
        session_name: &str,
        env: &HashMap<String, String>,
        instructions: Option<&str>,
        setup: Option<&str>,
    ) -> Result<()> {
        info!(
            "Initializing session structure for session {}",
            session_name
        );

        // Create base directories (no data folder in v0.4.0) with proper ownership
        // Use sudo to ensure proper ownership since volume may be root-owned initially
        let init_script = "sudo mkdir -p /session/code /session/logs /session/content /session/template
sudo touch /session/.env
sudo chown session:session /session/.env
sudo chmod 600 /session/.env
sudo chown -R session:session /session
sudo chmod -R 755 /session
# Seed default HTML template if missing
if [ ! -f /session/template/simple.html ] && [ -f /opt/ractor/templates/simple.html ]; then
  sudo cp /opt/ractor/templates/simple.html /session/template/simple.html && sudo chown session:session /session/template/simple.html;
fi
echo 'Session directories created (code, .env, logs, content, template)'
";

        self.execute_command(session_name, init_script).await?;

        // Write env values to /session/.env
        let env_content = render_env_file(env);
        let write_env_script = format!(
            "cat <<'EOF_ENV' | sudo tee /session/.env >/dev/null\n{}EOF_ENV\nsudo chown session:session /session/.env\nsudo chmod 600 /session/.env\n",
            env_content
        );
        self.execute_command(session_name, &write_env_script)
            .await?;

        // Write instructions if provided
        if let Some(instructions_content) = instructions {
            let escaped_instructions = instructions_content.replace("'", "'\"'\"'");
            let write_instructions_command = format!(
                "echo '{}' > /session/code/instructions.md",
                escaped_instructions
            );
            self.execute_command(session_name, &write_instructions_command)
                .await?;
        }

        // Write and make setup script executable if provided
        if let Some(setup_content) = setup {
            let escaped_setup = setup_content.replace("'", "'\"'\"'");
            let write_setup_command = format!(
                "echo '{}' > /session/code/setup.sh && chmod +x /session/code/setup.sh",
                escaped_setup
            );
            self.execute_command(session_name, &write_setup_command)
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
    pub async fn cleanup_session_volume(&self, session_name: &str) -> Result<()> {
        let expected_volume_name =
            format!("ractor_session_data_{}", session_name.to_ascii_lowercase());

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
        session_name: &str,
        parent_session_name: &str,
    ) -> Result<String> {
        info!(
            "Creating remix session {} with volume copy from {}",
            session_name, parent_session_name
        );

        // First create the container normally (this creates the empty target volume)
        let container_name = self.create_container(session_name).await?;

        // Then copy data from parent volume to new volume using Docker command
        let parent_volume = format!(
            "ractor_session_data_{}",
            parent_session_name.to_ascii_lowercase()
        );
        let new_volume = format!("ractor_session_data_{}", session_name.to_ascii_lowercase());

        info!(
            "Copying volume data from {} to {}",
            parent_volume, new_volume
        );

        // Use bollard Docker API to create copy container
        let copy_container_name = format!("ractor_volume_copy_{}", session_name);

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
                network_mode: Some("ractor_network".to_string()),
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

    pub async fn create_container_with_selective_copy(
        &self,
        session_name: &str,
        parent_session_name: &str,
        copy_data: bool,
        copy_code: bool,
        copy_env: bool,
        copy_content: bool,
    ) -> Result<String> {
        info!("Creating remix session {} with selective copy from {} (data: {}, code: {}, env: {}, content: {})", 
              session_name, parent_session_name, copy_data, copy_code, copy_env, copy_content);

        // First create the session volume (without starting container)
        let session_volume = self.create_session_volume(session_name).await?;

        // Then copy specific directories from parent volume to new volume
        let parent_volume = format!(
            "ractor_session_data_{}",
            parent_session_name.to_ascii_lowercase()
        );
        let new_volume = format!("ractor_session_data_{}", session_name.to_ascii_lowercase());

        info!(
            "Copying selective data from {} to {}",
            parent_volume, new_volume
        );

        // Build copy commands based on what should be copied
        let mut copy_commands = Vec::new();

        // Always create base directory structure with proper ownership (run as root to create dirs, then chown to session)
        copy_commands.push(
            "mkdir -p /dest/code /dest/data /dest/content /dest/logs && touch /dest/.env && chown -R 1000:1000 /dest && chmod 600 /dest/.env"
                .to_string(),
        );

        if copy_data {
            copy_commands.push("if [ -d /source/data ]; then cp -a /source/data/. /dest/data/ || echo 'No data to copy'; fi".to_string());
        }

        if copy_code {
            copy_commands.push("if [ -d /source/code ]; then cp -a /source/code/. /dest/code/ || echo 'No code to copy'; fi".to_string());
        }

        if copy_env {
            copy_commands.push(
                "if [ -f /source/.env ]; then cp /source/.env /dest/.env || echo 'No env file copied'; else echo 'No env file to copy'; fi"
                    .to_string(),
            );
        } else {
            copy_commands.push("echo 'Skipping env copy as requested'".to_string());
        }

        if copy_content {
            copy_commands.push("if [ -d /source/content ]; then cp -a /source/content/. /dest/content/ || echo 'No content to copy'; fi".to_string());
        } else {
            copy_commands.push("echo 'Skipping content copy as requested'".to_string());
        }

        // Always copy README.md from root if it exists
        copy_commands.push("if [ -f /source/README.md ]; then cp /source/README.md /dest/ || echo 'No README to copy'; fi".to_string());

        copy_commands.push("echo 'Selective copy completed'".to_string());

        let copy_command = copy_commands.join(" && ");

        // Use bollard Docker API to create copy container
        let copy_container_name = format!("ractor_volume_copy_{}", session_name);

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
                network_mode: Some("ractor_network".to_string()),
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
                info!("Selective volume copy completed successfully");
            } else {
                warn!(
                    "Selective volume copy container exited with code {}",
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

        info!("Selective copy logs: {}", log_output.trim());

        let env = if copy_env {
            match self.read_env_from_volume(&new_volume).await {
                Ok(map) => map,
                Err(e) => {
                    warn!(
                        "Failed to read env file from copied volume {}: {}",
                        new_volume, e
                    );
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        info!("Environment entries copied: {}", env.len());

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

        // Read instructions and setup if code was copied
        let instructions = if copy_code {
            self.read_file_from_volume(&new_volume, "code/instructions.md")
                .await
                .ok()
        } else {
            None
        };

        let setup = if copy_code {
            self.read_file_from_volume(&new_volume, "code/setup.sh")
                .await
                .ok()
        } else {
            None
        };

        info!(
            "Creating container with {} env from copied volume",
            env.len()
        );

        // Now create and start the container with the copied env as environment variables
        let container_name = self
            .create_container_internal(session_name, Some(env), instructions, setup)
            .await?;

        Ok(container_name)
    }

    pub async fn create_container_with_selective_copy_and_tokens(
        &self,
        session_name: &str,
        parent_session_name: &str,
        copy_data: bool,
        copy_code: bool,
        copy_env: bool,
        copy_content: bool,
        ractor_token: String,
        principal: String,
        principal_type: String,
        task_created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<String> {
        info!(
            "Creating remix session {} with selective copy from {} and fresh tokens",
            session_name, parent_session_name
        );

        // First create the session volume (without starting container)
        let session_volume = self.create_session_volume(session_name).await?;

        // Then copy specific directories from parent volume to new volume
        let parent_volume = format!(
            "ractor_session_data_{}",
            parent_session_name.to_ascii_lowercase()
        );
        let new_volume = format!("ractor_session_data_{}", session_name.to_ascii_lowercase());

        info!(
            "Copying selective data from {} to {}",
            parent_volume, new_volume
        );

        // Build copy commands based on what should be copied
        let mut copy_commands = Vec::new();

        // Always create base directory structure with proper ownership (run as root to create dirs, then chown to session)
        copy_commands.push(
            "mkdir -p /dest/code /dest/data /dest/content /dest/logs && touch /dest/.env && chown -R 1000:1000 /dest && chmod 600 /dest/.env"
                .to_string(),
        );

        if copy_data {
            copy_commands.push("if [ -d /source/data ]; then cp -a /source/data/. /dest/data/ || echo 'No data to copy'; fi".to_string());
        }

        if copy_code {
            copy_commands.push("if [ -d /source/code ]; then cp -a /source/code/. /dest/code/ || echo 'No code to copy'; fi".to_string());
        }

        if copy_env {
            copy_commands.push(
                "if [ -f /source/.env ]; then cp /source/.env /dest/.env || echo 'No env file copied'; else echo 'No env file to copy'; fi"
                    .to_string(),
            );
        } else {
            copy_commands.push("echo 'Skipping env copy as requested'".to_string());
        }

        if copy_content {
            copy_commands.push("if [ -d /source/content ]; then cp -a /source/content/. /dest/content/ || echo 'No content to copy'; fi".to_string());
        } else {
            copy_commands.push("echo 'Skipping content copy as requested'".to_string());
        }

        // Always copy README.md from root if it exists
        copy_commands.push("if [ -f /source/README.md ]; then cp /source/README.md /dest/ || echo 'No README to copy'; fi".to_string());

        copy_commands.push("echo 'Selective copy completed'".to_string());

        let copy_command = copy_commands.join(" && ");

        // Use bollard Docker API to create copy container
        let copy_container_name = format!("ractor_volume_copy_{}", session_name);

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
                network_mode: Some("ractor_network".to_string()),
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
                info!("Selective volume copy completed successfully");
            } else {
                warn!(
                    "Selective volume copy container exited with code {}",
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

        info!("Selective copy logs: {}", log_output.trim());

        let env = if copy_env {
            match self.read_env_from_volume(&new_volume).await {
                Ok(map) => map,
                Err(e) => {
                    warn!(
                        "Failed to read env file from copied volume {}: {}",
                        new_volume, e
                    );
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
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

        // Read instructions and setup if code was copied
        let instructions = if copy_code {
            self.read_file_from_volume(&new_volume, "code/instructions.md")
                .await
                .ok()
        } else {
            None
        };

        let setup = if copy_code {
            self.read_file_from_volume(&new_volume, "code/setup.sh")
                .await
                .ok()
        } else {
            None
        };

        info!(
            "Creating remix container with {} user env and fresh system tokens",
            env.len()
        );

        // Now create and start the container with user env + generated system tokens
        let container_name = self
            .create_container_internal_with_tokens(
                session_name,
                Some(env),
                instructions,
                setup,
                ractor_token,
                principal,
                principal_type,
                Some(task_created_at),
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
            "ractor_read_file_{}",
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
                network_mode: Some("ractor_network".to_string()),
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
        session_name: &str,
        env: std::collections::HashMap<String, String>,
        instructions: Option<String>,
        setup: Option<String>,
    ) -> Result<String> {
        let container_name = self
            .create_container_internal(session_name, Some(env), instructions, setup)
            .await?;
        Ok(container_name)
    }

    pub async fn create_container_with_params_and_tokens(
        &self,
        session_name: &str,
        env: std::collections::HashMap<String, String>,
        instructions: Option<String>,
        setup: Option<String>,
        ractor_token: String,
        principal: String,
        principal_type: String,
        task_created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<String> {
        let container_name = self
            .create_container_internal_with_tokens(
                session_name,
                Some(env),
                instructions,
                setup,
                ractor_token,
                principal,
                principal_type,
                Some(task_created_at),
            )
            .await?;
        Ok(container_name)
    }

    pub async fn create_container(&self, session_name: &str) -> Result<String> {
        let container_name = self
            .create_container_internal(session_name, None, None, None)
            .await?;
        Ok(container_name)
    }

    pub async fn wake_container(&self, session_name: &str) -> Result<String> {
        // Read existing env from the volume
        let volume_name = format!("ractor_session_data_{}", session_name.to_ascii_lowercase());
        info!(
            "Waking container for session {} - reading env from volume {}",
            session_name, volume_name
        );

        let env = match self.read_env_from_volume(&volume_name).await {
            Ok(s) => {
                info!(
                    "Found {} env in volume for session {}",
                    s.len(),
                    session_name
                );
                for key in s.keys() {
                    info!("  - Env key: {}", key);
                }
                Some(s)
            }
            Err(e) => {
                warn!(
                    "Could not read env from volume for session {}: {}",
                    session_name, e
                );
                None
            }
        };

        // Read existing instructions and setup from volume
        let instructions = self
            .read_file_from_volume(&volume_name, "code/instructions.md")
            .await
            .ok();
        let setup = self
            .read_file_from_volume(&volume_name, "code/setup.sh")
            .await
            .ok();

        let container_name = self
            .create_container_internal(session_name, env, instructions, setup)
            .await?;
        Ok(container_name)
    }

    pub async fn wake_container_with_tokens(
        &self,
        session_name: &str,
        ractor_token: String,
        principal: String,
        principal_type: String,
        task_created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<String> {
        // Read existing user env from the volume (but generate fresh system tokens)
        let volume_name = format!("ractor_session_data_{}", session_name.to_ascii_lowercase());
        info!(
            "Waking container for session {} with fresh tokens",
            session_name
        );

        let env = match self.read_env_from_volume(&volume_name).await {
            Ok(s) => {
                info!(
                    "Found {} user env in volume for session {}",
                    s.len(),
                    session_name
                );
                Some(s)
            }
            Err(e) => {
                warn!(
                    "Could not read env from volume for session {}: {}",
                    session_name, e
                );
                None
            }
        };

        // Read existing instructions and setup from volume
        let instructions = self
            .read_file_from_volume(&volume_name, "code/instructions.md")
            .await
            .ok();
        let setup = self
            .read_file_from_volume(&volume_name, "code/setup.sh")
            .await
            .ok();

        let container_name = self
            .create_container_internal_with_tokens(
                session_name,
                env,
                instructions,
                setup,
                ractor_token,
                principal,
                principal_type,
                Some(task_created_at),
            )
            .await?;
        Ok(container_name)
    }

    async fn create_container_internal(
        &self,
        session_name: &str,
        env_map: Option<std::collections::HashMap<String, String>>,
        instructions: Option<String>,
        setup: Option<String>,
    ) -> Result<String> {
        let container_name = format!("ractor_session_{}", session_name.to_ascii_lowercase());

        // No content port mapping; preview server is removed.

        // Use session image directly for all sessions
        let container_image = self.session_image.clone();
        info!(
            "Creating container {} with session image {},",
            container_name, container_image
        );

        info!("Creating container for session {}", session_name);

        // Create or get existing session volume
        let session_volume = if self.session_volume_exists(session_name).await? {
            self.get_session_volume_name(session_name)
        } else {
            self.create_session_volume(session_name).await?
        };

        let mut labels = HashMap::new();
        labels.insert("ractor.session".to_string(), session_name.to_string());
        labels.insert("ractor.managed".to_string(), "true".to_string());
        labels.insert("ractor.volume".to_string(), session_volume.clone());

        // Get user token from env (added automatically by session manager)
        let user_token = env_map
            .as_ref()
            .and_then(|s| s.get("RACTOR_TOKEN"))
            .cloned()
            .unwrap_or_else(|| {
                warn!("No RACTOR_TOKEN found in env, Host authentication may fail");
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
            format!("RACTOR_API_URL=http://ractor_api:9000"),
            format!("RACTOR_SESSION_NAME={}", session_name),
            format!("RACTOR_SESSION_DIR=/session"),
        ];

        // Propagate host branding and URL to sessions (provided by start script)
        let host_name = std::env::var("RACTOR_HOST_NAME").unwrap_or_else(|_| "Ractor".to_string());
        let host_url = std::env::var("RACTOR_HOST_URL")
            .expect("RACTOR_HOST_URL must be set by the start script");
        env_vars.push(format!("RACTOR_HOST_NAME={}", host_name));
        env_vars.push(format!("RACTOR_HOST_URL={}", host_url));

        // Configure Ollama host for model inference (required; no default)
        let ollama_host = std::env::var("OLLAMA_HOST").map_err(|_| {
            anyhow::anyhow!("Controller requires OLLAMA_HOST to be set (e.g., http://ollama:11434)")
        })?;
        env_vars.push(format!("OLLAMA_HOST={}", ollama_host));
        let ollama_model =
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gpt-oss:20b".to_string());
        env_vars.push(format!("OLLAMA_MODEL={}", ollama_model));
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
            env_vars.push("RACTOR_HAS_SETUP=true".to_string());
        }

        // Add principal information as environment variables
        if let Some(env_map) = &env_map {
            // Extract principal info from RACTOR_TOKEN if available
            if let Some(_token) = env_map.get("RACTOR_TOKEN") {
                // Set environment variables for Host principal logging
                env_vars.push(format!(
                    "RACTOR_PRINCIPAL={}",
                    env_map
                        .get("RACTOR_PRINCIPAL")
                        .unwrap_or(&"unknown".to_string())
                ));
                env_vars.push(format!(
                    "RACTOR_PRINCIPAL_TYPE={}",
                    env_map
                        .get("RACTOR_PRINCIPAL_TYPE")
                        .unwrap_or(&"unknown".to_string())
                ));
            }

            // Add user env as environment variables, but do NOT override
            // system-managed values like RACTOR_TOKEN or OLLAMA_HOST.
            for (key, value) in env_map {
                if key == "RACTOR_TOKEN" || key == "OLLAMA_HOST" {
                    info!(
                        "Skipping user-provided {} - using system-managed value instead for session {}",
                        key, session_name
                    );
                    continue;
                }
                env_vars.push(format!("{}={}", key, value));
                if key != "RACTOR_PRINCIPAL" && key != "RACTOR_PRINCIPAL_TYPE" {
                    info!(
                        "Adding env entry {} as environment variable for session {}",
                        key, session_name
                    );
                }
            }
        }

        // Set the command with required arguments
        let cmd = vec![
            "ractor-session".to_string(),
            "--api-url".to_string(),
            "http://ractor_api:9000".to_string(),
            "--session-name".to_string(),
            session_name.to_string(),
        ];

        let config = Config {
            image: Some(container_image),
            hostname: Some(format!(
                "session-{}",
                &session_name[..session_name.len().min(8)]
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
                network_mode: Some("ractor_network".to_string()),
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
                session_name,
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
        session_name: &str,
        env_map_opt: Option<std::collections::HashMap<String, String>>,
        instructions: Option<String>,
        setup: Option<String>,
        ractor_token: String,
        principal: String,
        principal_type: String,
        task_created_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<String> {
        let container_name = format!("ractor_session_{}", session_name.to_ascii_lowercase());

        // No content port mapping; preview server is removed.

        // Use session image directly for all sessions
        let container_image = self.session_image.clone();
        info!(
            "Creating container {} with session image and generated tokens",
            container_name
        );

        info!(
            "Creating container for session {} with fresh tokens",
            session_name
        );

        // Create or get existing session volume
        let session_volume = if self.session_volume_exists(session_name).await? {
            self.get_session_volume_name(session_name)
        } else {
            self.create_session_volume(session_name).await?
        };

        let mut labels = HashMap::new();
        labels.insert("ractor.session".to_string(), session_name.to_string());
        labels.insert("ractor.managed".to_string(), "true".to_string());
        labels.insert("ractor.volume".to_string(), session_volume.clone());

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
            format!("RACTOR_API_URL=http://ractor_api:9000"),
            format!("RACTOR_SESSION_NAME={}", session_name),
            format!("RACTOR_SESSION_DIR=/session"),
            // Set the generated system tokens directly as environment variables
            format!("RACTOR_TOKEN={}", ractor_token),
            format!("RACTOR_PRINCIPAL={}", principal),
            format!("RACTOR_PRINCIPAL_TYPE={}", principal_type),
        ];

        // Propagate host branding and URL to sessions (provided by start script)
        let host_name = std::env::var("RACTOR_HOST_NAME").unwrap_or_else(|_| "Ractor".to_string());
        let host_url = std::env::var("RACTOR_HOST_URL")
            .expect("RACTOR_HOST_URL must be set by the start script");
        env_vars.push(format!("RACTOR_HOST_NAME={}", host_name));
        env_vars.push(format!("RACTOR_HOST_URL={}", host_url));

        // Configure Ollama host for model inference (required; no default)
        let ollama_host = std::env::var("OLLAMA_HOST").map_err(|_| {
            anyhow::anyhow!("Controller requires OLLAMA_HOST to be set (e.g., http://ollama:11434)")
        })?;
        env_vars.push(format!("OLLAMA_HOST={}", ollama_host));
        let ollama_model =
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gpt-oss:20b".to_string());
        env_vars.push(format!("OLLAMA_MODEL={}", ollama_model));

        // No web_search tool; do not propagate BRAVE_API_KEY

        // Add hint about setup script availability to avoid unnecessary waiting
        if setup.is_some() {
            env_vars.push("RACTOR_HAS_SETUP=true".to_string());
        }

        // Add task creation timestamp for message processing
        if let Some(timestamp) = task_created_at {
            env_vars.push(format!("RACTOR_TASK_CREATED_AT={}", timestamp.to_rfc3339()));
        }

        info!("Set RACTOR_TOKEN and OLLAMA_HOST as environment variables");

        // Add user env as environment variables (but NOT RACTOR_TOKEN or OLLAMA_HOST)
        if let Some(env_map) = &env_map_opt {
            for (key, value) in env_map {
                // Skip if user provided their own RACTOR_TOKEN or OLLAMA_HOST - we use system-managed values
                if key == "RACTOR_TOKEN" || key == "OLLAMA_HOST" {
                    info!(
                        "Skipping user-provided {} - using system-managed value instead",
                        key
                    );
                    continue;
                }
                env_vars.push(format!("{}={}", key, value));
                info!(
                    "Adding user env entry {} as environment variable for session {}",
                    key, session_name
                );
            }
        }

        // Set the command with required arguments
        let cmd = vec![
            "ractor-session".to_string(),
            "--api-url".to_string(),
            "http://ractor_api:9000".to_string(),
            "--session-name".to_string(),
            session_name.to_string(),
        ];

        let config = Config {
            image: Some(container_image),
            hostname: Some(format!(
                "session-{}",
                &session_name[..session_name.len().min(8)]
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
                network_mode: Some("ractor_network".to_string()),
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
                session_name,
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

    // Sleep container but retain persistent volume (for session pause/sleep)
    pub async fn sleep_container(&self, session_name: &str) -> Result<()> {
        let container_name = format!("ractor_session_{}", session_name.to_ascii_lowercase());

        info!("Sleeping container {}", container_name);

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
                    "Container {} slept, persistent volume retained",
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
                    error!("Failed to sleep container {}: {}", container_name, e);
                    Err(anyhow::anyhow!("Failed to sleep container: {}", e))
                }
            }
        }
    }

    // Delete container and remove persistent volume (for session deletion)
    pub async fn delete_container(&self, session_name: &str) -> Result<()> {
        let container_name = format!("ractor_session_{}", session_name.to_ascii_lowercase());

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
                if let Err(e) = self.cleanup_session_volume(session_name).await {
                    warn!(
                        "Failed to cleanup session volume for {}: {}",
                        session_name, e
                    );
                }

                Ok(())
            }
            Err(e) => {
                if e.to_string().contains("404") || e.to_string().contains("No such container") {
                    warn!("Container {} already removed or doesn't exist, proceeding with volume cleanup", container_name);

                    // Still try to cleanup the session volume
                    if let Err(e) = self.cleanup_session_volume(session_name).await {
                        warn!(
                            "Failed to cleanup session volume for {}: {}",
                            session_name, e
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

    // Removed legacy destroy_container (deprecated). Use close_container or delete_container.

    pub async fn execute_command(&self, session_name: &str, command: &str) -> Result<String> {
        let container_name = format!("ractor_session_{}", session_name.to_ascii_lowercase());

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
        session_name: &str,
        cmd: Vec<String>,
    ) -> Result<(i32, Vec<u8>, Vec<u8>)> {
        let container_name = format!("ractor_session_{}", session_name.to_ascii_lowercase());
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

    pub async fn publish_content(&self, session_name: &str) -> Result<()> {
        let container_name = format!("ractor_session_{}", session_name.to_ascii_lowercase());
        let public_path = format!("/content/{}", session_name);

        info!(
            "Publishing content for session {} to {}",
            session_name, public_path
        );

        // Ensure directory exists inside content container
        let mkdir_output = std::process::Command::new("docker")
            .args(&["exec", "ractor_content", "mkdir", "-p", &public_path])
            .output();
        if let Err(e) = mkdir_output {
            return Err(anyhow::anyhow!("Failed to create content directory: {}", e));
        }

        // Copy content files directly into content container
        let copy_cmd = [
            "docker",
            "cp",
            &format!("{}:/session/content/.", container_name),
            &format!("ractor_content:{}/", public_path),
        ];
        match std::process::Command::new(copy_cmd[0])
            .args(&copy_cmd[1..])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    info!(
                        "Content published successfully for session {}",
                        session_name
                    );
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if stderr.contains("No such file or directory") {
                        info!(
                            "No content files found for session {}, creating empty directory",
                            session_name
                        );
                        Ok(())
                    } else {
                        error!(
                            "Failed to copy content files for session {}: {}",
                            session_name, stderr
                        );
                        Err(anyhow::anyhow!("Failed to copy content files: {}", stderr))
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to execute docker cp command for session {}: {}",
                    session_name, e
                );
                Err(anyhow::anyhow!("Failed to execute docker cp: {}", e))
            }
        }
    }

    pub async fn unpublish_content(&self, session_name: &str) -> Result<()> {
        let public_path = format!("/content/{}", session_name);

        info!(
            "Unpublishing content for session {} from content container: {}",
            session_name, public_path
        );

        // Remove public directory from server container using docker exec
        let output = std::process::Command::new("docker")
            .args(&["exec", "ractor_content", "rm", "-rf", &public_path])
            .output()
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to execute docker exec rm command for session {}: {}",
                    session_name,
                    e
                )
            })?;

        if output.status.success() {
            info!(
                "Content unpublished successfully for session {}",
                session_name
            );
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            error!(
                "Failed to remove public directory for session {}: stdout: {}, stderr: {}",
                session_name, stdout, stderr
            );
            Err(anyhow::anyhow!(
                "Failed to remove public directory for session {}: stdout: {}, stderr: {}",
                session_name,
                stdout,
                stderr
            ))
        }
    }

    /// Check if an session container exists and is running healthily
    pub async fn is_container_healthy(&self, session_name: &str) -> Result<bool> {
        let container_name = format!("ractor_session_{}", session_name.to_ascii_lowercase());

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
                                session_name
                            );
                            return Ok(false);
                        }
                    }
                }
                // Container state is unclear, assume unhealthy
                warn!("Session {} container state is unclear", session_name);
                Ok(false)
            }
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            }) => {
                // Container doesn't exist
                info!("Session {} container does not exist", session_name);
                Ok(false)
            }
            Err(e) => {
                // Other Docker API error
                error!(
                    "Failed to inspect session {} container: {}",
                    session_name, e
                );
                Err(anyhow::anyhow!(
                    "Docker API error for session {}: {}",
                    session_name,
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
