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
    agent_image: String,
    cpu_limit: f64,
    memory_limit: i64,
    db_pool: MySqlPool,
}

impl DockerManager {
    pub fn new(docker: Docker, db_pool: MySqlPool) -> Self {
        Self {
            docker,
            db_pool,
            agent_image: std::env::var("AGENT_IMAGE")
                .unwrap_or_else(|_| "raworc_agent:latest".to_string()),
            cpu_limit: std::env::var("AGENT_CPU_LIMIT")
                .unwrap_or_else(|_| "0.5".to_string())
                .parse()
                .unwrap_or(0.5),
            memory_limit: std::env::var("AGENT_MEMORY_LIMIT")
                .unwrap_or_else(|_| "536870912".to_string())
                .parse()
                .unwrap_or(536870912),
        }
    }

    // NEW: Create agent volume with explicit naming
    async fn create_agent_volume(&self, agent_name: &str) -> Result<String> {
        let volume_name = format!("raworc_agent_data_{}", agent_name.to_ascii_lowercase());

        let mut labels = HashMap::new();
        labels.insert("raworc.agent_name".to_string(), agent_name.to_string());
        labels.insert("raworc.type".to_string(), "agent_volume".to_string());
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
        info!("Created agent volume: {}", volume_name);

        Ok(volume_name)
    }

    // Get agent volume name (derived from agent name). Docker volume names must be lowercase.
    fn get_agent_volume_name(&self, agent_name: &str) -> String {
        format!("raworc_agent_data_{}", agent_name.to_ascii_lowercase())
    }

    // Get agent container name (derived from agent name). Docker container names must be lowercase.
    fn get_agent_container_name(&self, agent_name: &str) -> String {
        format!("raworc_agent_{}", agent_name.to_ascii_lowercase())
    }

    // Check if agent volume exists
    async fn agent_volume_exists(&self, agent_name: &str) -> Result<bool> {
        let volume_name = self.get_agent_volume_name(agent_name);
        match self.docker.inspect_volume(&volume_name).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    // Initialize agent directory structure with secrets, instructions, and setup
    async fn initialize_agent_structure(
        &self,
        agent_name: &str,
        secrets: &HashMap<String, String>,
        instructions: Option<&str>,
        setup: Option<&str>,
    ) -> Result<()> {
        info!("Initializing agent structure for agent {}", agent_name);

        // Create base directories (no data folder in v0.4.0) with proper ownership
        // Use sudo to ensure proper ownership since volume may be root-owned initially
        let init_script = "sudo mkdir -p /agent/code /agent/secrets /agent/logs /agent/content /agent/template
sudo chown -R agent:agent /agent
sudo chmod -R 755 /agent
# Seed default HTML template if missing
if [ ! -f /agent/template/simple.html ] && [ -f /opt/raworc/templates/simple.html ]; then
  sudo cp /opt/raworc/templates/simple.html /agent/template/simple.html && sudo chown agent:agent /agent/template/simple.html;
fi
echo 'Agent directories created (code, secrets, logs, content, template)'
";

        self.execute_command(agent_name, init_script).await?;

        // Write secrets to /agent/secrets/ folder
        for (key, value) in secrets {
            let write_secret_command = format!("echo '{}' > /agent/secrets/{}", value, key);
            self.execute_command(agent_name, &write_secret_command)
                .await?;
        }

        // Write instructions if provided
        if let Some(instructions_content) = instructions {
            let escaped_instructions = instructions_content.replace("'", "'\"'\"'");
            let write_instructions_command = format!(
                "echo '{}' > /agent/code/instructions.md",
                escaped_instructions
            );
            self.execute_command(agent_name, &write_instructions_command)
                .await?;
        }

        // Write and make setup script executable if provided
        if let Some(setup_content) = setup {
            let escaped_setup = setup_content.replace("'", "'\"'\"'");
            let write_setup_command = format!(
                "echo '{}' > /agent/code/setup.sh && chmod +x /agent/code/setup.sh",
                escaped_setup
            );
            self.execute_command(agent_name, &write_setup_command)
                .await?;
        }

        info!("Agent structure initialized with {} secrets", secrets.len());

        Ok(())
    }

    // NEW: Check if agent is initialized
    async fn agent_initialized(&self, _volume_name: &str) -> Result<bool> {
        // Skip initialization check for now - always initialize
        Ok(false)
    }

    // NEW: Cleanup agent volume
    pub async fn cleanup_agent_volume(&self, agent_name: &str) -> Result<()> {
        let expected_volume_name = format!("raworc_agent_data_{}", agent_name.to_ascii_lowercase());

        match self.docker.remove_volume(&expected_volume_name, None).await {
            Ok(_) => {
                info!("Removed agent volume: {}", expected_volume_name);
            }
            Err(e) => warn!(
                "Failed to remove agent volume {}: {}",
                expected_volume_name, e
            ),
        }

        Ok(())
    }

    pub async fn create_container_with_volume_copy(
        &self,
        agent_name: &str,
        parent_agent_name: &str,
    ) -> Result<String> {
        info!(
            "Creating remix agent {} with volume copy from {}",
            agent_name, parent_agent_name
        );

        // First create the container normally (this creates the empty target volume)
        let container_name = self.create_container(agent_name).await?;

        // Then copy data from parent volume to new volume using Docker command
        let parent_volume = format!(
            "raworc_agent_data_{}",
            parent_agent_name.to_ascii_lowercase()
        );
        let new_volume = format!("raworc_agent_data_{}", agent_name.to_ascii_lowercase());

        info!(
            "Copying volume data from {} to {}",
            parent_volume, new_volume
        );

        // Use bollard Docker API to create copy container
        let copy_container_name = format!("raworc_volume_copy_{}", agent_name);

        let config = Config {
            image: Some(self.agent_image.clone()),
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
        agent_name: &str,
        parent_agent_name: &str,
        copy_data: bool,
        copy_code: bool,
        copy_secrets: bool,
        copy_content: bool,
    ) -> Result<String> {
        info!("Creating remix agent {} with selective copy from {} (data: {}, code: {}, secrets: {}, content: {})", 
              agent_name, parent_agent_name, copy_data, copy_code, copy_secrets, copy_content);

        // First create the agent volume (without starting container)
        let agent_volume = self.create_agent_volume(agent_name).await?;

        // Then copy specific directories from parent volume to new volume
        let parent_volume = format!(
            "raworc_agent_data_{}",
            parent_agent_name.to_ascii_lowercase()
        );
        let new_volume = format!("raworc_agent_data_{}", agent_name.to_ascii_lowercase());

        info!(
            "Copying selective data from {} to {}",
            parent_volume, new_volume
        );

        // Build copy commands based on what should be copied
        let mut copy_commands = Vec::new();

        // Always create base directory structure with proper ownership (run as root to create dirs, then chown to agent)
        copy_commands.push("mkdir -p /dest/code /dest/data /dest/secrets /dest/content /dest/logs && chown -R 1000:1000 /dest".to_string());

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
        let copy_container_name = format!("raworc_volume_copy_{}", agent_name);

        let config = Config {
            image: Some(self.agent_image.clone()),
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

        // Parse secrets from copy output (always copied for remix agents)
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
            .create_container_internal(agent_name, Some(secrets), instructions, setup)
            .await?;

        Ok(container_name)
    }

    pub async fn create_container_with_selective_copy_and_tokens(
        &self,
        agent_name: &str,
        parent_agent_name: &str,
        copy_data: bool,
        copy_code: bool,
        copy_secrets: bool,
        copy_content: bool,
        raworc_token: String,
        principal: String,
        principal_type: String,
        task_created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<String> {
        info!(
            "Creating remix agent {} with selective copy from {} and fresh tokens",
            agent_name, parent_agent_name
        );

        // First create the agent volume (without starting container)
        let agent_volume = self.create_agent_volume(agent_name).await?;

        // Then copy specific directories from parent volume to new volume
        let parent_volume = format!(
            "raworc_agent_data_{}",
            parent_agent_name.to_ascii_lowercase()
        );
        let new_volume = format!("raworc_agent_data_{}", agent_name.to_ascii_lowercase());

        info!(
            "Copying selective data from {} to {}",
            parent_volume, new_volume
        );

        // Build copy commands based on what should be copied
        let mut copy_commands = Vec::new();

        // Always create base directory structure with proper ownership (run as root to create dirs, then chown to agent)
        copy_commands.push("mkdir -p /dest/code /dest/data /dest/secrets /dest/content /dest/logs && chown -R 1000:1000 /dest".to_string());

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
        let copy_container_name = format!("raworc_volume_copy_{}", agent_name);

        let config = Config {
            image: Some(self.agent_image.clone()),
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
                agent_name,
                Some(secrets),
                instructions,
                setup,
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
            image: Some(self.agent_image.clone()),
            user: Some("root".to_string()),
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
            image: Some(self.agent_image.clone()),
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
        agent_name: &str,
        secrets: std::collections::HashMap<String, String>,
        instructions: Option<String>,
        setup: Option<String>,
    ) -> Result<String> {
        let container_name = self
            .create_container_internal(agent_name, Some(secrets), instructions, setup)
            .await?;
        Ok(container_name)
    }

    pub async fn create_container_with_params_and_tokens(
        &self,
        agent_name: &str,
        secrets: std::collections::HashMap<String, String>,
        instructions: Option<String>,
        setup: Option<String>,
        raworc_token: String,
        principal: String,
        principal_type: String,
        task_created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<String> {
        let container_name = self
            .create_container_internal_with_tokens(
                agent_name,
                Some(secrets),
                instructions,
                setup,
                raworc_token,
                principal,
                principal_type,
                Some(task_created_at),
            )
            .await?;
        Ok(container_name)
    }

    pub async fn create_container(&self, agent_name: &str) -> Result<String> {
        let container_name = self
            .create_container_internal(agent_name, None, None, None)
            .await?;
        Ok(container_name)
    }

    pub async fn wake_container(&self, agent_name: &str) -> Result<String> {
        // Read existing secrets from the volume
        let volume_name = format!("raworc_agent_data_{}", agent_name.to_ascii_lowercase());
        info!(
            "Waking container for agent {} - reading secrets from volume {}",
            agent_name, volume_name
        );

        let secrets = match self.read_secrets_from_volume(&volume_name).await {
            Ok(s) => {
                info!(
                    "Found {} secrets in volume for agent {}",
                    s.len(),
                    agent_name
                );
                for key in s.keys() {
                    info!("  - Secret: {}", key);
                }
                Some(s)
            }
            Err(e) => {
                warn!(
                    "Could not read secrets from volume for agent {}: {}",
                    agent_name, e
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
            .create_container_internal(agent_name, secrets, instructions, setup)
            .await?;
        Ok(container_name)
    }

    pub async fn wake_container_with_tokens(
        &self,
        agent_name: &str,
        raworc_token: String,
        principal: String,
        principal_type: String,
        task_created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<String> {
        // Read existing user secrets from the volume (but generate fresh system tokens)
        let volume_name = format!("raworc_agent_data_{}", agent_name.to_ascii_lowercase());
        info!(
            "Waking container for agent {} with fresh tokens",
            agent_name
        );

        let secrets = match self.read_secrets_from_volume(&volume_name).await {
            Ok(s) => {
                info!(
                    "Found {} user secrets in volume for agent {}",
                    s.len(),
                    agent_name
                );
                Some(s)
            }
            Err(e) => {
                warn!(
                    "Could not read secrets from volume for agent {}: {}",
                    agent_name, e
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
                agent_name,
                secrets,
                instructions,
                setup,
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
        agent_name: &str,
        secrets: Option<std::collections::HashMap<String, String>>,
        instructions: Option<String>,
        setup: Option<String>,
    ) -> Result<String> {
        let container_name = format!("raworc_agent_{}", agent_name.to_ascii_lowercase());

        // No content port mapping; preview server is removed.

        // Use agent image directly for all agents
        let container_image = self.agent_image.clone();
        info!(
            "Creating container {} with agent image {},",
            container_name, container_image
        );

        info!("Creating container for agent {}", agent_name);

        // Create or get existing agent volume
        let agent_volume = if self.agent_volume_exists(agent_name).await? {
            self.get_agent_volume_name(agent_name)
        } else {
            self.create_agent_volume(agent_name).await?
        };

        let mut labels = HashMap::new();
        labels.insert("raworc.agent".to_string(), agent_name.to_string());
        labels.insert("raworc.managed".to_string(), "true".to_string());
        labels.insert("raworc.volume".to_string(), agent_volume.clone());

        // Get user token from secrets (added automatically by agent manager)
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
            source: Some(agent_volume.clone()),
            target: Some("/agent".to_string()),
            read_only: Some(false),
            ..Default::default()
        }];

        // No port bindings or exposed ports needed.

        // Set environment variables for the agent structure
        let mut env = vec![
            format!("RAWORC_API_URL=http://raworc_api:9000"),
            format!("RAWORC_AGENT_NAME={}", agent_name),
            format!("RAWORC_AGENT_DIR=/agent"),
        ];

        // Propagate host branding and URL to agents (provided by start script)
        let host_name = std::env::var("RAWORC_HOST_NAME").unwrap_or_else(|_| "Raworc".to_string());
        let host_url = std::env::var("RAWORC_HOST_URL")
            .expect("RAWORC_HOST_URL must be set by the start script");
        env.push(format!("RAWORC_HOST_NAME={}", host_name));
        env.push(format!("RAWORC_HOST_URL={}", host_url));

        // Configure Ollama host for model inference (required; no default)
        let ollama_host = std::env::var("OLLAMA_HOST").map_err(|_| {
            anyhow::anyhow!("Controller requires OLLAMA_HOST to be set (e.g., http://ollama:11434)")
        })?;
        env.push(format!("OLLAMA_HOST={}", ollama_host));
        let ollama_model =
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gpt-oss:20b".to_string());
        env.push(format!("OLLAMA_MODEL={}", ollama_model));
        let ollama_timeout =
            std::env::var("OLLAMA_TIMEOUT_SECS").unwrap_or_else(|_| "600".to_string());
        env.push(format!("OLLAMA_TIMEOUT_SECS={}", ollama_timeout));
        // Propagate timeout for model calls; default 600s if unspecified
        let ollama_timeout =
            std::env::var("OLLAMA_TIMEOUT_SECS").unwrap_or_else(|_| "600".to_string());
        env.push(format!("OLLAMA_TIMEOUT_SECS={}", ollama_timeout));

        // No web_search tool; do not propagate BRAVE_API_KEY

        // Add hint about setup script availability to avoid unnecessary waiting
        if setup.is_some() {
            env.push("RAWORC_HAS_SETUP=true".to_string());
        }

        // Add principal information as environment variables
        if let Some(secrets_map) = &secrets {
            // Extract principal info from RAWORC_TOKEN if available
            if let Some(_token) = secrets_map.get("RAWORC_TOKEN") {
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

            // Add user secrets as environment variables, but do NOT override
            // system-managed values like RAWORC_TOKEN or OLLAMA_HOST.
            for (key, value) in secrets_map {
                if key == "RAWORC_TOKEN" || key == "OLLAMA_HOST" {
                    info!(
                        "Skipping user-provided {} - using system-managed value instead for agent {}",
                        key, agent_name
                    );
                    continue;
                }
                env.push(format!("{}={}", key, value));
                if key != "RAWORC_PRINCIPAL" && key != "RAWORC_PRINCIPAL_TYPE" {
                    info!(
                        "Adding secret {} as environment variable for agent {}",
                        key, agent_name
                    );
                }
            }
        }

        // Set the command with required arguments
        let cmd = vec![
            "raworc-agent".to_string(),
            "--api-url".to_string(),
            "http://raworc_api:9000".to_string(),
            "--agent-name".to_string(),
            agent_name.to_string(),
        ];

        let config = Config {
            image: Some(container_image),
            hostname: Some(format!("agent-{}", &agent_name[..agent_name.len().min(8)])),
            labels: Some(labels),
            env: Some(env),
            cmd: Some(cmd),
            working_dir: Some("/agent".to_string()), // User starts in their agent
            exposed_ports: None,
            host_config: Some(bollard::models::HostConfig {
                cpu_quota: Some((self.cpu_limit * 100000.0) as i64),
                cpu_period: Some(100000),
                memory: Some(self.memory_limit),
                memory_swap: Some(self.memory_limit),
                network_mode: Some("raworc_network".to_string()),
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

        // Initialize agent structure after starting container so host can execute setup script
        if !self.agent_initialized(&agent_volume).await? {
            let empty_secrets = HashMap::new();
            let secrets_ref = secrets.as_ref().unwrap_or(&empty_secrets);
            self.initialize_agent_structure(
                agent_name,
                secrets_ref,
                instructions.as_deref(),
                setup.as_deref(),
            )
            .await?;
        }

        info!(
            "Container {} created with agent volume {}",
            container_name, agent_volume
        );
        Ok(container.id)
    }

    async fn create_container_internal_with_tokens(
        &self,
        agent_name: &str,
        secrets: Option<std::collections::HashMap<String, String>>,
        instructions: Option<String>,
        setup: Option<String>,
        raworc_token: String,
        principal: String,
        principal_type: String,
        task_created_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<String> {
        let container_name = format!("raworc_agent_{}", agent_name.to_ascii_lowercase());

        // No content port mapping; preview server is removed.

        // Use agent image directly for all agents
        let container_image = self.agent_image.clone();
        info!(
            "Creating container {} with agent image and generated tokens",
            container_name
        );

        info!(
            "Creating container for agent {} with fresh tokens",
            agent_name
        );

        // Create or get existing agent volume
        let agent_volume = if self.agent_volume_exists(agent_name).await? {
            self.get_agent_volume_name(agent_name)
        } else {
            self.create_agent_volume(agent_name).await?
        };

        let mut labels = HashMap::new();
        labels.insert("raworc.agent".to_string(), agent_name.to_string());
        labels.insert("raworc.managed".to_string(), "true".to_string());
        labels.insert("raworc.volume".to_string(), agent_volume.clone());

        // Configure volume mounts
        let mounts = vec![bollard::models::Mount {
            typ: Some(bollard::models::MountTypeEnum::VOLUME),
            source: Some(agent_volume.clone()),
            target: Some("/agent".to_string()),
            read_only: Some(false),
            ..Default::default()
        }];

        // No port bindings or exposed ports needed.

        // Set environment variables for the agent structure
        let mut env = vec![
            format!("RAWORC_API_URL=http://raworc_api:9000"),
            format!("RAWORC_AGENT_NAME={}", agent_name),
            format!("RAWORC_AGENT_DIR=/agent"),
            // Set the generated system tokens directly as environment variables
            format!("RAWORC_TOKEN={}", raworc_token),
            format!("RAWORC_PRINCIPAL={}", principal),
            format!("RAWORC_PRINCIPAL_TYPE={}", principal_type),
        ];

        // Propagate host branding and URL to agents (provided by start script)
        let host_name = std::env::var("RAWORC_HOST_NAME").unwrap_or_else(|_| "Raworc".to_string());
        let host_url = std::env::var("RAWORC_HOST_URL")
            .expect("RAWORC_HOST_URL must be set by the start script");
        env.push(format!("RAWORC_HOST_NAME={}", host_name));
        env.push(format!("RAWORC_HOST_URL={}", host_url));

        // Configure Ollama host for model inference (required; no default)
        let ollama_host = std::env::var("OLLAMA_HOST").map_err(|_| {
            anyhow::anyhow!("Controller requires OLLAMA_HOST to be set (e.g., http://ollama:11434)")
        })?;
        env.push(format!("OLLAMA_HOST={}", ollama_host));
        let ollama_model =
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gpt-oss:20b".to_string());
        env.push(format!("OLLAMA_MODEL={}", ollama_model));

        // No web_search tool; do not propagate BRAVE_API_KEY

        // Add hint about setup script availability to avoid unnecessary waiting
        if setup.is_some() {
            env.push("RAWORC_HAS_SETUP=true".to_string());
        }

        // Add task creation timestamp for message processing
        if let Some(timestamp) = task_created_at {
            env.push(format!("RAWORC_TASK_CREATED_AT={}", timestamp.to_rfc3339()));
        }

        info!("Set RAWORC_TOKEN and OLLAMA_HOST as environment variables");

        // Add user secrets as environment variables (but NOT RAWORC_TOKEN or OLLAMA_HOST)
        if let Some(secrets_map) = &secrets {
            for (key, value) in secrets_map {
                // Skip if user provided their own RAWORC_TOKEN or OLLAMA_HOST - we use system-managed values
                if key == "RAWORC_TOKEN" || key == "OLLAMA_HOST" {
                    info!(
                        "Skipping user-provided {} - using system-managed value instead",
                        key
                    );
                    continue;
                }
                env.push(format!("{}={}", key, value));
                info!(
                    "Adding user secret {} as environment variable for agent {}",
                    key, agent_name
                );
            }
        }

        // Set the command with required arguments
        let cmd = vec![
            "raworc-agent".to_string(),
            "--api-url".to_string(),
            "http://raworc_api:9000".to_string(),
            "--agent-name".to_string(),
            agent_name.to_string(),
        ];

        let config = Config {
            image: Some(container_image),
            hostname: Some(format!("agent-{}", &agent_name[..agent_name.len().min(8)])),
            labels: Some(labels),
            env: Some(env),
            cmd: Some(cmd),
            working_dir: Some("/agent".to_string()), // User starts in their agent
            exposed_ports: None,
            host_config: Some(bollard::models::HostConfig {
                cpu_quota: Some((self.cpu_limit * 100000.0) as i64),
                cpu_period: Some(100000),
                memory: Some(self.memory_limit),
                memory_swap: Some(self.memory_limit),
                network_mode: Some("raworc_network".to_string()),
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

        // Initialize agent structure after starting container so host can execute setup script
        // Only initialize with user secrets (system tokens are already in environment)
        if !self.agent_initialized(&agent_volume).await? {
            let empty_secrets = HashMap::new();
            let secrets_ref = secrets.as_ref().unwrap_or(&empty_secrets);
            self.initialize_agent_structure(
                agent_name,
                secrets_ref,
                instructions.as_deref(),
                setup.as_deref(),
            )
            .await?;
        }

        info!(
            "Container {} created with agent volume {} and fresh system tokens",
            container_name, agent_volume
        );
        Ok(container.id)
    }

    // Sleep container but retain persistent volume (for agent pause/sleep)
    pub async fn sleep_container(&self, agent_name: &str) -> Result<()> {
        let container_name = format!("raworc_agent_{}", agent_name.to_ascii_lowercase());

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

    // Delete container and remove persistent volume (for agent deletion)
    pub async fn delete_container(&self, agent_name: &str) -> Result<()> {
        let container_name = format!("raworc_agent_{}", agent_name.to_ascii_lowercase());

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

                // Cleanup the agent volume
                if let Err(e) = self.cleanup_agent_volume(agent_name).await {
                    warn!("Failed to cleanup agent volume for {}: {}", agent_name, e);
                }

                Ok(())
            }
            Err(e) => {
                if e.to_string().contains("404") || e.to_string().contains("No such container") {
                    warn!("Container {} already removed or doesn't exist, proceeding with volume cleanup", container_name);

                    // Still try to cleanup the agent volume
                    if let Err(e) = self.cleanup_agent_volume(agent_name).await {
                        warn!("Failed to cleanup agent volume for {}: {}", agent_name, e);
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

    pub async fn execute_command(&self, agent_name: &str, command: &str) -> Result<String> {
        let container_name = format!("raworc_agent_{}", agent_name.to_ascii_lowercase());

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

    pub async fn publish_content(&self, agent_name: &str) -> Result<()> {
        let container_name = format!("raworc_agent_{}", agent_name.to_ascii_lowercase());
        let public_path = format!("/content/{}", agent_name);

        info!(
            "Publishing content for agent {} to {}",
            agent_name, public_path
        );

        // Ensure directory exists inside content container
        let mkdir_output = std::process::Command::new("docker")
            .args(&["exec", "raworc_content", "mkdir", "-p", &public_path])
            .output();
        if let Err(e) = mkdir_output {
            return Err(anyhow::anyhow!("Failed to create content directory: {}", e));
        }

        // Copy content files directly into content container
        let copy_cmd = [
            "docker",
            "cp",
            &format!("{}:/agent/content/.", container_name),
            &format!("raworc_content:{}/", public_path),
        ];
        match std::process::Command::new(copy_cmd[0])
            .args(&copy_cmd[1..])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    info!("Content published successfully for agent {}", agent_name);
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if stderr.contains("No such file or directory") {
                        info!(
                            "No content files found for agent {}, creating empty directory",
                            agent_name
                        );
                        Ok(())
                    } else {
                        error!(
                            "Failed to copy content files for agent {}: {}",
                            agent_name, stderr
                        );
                        Err(anyhow::anyhow!("Failed to copy content files: {}", stderr))
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to execute docker cp command for agent {}: {}",
                    agent_name, e
                );
                Err(anyhow::anyhow!("Failed to execute docker cp: {}", e))
            }
        }
    }

    pub async fn unpublish_content(&self, agent_name: &str) -> Result<()> {
        let public_path = format!("/content/{}", agent_name);

        info!(
            "Unpublishing content for agent {} from content container: {}",
            agent_name, public_path
        );

        // Remove public directory from server container using docker exec
        let output = std::process::Command::new("docker")
            .args(&["exec", "raworc_content", "rm", "-rf", &public_path])
            .output()
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to execute docker exec rm command for agent {}: {}",
                    agent_name,
                    e
                )
            })?;

        if output.status.success() {
            info!("Content unpublished successfully for agent {}", agent_name);
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            error!(
                "Failed to remove public directory for agent {}: stdout: {}, stderr: {}",
                agent_name, stdout, stderr
            );
            Err(anyhow::anyhow!(
                "Failed to remove public directory for agent {}: stdout: {}, stderr: {}",
                agent_name,
                stdout,
                stderr
            ))
        }
    }

    /// Check if an agent container exists and is running healthily
    pub async fn is_container_healthy(&self, agent_name: &str) -> Result<bool> {
        let container_name = format!("raworc_agent_{}", agent_name.to_ascii_lowercase());

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
                            info!("Agent {} container exists but is not running", agent_name);
                            return Ok(false);
                        }
                    }
                }
                // Container state is unclear, assume unhealthy
                warn!("Agent {} container state is unclear", agent_name);
                Ok(false)
            }
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            }) => {
                // Container doesn't exist
                info!("Agent {} container does not exist", agent_name);
                Ok(false)
            }
            Err(e) => {
                // Other Docker API error
                error!("Failed to inspect agent {} container: {}", agent_name, e);
                Err(anyhow::anyhow!(
                    "Docker API error for agent {}: {}",
                    agent_name,
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
