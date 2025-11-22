use anyhow::{anyhow, Result};
use bollard::{
    container::{
        Config, CreateContainerOptions, DownloadFromContainerOptions, LogsOptions,
        RemoveContainerOptions, UploadToContainerOptions,
    },
    exec::{CreateExecOptions, StartExecResults},
    models::{HostConfig, Mount, MountTypeEnum},
    Docker,
};
use bytes::Bytes;
use futures::StreamExt;
use sqlx::MySqlPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::controller::shared_config::TsbxConfig;
use crate::controller::shared_inference::{InferenceRegistry, ResolvedInferenceTarget};

pub struct DockerManager {
    docker: Docker,
    sandbox_image: String,
    cpu_limit: f64,
    memory_limit: i64,
    db_pool: MySqlPool,
    config: Arc<TsbxConfig>,
    inference_registry: Arc<InferenceRegistry>,
}

fn render_env_file(env: &HashMap<String, String>) -> String {
    let mut lines = String::from(
        "# TSBX sandbox environment\n# Managed by TSBX controller; do not modify without explicit approval.\n",
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
    pub fn new(
        docker: Docker,
        db_pool: MySqlPool,
        config: Arc<TsbxConfig>,
        inference_registry: Arc<InferenceRegistry>,
    ) -> Self {
        Self {
            docker,
            db_pool,
            sandbox_image: std::env::var("SANDBOX_IMAGE")
                .unwrap_or_else(|_| "tsbx_sandbox:latest".to_string()),
            cpu_limit: std::env::var("SANDBOX_CPU_LIMIT")
                .unwrap_or_else(|_| "0.5".to_string())
                .parse()
                .unwrap_or(0.5),
            memory_limit: std::env::var("SANDBOX_MEMORY_LIMIT")
                .unwrap_or_else(|_| "536870912".to_string())
                .parse()
                .unwrap_or(536870912),
            config,
            inference_registry,
        }
    }

    // NEW: Create sandbox volume with explicit naming
    async fn create_sandbox_volume(&self, sandbox_id: &str) -> Result<String> {
        let volume_name = format!("tsbx_sandbox_data_{}", sandbox_id);

        let mut labels = HashMap::new();
        labels.insert("tsbx.sandbox_id".to_string(), sandbox_id.to_string());
        labels.insert("tsbx.type".to_string(), "sandbox_volume".to_string());
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
        info!("Created sandbox volume: {}", volume_name);

        Ok(volume_name)
    }

    // Get sandbox volume name (derived from sandbox ID)
    fn get_sandbox_volume_name(&self, sandbox_id: &str) -> String {
        format!("tsbx_sandbox_data_{}", sandbox_id)
    }

    // Get sandbox container name (derived from sandbox ID)
    fn get_sandbox_container_name(&self, sandbox_id: &str) -> String {
        format!("tsbx_sandbox_{}", sandbox_id)
    }

    async fn resolve_inference_target(
        &self,
        sandbox_id: &str,
        provider_override: Option<String>,
        model_override: Option<String>,
    ) -> Result<ResolvedInferenceTarget> {
        let (db_provider, db_model) = sqlx::query_as::<_, (Option<String>, Option<String>)>(
            "SELECT inference_provider, inference_model FROM sandboxes WHERE id = ?",
        )
        .bind(sandbox_id)
        .fetch_one(&self.db_pool)
        .await?;

        let provider = provider_override
            .or(db_provider)
            .filter(|s| !s.trim().is_empty());
        let model = model_override.or(db_model).filter(|s| !s.trim().is_empty());

        self.inference_registry
            .resolve_provider_and_model(provider.as_deref(), model.as_deref())
            .map_err(|e| anyhow!(e.to_string()))
    }

    // Check if sandbox volume exists
    async fn sandbox_volume_exists(&self, sandbox_id: &str) -> Result<bool> {
        let volume_name = self.get_sandbox_volume_name(sandbox_id);
        match self.docker.inspect_volume(&volume_name).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    // Initialize sandbox directory structure with env, instructions, and setup
    async fn initialize_sandbox_structure(
        &self,
        sandbox_id: &str,
        env: &HashMap<String, String>,
        instructions: Option<&str>,
        setup: Option<&str>,
    ) -> Result<()> {
        info!("Initializing sandbox structure for sandbox {}", sandbox_id);

        // Create base directories with proper ownership
        // Use sudo to ensure proper ownership since volume may be root-owned initially
        let init_script = "sudo mkdir -p /sandbox/logs
sudo touch /sandbox/.env
sudo chown sandbox:sandbox /sandbox/.env
sudo chmod 600 /sandbox/.env
sudo chown -R sandbox:sandbox /sandbox
sudo chmod -R 755 /sandbox
echo 'Session directories created (.env, logs)'
";

        self.execute_command(sandbox_id, init_script).await?;

        // Write env values to /sandbox/.env
        let env_content = render_env_file(env);
        let write_env_script = format!(
            "cat <<'EOF_ENV' | sudo tee /sandbox/.env >/dev/null\n{}EOF_ENV\nsudo chown sandbox:sandbox /sandbox/.env\nsudo chmod 600 /sandbox/.env\n",
            env_content
        );
        self.execute_command(sandbox_id, &write_env_script).await?;

        // Write instructions if provided
        if let Some(instructions_content) = instructions {
            let escaped_instructions = instructions_content.replace("'", "'\"'\"'");
            let write_instructions_command =
                format!("echo '{}' > /sandbox/instructions.md", escaped_instructions);
            self.execute_command(sandbox_id, &write_instructions_command)
                .await?;
        }

        // Write and make setup script executable if provided
        if let Some(setup_content) = setup {
            let escaped_setup = setup_content.replace("'", "'\"'\"'");
            let write_setup_command = format!(
                "echo '{}' > /sandbox/setup.sh && chmod +x /sandbox/setup.sh",
                escaped_setup
            );
            self.execute_command(sandbox_id, &write_setup_command)
                .await?;
        }

        info!(
            "Session structure initialized with {} env entries",
            env.len()
        );

        Ok(())
    }

    // NEW: Check if sandbox is initialized
    async fn sandbox_initialized(&self, _volume_name: &str) -> Result<bool> {
        // Skip initialization check for now - always initialize
        Ok(false)
    }

    // NEW: Cleanup sandbox volume
    pub async fn cleanup_sandbox_volume(&self, sandbox_id: &str) -> Result<()> {
        let expected_volume_name = format!("tsbx_sandbox_data_{}", sandbox_id);

        match self.docker.remove_volume(&expected_volume_name, None).await {
            Ok(_) => {
                info!("Removed sandbox volume: {}", expected_volume_name);
            }
            Err(e) => warn!(
                "Failed to remove sandbox volume {}: {}",
                expected_volume_name, e
            ),
        }

        Ok(())
    }

    pub async fn create_container_with_volume_copy(
        &self,
        sandbox_id: &str,
        parent_sandbox_id: &str,
    ) -> Result<String> {
        info!(
            "Creating cloned sandbox {} with volume copy from {}",
            sandbox_id, parent_sandbox_id
        );

        // First create the container normally (this creates the empty target volume)
        let container_name = self.create_container(sandbox_id).await?;

        // Then copy data from parent volume to new volume using Docker command
        let parent_volume = format!("tsbx_sandbox_data_{}", parent_sandbox_id);
        let new_volume = format!("tsbx_sandbox_data_{}", sandbox_id);

        info!(
            "Copying volume data from {} to {}",
            parent_volume, new_volume
        );

        // Use bollard Docker API to create copy container
        let copy_container_name = format!("tsbx_volume_copy_{}", sandbox_id);

        let config = Config {
            image: Some(self.sandbox_image.clone()),
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
        sandbox_id: &str,
        parent_sandbox_id: &str,
    ) -> Result<String> {
        info!(
            "Creating cloned sandbox {} with full copy from {}",
            sandbox_id, parent_sandbox_id
        );

        // First create the sandbox volume (without starting container)
        self.create_sandbox_volume(sandbox_id).await?;

        // Then copy specific directories from parent volume to new volume
        let parent_volume = format!("tsbx_sandbox_data_{}", parent_sandbox_id);
        let new_volume = format!("tsbx_sandbox_data_{}", sandbox_id);

        info!(
            "Copying full volume data from {} to {}",
            parent_volume, new_volume
        );

        // Copy entire /sandbox directory from parent
        let copy_command = "cp -a /source/. /dest/ 2>/dev/null || echo 'No source data'; echo 'Full volume copy completed'".to_string();

        // Use bollard Docker API to create copy container
        let copy_container_name = format!("tsbx_volume_copy_{}", sandbox_id);

        let config = Config {
            image: Some(self.sandbox_image.clone()),
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

        info!("Environment entries in cloned sandbox: {}", env.len());

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
            .create_container_internal(sandbox_id, Some(env), instructions, setup)
            .await?;

        Ok(container_name)
    }

    pub async fn create_container_with_full_copy_and_tokens(
        &self,
        sandbox_id: &str,
        parent_sandbox_id: &str,
        tsbx_token: String,
        principal: String,
        principal_type: String,
        request_created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<String> {
        info!(
            "Creating cloned sandbox {} with full copy from {} and fresh tokens",
            sandbox_id, parent_sandbox_id
        );

        // First create the sandbox volume (without starting container)
        self.create_sandbox_volume(sandbox_id).await?;

        // Then copy specific directories from parent volume to new volume
        let parent_volume = format!("tsbx_sandbox_data_{}", parent_sandbox_id);
        let new_volume = format!("tsbx_sandbox_data_{}", sandbox_id);

        info!(
            "Copying full volume data from {} to {}",
            parent_volume, new_volume
        );

        // Copy entire /sandbox directory from parent
        let copy_command = "cp -a /source/. /dest/ 2>/dev/null || echo 'No source data'; echo 'Full volume copy completed'".to_string();

        // Use bollard Docker API to create copy container
        let copy_container_name = format!("tsbx_volume_copy_{}", sandbox_id);

        let config = Config {
            image: Some(self.sandbox_image.clone()),
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
                sandbox_id,
                Some(env),
                instructions,
                setup,
                tsbx_token,
                principal,
                principal_type,
                Some(request_created_at),
                None,
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
            image: Some(self.sandbox_image.clone()),
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
        sandbox_id: &str,
        env: std::collections::HashMap<String, String>,
        instructions: Option<String>,
        setup: Option<String>,
    ) -> Result<String> {
        let container_name = self
            .create_container_internal(sandbox_id, Some(env), instructions, setup)
            .await?;
        Ok(container_name)
    }

    pub async fn create_container_with_params_and_tokens(
        &self,
        sandbox_id: &str,
        env: std::collections::HashMap<String, String>,
        instructions: Option<String>,
        setup: Option<String>,
        tsbx_token: String,
        principal: String,
        principal_type: String,
        request_created_at: chrono::DateTime<chrono::Utc>,
        inference_api_key: Option<String>, // New parameter
    ) -> Result<String> {
        let container_name = self
            .create_container_internal_with_tokens(
                sandbox_id,
                Some(env),
                instructions,
                setup,
                tsbx_token,
                principal,
                principal_type,
                Some(request_created_at),
                inference_api_key, // Pass the new parameter
            )
            .await?;
        Ok(container_name)
    }

    pub async fn create_container(&self, sandbox_id: &str) -> Result<String> {
        let container_name = self
            .create_container_internal(sandbox_id, None, None, None)
            .await?;
        Ok(container_name)
    }

    async fn create_container_internal(
        &self,
        sandbox_id: &str,
        env_map: Option<std::collections::HashMap<String, String>>,
        instructions: Option<String>,
        setup: Option<String>,
    ) -> Result<String> {
        let container_name = format!("tsbx_sandbox_{}", sandbox_id);

        // No content port mapping; preview server is removed.

        // Use sandbox image directly for all sandboxes
        let container_image = self.sandbox_image.clone();
        info!(
            "Creating container {} with sandbox image {},",
            container_name, container_image
        );

        info!("Creating container for sandbox {}", sandbox_id);

        // Sandboxes use their own container filesystem (no external volumes)

        let mut labels = HashMap::new();
        labels.insert("tsbx.sandbox_id".to_string(), sandbox_id.to_string());
        labels.insert("tsbx.managed".to_string(), "true".to_string());

        // No external volume mounts - sandbox uses its own container filesystem
        let mounts: Vec<bollard::models::Mount> = vec![];

        // No port bindings or exposed ports needed.

        let inference_target = self
            .resolve_inference_target(sandbox_id, None, None)
            .await?;

        // Set environment variables for the sandbox structure
        let mut env_vars = vec![
            format!("TSBX_API_URL=http://tsbx_api:9000"),
            format!("SANDBOX_ID={}", sandbox_id),
            format!("TSBX_SANDBOX_DIR=/sandbox"),
        ];

        // Propagate host branding and URL to sandboxes (provided by start script)
        let host_name = self.config.host.name.clone();
        let host_url = self.config.host.url.clone();
        env_vars.push(format!("TSBX_HOST_NAME={}", host_name));
        env_vars.push(format!("TSBX_HOST_URL={}", host_url));

        env_vars.push(format!(
            "TSBX_INFERENCE_PROVIDER={}",
            inference_target.provider.name
        ));
        env_vars.push(format!(
            "TSBX_INFERENCE_PROVIDER_NAME={}",
            inference_target.provider.display_name
        ));
        env_vars.push(format!(
            "TSBX_INFERENCE_URL={}",
            inference_target.provider.url
        ));
        env_vars.push(format!("TSBX_INFERENCE_MODEL={}", inference_target.model));

        if let Ok(timeout) = std::env::var("TSBX_INFERENCE_TIMEOUT_SECS") {
            env_vars.push(format!("TSBX_INFERENCE_TIMEOUT_SECS={}", timeout));
        }

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
            // system-managed values like TSBX_TOKEN or inference configuration.
            for (key, value) in env_map {
                if matches!(
                    key.as_str(),
                    "TSBX_TOKEN"
                        | "TSBX_INFERENCE_URL"
                        | "TSBX_INFERENCE_API_KEY"
                        | "TSBX_INFERENCE_TIMEOUT_SECS"
                        | "TSBX_INFERENCE_MODEL"
                ) {
                    info!(
                        "Skipping user-provided {} - using system-managed value instead for sandbox {}",
                        key, sandbox_id
                    );
                    continue;
                }
                env_vars.push(format!("{}={}", key, value));
                if key != "TSBX_PRINCIPAL" && key != "TSBX_PRINCIPAL_TYPE" {
                    info!(
                        "Adding env entry {} as environment variable for sandbox {}",
                        key, sandbox_id
                    );
                }
            }
        }

        // Set the command with required arguments
        let cmd = vec![
            "tsbx-sandbox".to_string(),
            "--api-url".to_string(),
            "http://tsbx_api:9000".to_string(),
            "--sandbox-id".to_string(),
            sandbox_id.to_string(),
        ];

        let config = Config {
            image: Some(container_image),
            hostname: Some(format!(
                "sandbox-{}",
                &sandbox_id[..sandbox_id.len().min(8)]
            )),
            labels: Some(labels),
            env: Some(env_vars),
            cmd: Some(cmd),
            working_dir: Some("/sandbox".to_string()), // User starts in their sandbox
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

        // Initialize sandbox structure after starting container so host can execute setup script
        let empty_env: HashMap<String, String> = HashMap::new();
        let env_ref = env_map.as_ref().unwrap_or(&empty_env);
        self.initialize_sandbox_structure(
            sandbox_id,
            env_ref,
            instructions.as_deref(),
            setup.as_deref(),
        )
        .await?;

        info!(
            "Container {} created (using container filesystem)",
            container_name
        );
        Ok(container.id)
    }

    async fn create_container_internal_with_tokens(
        &self,
        sandbox_id: &str,
        env_map_opt: Option<std::collections::HashMap<String, String>>,
        instructions: Option<String>,
        setup: Option<String>,
        tsbx_token: String,
        principal: String,
        principal_type: String,
        request_created_at: Option<chrono::DateTime<chrono::Utc>>,
        inference_api_key: Option<String>, // New parameter
    ) -> Result<String> {
        let container_name = format!("tsbx_sandbox_{}", sandbox_id);

        // No content port mapping; preview server is removed.

        // Use sandbox image directly for all sandboxes
        let container_image = self.sandbox_image.clone();
        info!(
            "Creating container {} with sandbox image and generated tokens",
            container_name
        );

        info!(
            "Creating container for sandbox {} with fresh tokens",
            sandbox_id
        );

        // Sandboxes use their own container filesystem (no external volumes)

        let mut labels = HashMap::new();
        labels.insert("tsbx.sandbox_id".to_string(), sandbox_id.to_string());
        labels.insert("tsbx.managed".to_string(), "true".to_string());

        // No external volume mounts - sandbox uses its own container filesystem
        let mounts: Vec<bollard::models::Mount> = vec![];

        // No port bindings or exposed ports needed.

        // Set environment variables for the sandbox structure
        let mut env_vars = vec![
            format!("TSBX_API_URL=http://tsbx_api:9000"),
            format!("SANDBOX_ID={}", sandbox_id),
            format!("TSBX_SANDBOX_DIR=/sandbox"),
            // Set the generated system tokens directly as environment variables
            format!("TSBX_TOKEN={}", tsbx_token),
            format!("TSBX_PRINCIPAL={}", principal),
            format!("TSBX_PRINCIPAL_TYPE={}", principal_type),
        ];

        // Propagate host branding and URL to sandboxes (provided by start script)
        let host_name = self.config.host.name.clone();
        let host_url = self.config.host.url.clone();
        env_vars.push(format!("TSBX_HOST_NAME={}", host_name));
        env_vars.push(format!("TSBX_HOST_URL={}", host_url));

        let inference_target = self
            .resolve_inference_target(sandbox_id, None, None)
            .await?;

        env_vars.push(format!(
            "TSBX_INFERENCE_PROVIDER={}",
            inference_target.provider.name
        ));
        env_vars.push(format!(
            "TSBX_INFERENCE_PROVIDER_NAME={}",
            inference_target.provider.display_name
        ));
        env_vars.push(format!(
            "TSBX_INFERENCE_URL={}",
            inference_target.provider.url
        ));
        env_vars.push(format!("TSBX_INFERENCE_MODEL={}", inference_target.model));

        if let Some(trimmed_key) = inference_api_key
            .as_ref()
            .map(|k| k.trim())
            .filter(|k| !k.is_empty())
        {
            info!(
                "Using provided inference API key for sandbox {}",
                sandbox_id
            );
            env_vars.push(format!("TSBX_INFERENCE_API_KEY={}", trimmed_key));
        }

        if let Ok(timeout) = std::env::var("TSBX_INFERENCE_TIMEOUT_SECS") {
            env_vars.push(format!("TSBX_INFERENCE_TIMEOUT_SECS={}", timeout));
        }

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

        info!("Set TSBX_TOKEN and inference configuration as environment variables");

        // Add user env as environment variables (but NOT TSBX_TOKEN or inference configuration)
        if let Some(env_map) = &env_map_opt {
            for (key, value) in env_map {
                if matches!(
                    key.as_str(),
                    "TSBX_TOKEN"
                        | "TSBX_INFERENCE_URL"
                        | "TSBX_INFERENCE_API_KEY"
                        | "TSBX_INFERENCE_TIMEOUT_SECS"
                        | "TSBX_INFERENCE_MODEL"
                ) {
                    info!(
                        "Skipping user-provided {} - using system-managed value instead",
                        key
                    );
                    continue;
                }
                env_vars.push(format!("{}={}", key, value));
                info!(
                    "Adding user env entry {} as environment variable for sandbox {}",
                    key, sandbox_id
                );
            }
        }

        // Set the command with required arguments
        let cmd = vec![
            "tsbx-sandbox".to_string(),
            "--api-url".to_string(),
            "http://tsbx_api:9000".to_string(),
            "--sandbox-id".to_string(),
            sandbox_id.to_string(),
        ];

        let config = Config {
            image: Some(container_image),
            hostname: Some(format!(
                "sandbox-{}",
                &sandbox_id[..sandbox_id.len().min(8)]
            )),
            labels: Some(labels),
            env: Some(env_vars),
            cmd: Some(cmd),
            working_dir: Some("/sandbox".to_string()), // User starts in their sandbox
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

        // Initialize sandbox structure after starting container so host can execute setup script
        // Only initialize with user env (system tokens are already in environment)
        let empty_env: HashMap<String, String> = HashMap::new();
        let env_ref = env_map_opt.as_ref().unwrap_or(&empty_env);
        self.initialize_sandbox_structure(
            sandbox_id,
            env_ref,
            instructions.as_deref(),
            setup.as_deref(),
        )
        .await?;

        info!(
            "Container {} created with fresh system tokens (using container filesystem)",
            container_name
        );
        Ok(container.id)
    }

    // Stop and remove container (sandbox data is lost)
    pub async fn stop_container(&self, sandbox_id: &str) -> Result<()> {
        let container_name = format!("tsbx_sandbox_{}", sandbox_id);

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
                    "Container {} deleted, persistent volume retained",
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

    // Delete container and remove persistent volume (for sandbox deletion)
    pub async fn delete_container(&self, sandbox_id: &str) -> Result<()> {
        let container_name = format!("tsbx_sandbox_{}", sandbox_id);

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

                // Cleanup the sandbox volume
                if let Err(e) = self.cleanup_sandbox_volume(sandbox_id).await {
                    warn!("Failed to cleanup sandbox volume for {}: {}", sandbox_id, e);
                }

                Ok(())
            }
            Err(e) => {
                if e.to_string().contains("404") || e.to_string().contains("No such container") {
                    warn!("Container {} already removed or doesn't exist, proceeding with volume cleanup", container_name);

                    // Still try to cleanup the sandbox volume
                    if let Err(e) = self.cleanup_sandbox_volume(sandbox_id).await {
                        warn!("Failed to cleanup sandbox volume for {}: {}", sandbox_id, e);
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

    // Create a snapshot of a sandbox by copying its /sandbox/ directory to the snapshots volume
    pub async fn create_snapshot(&self, sandbox_id: &str, snapshot_id: &str) -> Result<()> {
        let container_name = format!("tsbx_sandbox_{}", sandbox_id);

        info!(
            "Creating snapshot {} for sandbox {}",
            snapshot_id, sandbox_id
        );

        // Check if container exists
        if let Err(_) = self.docker.inspect_container(&container_name, None).await {
            warn!(
                "Container {} not found, cannot create snapshot",
                container_name
            );
            return Err(anyhow::anyhow!(
                "Cannot create snapshot: container {} not found",
                container_name
            ));
        }

        // Create snapshot directory in controller's local filesystem
        // The controller has /data/snapshots mounted from tsbx_snapshots_data volume
        let snapshot_dir = format!("/data/snapshots/{}", snapshot_id);
        fs::create_dir_all(&snapshot_dir).await?;

        info!(
            "Copying data from container {} to {}",
            container_name, snapshot_dir
        );

        // Download tar stream from container's /sandbox directory
        let options = DownloadFromContainerOptions {
            path: "/sandbox".to_string(),
        };

        let mut tar_stream = self
            .docker
            .download_from_container(&container_name, Some(options));

        // Write tar file to snapshot directory
        let tar_path = format!("{}/sandbox.tar", snapshot_dir);
        let mut tar_file = fs::File::create(&tar_path).await?;

        while let Some(chunk) = tar_stream.next().await {
            let bytes = chunk?;
            tar_file.write_all(&bytes).await?;
        }

        tar_file.flush().await?;
        drop(tar_file);

        // Extract the tar file
        let extract_status = tokio::process::Command::new("tar")
            .arg("-xf")
            .arg("sandbox.tar")
            .current_dir(&snapshot_dir)
            .status()
            .await?;

        if !extract_status.success() {
            return Err(anyhow::anyhow!("Failed to extract snapshot tar file"));
        }

        // Remove the tar file after extraction
        let _ = fs::remove_file(&tar_path).await;

        info!("Snapshot {} created successfully", snapshot_id);
        Ok(())
    }

    // Restore a snapshot to a sandbox by copying from /data/snapshots/{snapshot_id}/ to container's /sandbox/
    pub async fn restore_snapshot(&self, sandbox_id: &str, snapshot_id: &str) -> Result<()> {
        let container_name = format!("tsbx_sandbox_{}", sandbox_id);

        info!(
            "Restoring snapshot {} to sandbox {}",
            snapshot_id, sandbox_id
        );

        let snapshot_dir = format!("/data/snapshots/{}", snapshot_id);
        let snapshot_root = format!("{}/sandbox", snapshot_dir);

        let source_dir = if tokio::fs::metadata(&snapshot_root).await.is_ok() {
            snapshot_root
        } else if tokio::fs::metadata(&snapshot_dir).await.is_ok() {
            snapshot_dir.clone()
        } else {
            return Err(anyhow::anyhow!(
                "Snapshot {} not found on controller",
                snapshot_id
            ));
        };

        // Clean the target directory inside the sandbox container
        let cleanup_cmd =
            "set -euo pipefail; rm -rf /sandbox/* /sandbox/.[!.]* /sandbox/..?* || true";
        self.execute_command(sandbox_id, cleanup_cmd).await?;

        // Create a tar stream from the snapshot directory
        let tar_output = tokio::process::Command::new("tar")
            .arg("-C")
            .arg(&source_dir)
            .arg("-cf")
            .arg("-")
            .arg(".")
            .output()
            .await?;

        if !tar_output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to archive snapshot {}",
                snapshot_id
            ));
        }

        // Upload tar stream into the container's /sandbox directory
        let tar_bytes = Bytes::from(tar_output.stdout);
        let upload_opts = UploadToContainerOptions::<&str> {
            path: "/sandbox",
            no_overwrite_dir_non_dir: "false",
        };

        self.docker
            .upload_to_container(&container_name, Some(upload_opts), tar_bytes)
            .await?;

        info!(
            "Snapshot {} restored successfully to sandbox {}",
            snapshot_id, sandbox_id
        );

        Ok(())
    }

    pub async fn execute_command(&self, sandbox_id: &str, command: &str) -> Result<String> {
        let container_name = format!("tsbx_sandbox_{}", sandbox_id);

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
        sandbox_id: &str,
        cmd: Vec<String>,
    ) -> Result<(i32, Vec<u8>, Vec<u8>)> {
        let container_name = format!("tsbx_sandbox_{}", sandbox_id);
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

    /// Check if a sandbox container exists and is running healthily
    pub async fn is_container_healthy(&self, sandbox_id: &str) -> Result<bool> {
        let container_name = format!("tsbx_sandbox_{}", sandbox_id);

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
                            info!("Session {} container exists but is not running", sandbox_id);
                            return Ok(false);
                        }
                    }
                }
                // Container state is unclear, assume unhealthy
                warn!("Session {} container state is unclear", sandbox_id);
                Ok(false)
            }
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            }) => {
                // Container doesn't exist
                info!("Sandbox {} container does not exist", sandbox_id);
                Ok(false)
            }
            Err(e) => {
                // Other Docker API error
                error!("Failed to inspect sandbox {} container: {}", sandbox_id, e);
                Err(anyhow::anyhow!(
                    "Docker API error for sandbox {}: {}",
                    sandbox_id,
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
