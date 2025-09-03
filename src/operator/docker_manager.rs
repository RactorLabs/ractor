use anyhow::Result;
use bollard::{
    container::{Config, CreateContainerOptions, LogsOptions, RemoveContainerOptions},
    exec::{CreateExecOptions, StartExecResults},
    models::{HostConfig, Mount, MountTypeEnum, PortBinding},
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
    async fn create_session_volume(&self, session_name: &str) -> Result<String> {
        let volume_name = format!("raworc_session_data_{}", session_name);

        let mut labels = HashMap::new();
        labels.insert("raworc.session_name".to_string(), session_name.to_string());
        labels.insert("raworc.type".to_string(), "host_session_volume".to_string());
        labels.insert(
            "raworc.created_at".to_string(),
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

    // Get session volume name (derived from session name)
    fn get_session_volume_name(&self, session_name: &str) -> String {
        format!("raworc_session_data_{}", session_name)
    }

    // Get session container name (derived from session name)
    fn get_session_container_name(&self, session_name: &str) -> String {
        format!("raworc_session_{}", session_name)
    }

    // Check if session volume exists
    async fn session_volume_exists(&self, session_name: &str) -> Result<bool> {
        let volume_name = self.get_session_volume_name(session_name);
        match self.docker.inspect_volume(&volume_name).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    // Initialize session directory structure with secrets, instructions, and setup
    async fn initialize_session_structure(
        &self,
        session_name: &str,
        secrets: &HashMap<String, String>,
        instructions: Option<&str>,
        setup: Option<&str>,
    ) -> Result<()> {
        info!(
            "Initializing session structure for session {}",
            session_name
        );

        // Create base directories (no data folder in v0.4.0)
        let init_script = "mkdir -p /session/{code,secrets,logs,content}
chmod -R 755 /session
echo 'Session directories created (code, secrets, logs, content)'
";

        self.execute_command(session_name, init_script).await?;

        // Write secrets to /session/secrets/ folder
        for (key, value) in secrets {
            let write_secret_command = format!("echo '{}' > /session/secrets/{}", value, key);
            self.execute_command(session_name, &write_secret_command)
                .await?;
        }

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
            "Session structure initialized with {} secrets",
            secrets.len()
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
        let expected_volume_name = format!("raworc_session_data_{}", session_name);

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
        let parent_volume = format!("raworc_session_data_{}", parent_session_name);
        let new_volume = format!("raworc_session_data_{}", session_name);

        info!(
            "Copying volume data from {} to {}",
            parent_volume, new_volume
        );

        // Use bollard Docker API to create copy container
        let copy_container_name = format!("raworc_volume_copy_{}", session_name);

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
        copy_secrets: bool,
        copy_content: bool,
    ) -> Result<String> {
        info!("Creating remix session {} with selective copy from {} (data: {}, code: {}, secrets: {}, content: {})", 
              session_name, parent_session_name, copy_data, copy_code, copy_secrets, copy_content);

        // First create the session volume (without starting container)
        let session_volume = self.create_session_volume(session_name).await?;

        // Then copy specific directories from parent volume to new volume
        let parent_volume = format!("raworc_session_data_{}", parent_session_name);
        let new_volume = format!("raworc_session_data_{}", session_name);

        info!(
            "Copying selective data from {} to {}",
            parent_volume, new_volume
        );

        // Build copy commands based on what should be copied
        let mut copy_commands = Vec::new();

        // Always create base directory structure with proper ownership
        copy_commands.push("sudo mkdir -p /dest/code /dest/data /dest/secrets /dest/content && sudo chown -R host:host /dest".to_string());

        if copy_data {
            copy_commands.push("if [ -d /source/data ]; then cp -a /source/data/. /dest/data/ || echo 'No data to copy'; fi".to_string());
        }

        if copy_code {
            copy_commands.push("if [ -d /source/code ]; then cp -a /source/code/. /dest/code/ || echo 'No code to copy'; fi".to_string());
        }

        if copy_secrets {
            copy_commands.push("if [ -d /source/secrets ]; then cp -a /source/secrets/. /dest/secrets/ && echo 'SECRETS_COPIED:' && find /source/secrets -type f -exec bash -c 'echo \"SECRET:$(basename {})=$(cat {})\"' \\; || echo 'No secrets to copy'; fi".to_string());
        } else {
            copy_commands.push("echo 'Skipping secrets copy as requested'".to_string());
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
        let copy_container_name = format!("raworc_volume_copy_{}", session_name);

        let config = Config {
            image: Some(self.host_image.clone()),
            user: Some("host".to_string()),
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
                network_mode: Some("raworc_network".to_string()),
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

        info!(
            "Successfully parsed {} secrets from copy output",
            secrets.len()
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
            "Creating container with {} secrets from copied volume",
            secrets.len()
        );

        // Now create and start the container with the copied secrets as environment variables
        let container_name = self
            .create_container_internal(session_name, Some(secrets), instructions, setup)
            .await?;

        Ok(container_name)
    }

    pub async fn create_container_with_selective_copy_and_tokens(
        &self,
        session_name: &str,
        parent_session_name: &str,
        copy_data: bool,
        copy_code: bool,
        copy_secrets: bool,
        copy_content: bool,
        api_key: String,
        raworc_token: String,
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
        let parent_volume = format!("raworc_session_data_{}", parent_session_name);
        let new_volume = format!("raworc_session_data_{}", session_name);

        info!(
            "Copying selective data from {} to {}",
            parent_volume, new_volume
        );

        // Build copy commands based on what should be copied
        let mut copy_commands = Vec::new();

        // Always create base directory structure with proper ownership
        copy_commands.push("sudo mkdir -p /dest/code /dest/data /dest/secrets /dest/content && sudo chown -R host:host /dest".to_string());

        if copy_data {
            copy_commands.push("if [ -d /source/data ]; then cp -a /source/data/. /dest/data/ || echo 'No data to copy'; fi".to_string());
        }

        if copy_code {
            copy_commands.push("if [ -d /source/code ]; then cp -a /source/code/. /dest/code/ || echo 'No code to copy'; fi".to_string());
        }

        if copy_secrets {
            copy_commands.push("if [ -d /source/secrets ]; then cp -a /source/secrets/. /dest/secrets/ && echo 'SECRETS_COPIED:' && find /source/secrets -type f -exec bash -c 'echo \"SECRET:$(basename {})=$(cat {})\"' \\; || echo 'No secrets to copy'; fi".to_string());
        } else {
            copy_commands.push("echo 'Skipping secrets copy as requested'".to_string());
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
        let copy_container_name = format!("raworc_volume_copy_{}", session_name);

        let config = Config {
            image: Some(self.host_image.clone()),
            user: Some("host".to_string()),
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
                network_mode: Some("raworc_network".to_string()),
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

        // Parse secrets from copy output (user secrets only - system tokens are generated separately)
        let mut secrets = std::collections::HashMap::new();

        // Parse SECRET:key=value lines from the copy output
        for line in log_output.lines() {
            if line.starts_with("SECRET:") {
                if let Some(secret_part) = line.strip_prefix("SECRET:") {
                    if let Some((key, value)) = secret_part.split_once('=') {
                        secrets.insert(key.to_string(), value.to_string());
                        info!("Parsed user secret from copy output: {}", key);
                    }
                }
            }
        }

        info!(
            "Successfully parsed {} user secrets from copy output",
            secrets.len()
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
            "Creating remix container with {} user secrets and fresh system tokens",
            secrets.len()
        );

        // Now create and start the container with user secrets + generated system tokens
        let container_name = self
            .create_container_internal_with_tokens(
                session_name,
                Some(secrets),
                instructions,
                setup,
                api_key,
                raworc_token,
                principal,
                principal_type,
                Some(task_created_at),
            )
            .await?;

        Ok(container_name)
    }

    // Helper method to read secrets from a volume
    async fn read_secrets_from_volume(
        &self,
        volume_name: &str,
    ) -> Result<std::collections::HashMap<String, String>> {
        let mut secrets = std::collections::HashMap::new();

        // Create a temporary container to read secrets from the volume
        let read_container_name = format!(
            "raworc_read_secrets_{}",
            Uuid::new_v4().to_string()[..8].to_string()
        );

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

        // Get output (list of secret file names)
        let logs = self.docker.logs::<String>(
            &read_container_name,
            Some(LogsOptions {
                stdout: true,
                stderr: false,
                ..Default::default()
            }),
        );

        let secret_files = logs
            .map(|log| match log {
                Ok(line) => String::from_utf8_lossy(&line.into_bytes())
                    .trim()
                    .to_string(),
                Err(_) => String::new(),
            })
            .collect::<Vec<_>>()
            .await
            .join("\n")
            .lines()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        info!(
            "Found {} secret files in volume {}: {:?}",
            secret_files.len(),
            volume_name,
            secret_files
        );

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

        // Now read each secret file content
        for secret_file in secret_files {
            if !secret_file.is_empty() {
                if let Ok(value) = self
                    .read_file_from_volume(volume_name, &format!("secrets/{}", secret_file))
                    .await
                {
                    secrets.insert(secret_file, value);
                }
            }
        }

        Ok(secrets)
    }

    // Helper method to read a file from a volume
    async fn read_file_from_volume(&self, volume_name: &str, file_path: &str) -> Result<String> {
        let read_container_name = format!(
            "raworc_read_file_{}",
            Uuid::new_v4().to_string()[..8].to_string()
        );

        let config = Config {
            image: Some(self.host_image.clone()),
            user: Some("host".to_string()),
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
                network_mode: Some("raworc_network".to_string()),
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
        secrets: std::collections::HashMap<String, String>,
        instructions: Option<String>,
        setup: Option<String>,
    ) -> Result<String> {
        let container_name = self
            .create_container_internal(session_name, Some(secrets), instructions, setup)
            .await?;
        Ok(container_name)
    }

    pub async fn create_container_with_params_and_tokens(
        &self,
        session_name: &str,
        secrets: std::collections::HashMap<String, String>,
        instructions: Option<String>,
        setup: Option<String>,
        api_key: String,
        raworc_token: String,
        principal: String,
        principal_type: String,
        task_created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<String> {
        let container_name = self
            .create_container_internal_with_tokens(
                session_name,
                Some(secrets),
                instructions,
                setup,
                api_key,
                raworc_token,
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

    pub async fn restore_container(&self, session_name: &str) -> Result<String> {
        // Read existing secrets from the volume
        let volume_name = format!("raworc_session_data_{}", session_name);
        info!(
            "Restoring container for session {} - reading secrets from volume {}",
            session_name, volume_name
        );

        let secrets = match self.read_secrets_from_volume(&volume_name).await {
            Ok(s) => {
                info!(
                    "Found {} secrets in volume for session {}",
                    s.len(),
                    session_name
                );
                for key in s.keys() {
                    info!("  - Secret: {}", key);
                }
                Some(s)
            }
            Err(e) => {
                warn!(
                    "Could not read secrets from volume for session {}: {}",
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
            .create_container_internal(session_name, secrets, instructions, setup)
            .await?;
        Ok(container_name)
    }

    pub async fn restore_container_with_tokens(
        &self,
        session_name: &str,
        api_key: String,
        raworc_token: String,
        principal: String,
        principal_type: String,
        task_created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<String> {
        // Read existing user secrets from the volume (but generate fresh system tokens)
        let volume_name = format!("raworc_session_data_{}", session_name);
        info!(
            "Restoring container for session {} with fresh tokens",
            session_name
        );

        let secrets = match self.read_secrets_from_volume(&volume_name).await {
            Ok(s) => {
                info!(
                    "Found {} user secrets in volume for session {}",
                    s.len(),
                    session_name
                );
                Some(s)
            }
            Err(e) => {
                warn!(
                    "Could not read secrets from volume for session {}: {}",
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
                secrets,
                instructions,
                setup,
                api_key,
                raworc_token,
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
        secrets: Option<std::collections::HashMap<String, String>>,
        instructions: Option<String>,
        setup: Option<String>,
    ) -> Result<String> {
        let container_name = format!("raworc_session_{session_name}");

        // Get content port from session (already allocated during session creation)
        let content_port: i32 = sqlx::query_scalar::<_, Option<i32>>(
            "SELECT content_port FROM sessions WHERE name = ?",
        )
        .bind(session_name)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get content port from session: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("Session has no content port assigned"))?;

        info!(
            "Using Content port {} from session {}",
            content_port, session_name
        );

        // Use host image directly for all sessions
        let container_image = self.host_image.clone();
        info!(
            "Creating container {} with host image {}",
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
        labels.insert("raworc.session".to_string(), session_name.to_string());
        labels.insert("raworc.managed".to_string(), "true".to_string());
        labels.insert("raworc.volume".to_string(), session_volume.clone());

        // Get user token from secrets (added automatically by session manager)
        let user_token = secrets
            .as_ref()
            .and_then(|s| s.get("RAWORC_TOKEN"))
            .cloned()
            .unwrap_or_else(|| {
                warn!("No RAWORC_TOKEN found in secrets, Host authentication may fail");
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

        // Configure port mapping for Content HTTP server (8000 inside -> random port outside)
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            "8000/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some(content_port.to_string()),
            }]),
        );

        let mut exposed_ports = HashMap::new();
        exposed_ports.insert("8000/tcp".to_string(), HashMap::new());

        // Set environment variables for the session structure
        let mut env = vec![
            format!("RAWORC_API_URL=http://raworc_server:9000"),
            format!("RAWORC_SESSION_NAME={}", session_name),
            format!("RAWORC_SESSION_DIR=/session"),
        ];

        // Add hint about setup script availability to avoid unnecessary waiting
        if setup.is_some() {
            env.push("RAWORC_HAS_SETUP=true".to_string());
        }

        // Add principal information as environment variables
        if let Some(secrets_map) = &secrets {
            // Extract principal info from RAWORC_TOKEN if available
            if let Some(token) = secrets_map.get("RAWORC_TOKEN") {
                // Set environment variables for Host principal logging
                env.push(format!(
                    "RAWORC_PRINCIPAL={}",
                    secrets_map
                        .get("RAWORC_PRINCIPAL")
                        .unwrap_or(&"unknown".to_string())
                ));
                env.push(format!(
                    "RAWORC_PRINCIPAL_TYPE={}",
                    secrets_map
                        .get("RAWORC_PRINCIPAL_TYPE")
                        .unwrap_or(&"unknown".to_string())
                ));
            }

            for (key, value) in secrets_map {
                env.push(format!("{}={}", key, value));
                if key != "RAWORC_TOKEN"
                    && key != "RAWORC_PRINCIPAL"
                    && key != "RAWORC_PRINCIPAL_TYPE"
                {
                    info!(
                        "Adding secret {} as environment variable for session {}",
                        key, session_name
                    );
                }
            }
        }

        // Set the command with required arguments
        let cmd = vec![
            "raworc-host".to_string(),
            "--api-url".to_string(),
            "http://raworc_server:9000".to_string(),
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
            env: Some(env),
            cmd: Some(cmd),
            working_dir: Some("/session".to_string()), // User starts in their session
            exposed_ports: Some(exposed_ports),
            host_config: Some(bollard::models::HostConfig {
                cpu_quota: Some((self.cpu_limit * 100000.0) as i64),
                cpu_period: Some(100000),
                memory: Some(self.memory_limit),
                memory_swap: Some(self.memory_limit),
                network_mode: Some("raworc_network".to_string()),
                mounts: Some(mounts),
                port_bindings: Some(port_bindings),
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
            let empty_secrets = HashMap::new();
            let secrets_ref = secrets.as_ref().unwrap_or(&empty_secrets);
            self.initialize_session_structure(
                session_name,
                secrets_ref,
                instructions.as_deref(),
                setup.as_deref(),
            )
            .await?;
        }

        info!(
            "Container {} created with session volume {} using Content port {}",
            container_name, session_volume, content_port
        );
        Ok(container.id)
    }

    async fn create_container_internal_with_tokens(
        &self,
        session_name: &str,
        secrets: Option<std::collections::HashMap<String, String>>,
        instructions: Option<String>,
        setup: Option<String>,
        api_key: String,
        raworc_token: String,
        principal: String,
        principal_type: String,
        task_created_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<String> {
        let container_name = format!("raworc_session_{session_name}");

        // Get content port from session (already allocated during session creation)
        let content_port: i32 = sqlx::query_scalar::<_, Option<i32>>(
            "SELECT content_port FROM sessions WHERE name = ?",
        )
        .bind(session_name)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get content port from session: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("Session has no content port assigned"))?;

        info!(
            "Using Content port {} from session {}",
            content_port, session_name
        );

        // Use host image directly for all sessions
        let container_image = self.host_image.clone();
        info!(
            "Creating container {} with host image and generated tokens",
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
        labels.insert("raworc.session".to_string(), session_name.to_string());
        labels.insert("raworc.managed".to_string(), "true".to_string());
        labels.insert("raworc.volume".to_string(), session_volume.clone());

        // Configure volume mounts
        let mounts = vec![bollard::models::Mount {
            typ: Some(bollard::models::MountTypeEnum::VOLUME),
            source: Some(session_volume.clone()),
            target: Some("/session".to_string()),
            read_only: Some(false),
            ..Default::default()
        }];

        // Configure port mapping for Content HTTP server (8000 inside -> random port outside)
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            "8000/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some(content_port.to_string()),
            }]),
        );

        let mut exposed_ports = HashMap::new();
        exposed_ports.insert("8000/tcp".to_string(), HashMap::new());

        // Set environment variables for the session structure
        let mut env = vec![
            format!("RAWORC_API_URL=http://raworc_server:9000"),
            format!("RAWORC_SESSION_NAME={}", session_name),
            format!("RAWORC_SESSION_DIR=/session"),
            // Set the generated system tokens directly as environment variables
            format!("ANTHROPIC_API_KEY={}", api_key),
            format!("RAWORC_TOKEN={}", raworc_token),
            format!("RAWORC_PRINCIPAL={}", principal),
            format!("RAWORC_PRINCIPAL_TYPE={}", principal_type),
        ];

        // Add hint about setup script availability to avoid unnecessary waiting
        if setup.is_some() {
            env.push("RAWORC_HAS_SETUP=true".to_string());
        }

        // Add task creation timestamp for message processing
        if let Some(timestamp) = task_created_at {
            env.push(format!("RAWORC_TASK_CREATED_AT={}", timestamp.to_rfc3339()));
        }

        info!("Set system-generated ANTHROPIC_API_KEY and RAWORC_TOKEN as environment variables");

        // Add user secrets as environment variables (but NOT ANTHROPIC_API_KEY or RAWORC_TOKEN)
        if let Some(secrets_map) = &secrets {
            for (key, value) in secrets_map {
                // Skip if user provided their own ANTHROPIC_API_KEY or RAWORC_TOKEN - we use system ones
                if key == "ANTHROPIC_API_KEY" || key == "RAWORC_TOKEN" {
                    info!(
                        "Skipping user-provided {} - using system-generated token instead",
                        key
                    );
                    continue;
                }
                env.push(format!("{}={}", key, value));
                info!(
                    "Adding user secret {} as environment variable for session {}",
                    key, session_name
                );
            }
        }

        // Set the command with required arguments
        let cmd = vec![
            "raworc-host".to_string(),
            "--api-url".to_string(),
            "http://raworc_server:9000".to_string(),
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
            env: Some(env),
            cmd: Some(cmd),
            working_dir: Some("/session".to_string()), // User starts in their session
            exposed_ports: Some(exposed_ports),
            host_config: Some(bollard::models::HostConfig {
                cpu_quota: Some((self.cpu_limit * 100000.0) as i64),
                cpu_period: Some(100000),
                memory: Some(self.memory_limit),
                memory_swap: Some(self.memory_limit),
                network_mode: Some("raworc_network".to_string()),
                mounts: Some(mounts),
                port_bindings: Some(port_bindings),
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
        // Only initialize with user secrets (system tokens are already in environment)
        if !self.session_initialized(&session_volume).await? {
            let empty_secrets = HashMap::new();
            let secrets_ref = secrets.as_ref().unwrap_or(&empty_secrets);
            self.initialize_session_structure(
                session_name,
                secrets_ref,
                instructions.as_deref(),
                setup.as_deref(),
            )
            .await?;
        }

        info!("Container {} created with session volume {} using Content port {}, and fresh system tokens", container_name, session_volume, content_port);
        Ok(container.id)
    }

    // Close container but retain persistent volume (for session pause/close)
    pub async fn close_container(&self, session_name: &str) -> Result<()> {
        let container_name = format!("raworc_session_{session_name}");

        info!("Closing container {}", container_name);

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
                    "Container {} closed, persistent volume retained",
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
                    error!("Failed to close container {}: {}", container_name, e);
                    Err(anyhow::anyhow!("Failed to close container: {}", e))
                }
            }
        }
    }

    // Delete container and remove persistent volume (for session deletion)
    pub async fn delete_container(&self, session_name: &str) -> Result<()> {
        let container_name = format!("raworc_session_{session_name}");

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

    // Legacy method - kept for backward compatibility, but deprecated
    // Use close_container or delete_container instead
    pub async fn destroy_container(&self, session_name: &str) -> Result<()> {
        warn!("destroy_container is deprecated, use close_container or delete_container instead");
        self.delete_container(session_name).await
    }

    pub async fn execute_command(&self, session_name: &str, command: &str) -> Result<String> {
        let container_name = format!("raworc_session_{session_name}");

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

    pub async fn publish_content(&self, session_name: &str) -> Result<()> {
        let container_name = format!("raworc_session_{}", session_name);
        let public_path = format!("/public/{}", session_name);

        info!(
            "Publishing content for session {} to {}",
            session_name, public_path
        );

        // Create public directory
        let create_dir_cmd = format!("mkdir -p {}", public_path);
        if let Err(e) = std::process::Command::new("bash")
            .arg("-c")
            .arg(&create_dir_cmd)
            .output()
        {
            error!("Failed to create public directory {}: {}", public_path, e);
            return Err(anyhow::anyhow!("Failed to create public directory: {}", e));
        }

        // Copy content files from container to public directory
        let copy_cmd = format!(
            "docker cp {}:/session/content/. {}/",
            container_name, public_path
        );
        match std::process::Command::new("bash")
            .arg("-c")
            .arg(&copy_cmd)
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
                        info!("No content files found for session {}, creating empty public directory", session_name);
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
        let public_path = format!("/public/{}", session_name);

        info!(
            "Unpublishing content for session {} from {}",
            session_name, public_path
        );

        // Remove public directory
        let remove_cmd = format!("rm -rf {}", public_path);
        match std::process::Command::new("bash")
            .arg("-c")
            .arg(&remove_cmd)
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    info!(
                        "Content unpublished successfully for session {}",
                        session_name
                    );
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    error!(
                        "Failed to remove public directory for session {}: {}",
                        session_name, stderr
                    );
                    Err(anyhow::anyhow!(
                        "Failed to remove public directory: {}",
                        stderr
                    ))
                }
            }
            Err(e) => {
                error!(
                    "Failed to execute rm command for session {}: {}",
                    session_name, e
                );
                Err(anyhow::anyhow!("Failed to execute rm: {}", e))
            }
        }
    }
}
