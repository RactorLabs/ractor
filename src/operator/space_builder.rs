use anyhow::Result;
use bollard::Docker;
use bollard::image::BuildImageOptions;
use sqlx::MySqlPool;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs;
use tracing::{error, info, warn};

pub struct SpaceBuilder {
    docker: Docker,
    pool: Arc<MySqlPool>,
}

impl SpaceBuilder {
    pub fn new(pool: Arc<MySqlPool>) -> Result<Self> {
        let docker = Docker::connect_with_socket_defaults()?;
        Ok(Self { docker, pool })
    }
    
    pub async fn build_space(
        space: &str,
        build_id: &str,
        force_rebuild: bool,
        pool: Arc<MySqlPool>,
    ) -> Result<()> {
        let builder = Self::new(pool.clone())?;
        
        info!("Starting space build for {} (build_id: {})", space, build_id);
        
        // Update build status to 'building'
        builder.update_build_status(space, build_id, "building", None, None, None).await?;
        
        match builder.build_space_image(space, build_id, force_rebuild).await {
            Ok((image_tag, agents_deployed)) => {
                info!("Space build completed successfully: {}", image_tag);
                builder.update_build_status(
                    space, 
                    build_id, 
                    "completed", 
                    Some(&image_tag),
                    Some(&agents_deployed),
                    None
                ).await?;
            }
            Err(e) => {
                error!("Space build failed: {}", e);
                builder.update_build_status(
                    space, 
                    build_id, 
                    "failed", 
                    None,
                    None,
                    Some(&e.to_string())
                ).await?;
                return Err(e);
            }
        }
        
        Ok(())
    }
    
    async fn build_space_image(
        &self,
        space: &str,
        build_id: &str,
        _force_rebuild: bool,
    ) -> Result<(String, Vec<String>)> {
        // Create temporary directory for build context
        let temp_dir = TempDir::new()?;
        let build_context = temp_dir.path();
        
        info!("Creating build context in {:?}", build_context);
        
        // Check if base image exists and get its ID first
        info!("===== EXECUTING NEW IMAGE ID APPROACH =====");
        let base_image_name = std::env::var("HOST_AGENT_IMAGE")
            .unwrap_or_else(|_| "raworc_host:latest".to_string());
        
        let base_image_for_dockerfile = match self.docker.inspect_image(&base_image_name).await {
            Ok(image_info) => {
                info!("Base image {} found with ID: {:?}", base_image_name, image_info.id);
                
                // Use the image ID instead of name to avoid Docker trying to pull
                if let Some(id) = &image_info.id {
                    info!("WILL USE IMAGE ID: {} instead of name to prevent pull", id);
                    id.clone()
                } else {
                    warn!("No image ID found, falling back to name");
                    base_image_name.clone()
                }
            },
            Err(e) => {
                error!("Base image {} not found: {}", base_image_name, e);
                return Err(anyhow::anyhow!("Base image {} not available: {}", base_image_name, e));
            }
        };
        
        // Create Dockerfile using the image ID instead of name
        let dockerfile_content = self.create_space_dockerfile_with_base(space, &base_image_for_dockerfile).await?;
        info!("Generated Dockerfile content:\n{}", dockerfile_content);
        let dockerfile_path = build_context.join("Dockerfile");
        fs::write(&dockerfile_path, dockerfile_content).await?;
        
        // Fetch and prepare agents
        let agents_deployed = match self.prepare_agents(space, build_context).await {
            Ok(agents) => agents,
            Err(e) => {
                warn!("Failed to prepare agents for space {}: {}", space, e);
                vec![] // Continue build even if no agents or agent preparation fails
            }
        };
        
        // Build Docker image
        let image_tag = format!("raworc_space_{}:{}", space, build_id);
        
        info!("Building Docker image: {}", image_tag);
        
        // Create tar archive of build context
        let tar_data = self.create_tar_archive(build_context).await?;
        
        let build_options = BuildImageOptions {
            dockerfile: "Dockerfile".to_string(),
            t: image_tag.clone(),
            pull: false,  // Don't pull base images - use local images
            rm: true,
            forcerm: true,  // Remove intermediate containers on failure
            nocache: false,  // Use cache if available
            ..Default::default()
        };
        
        let mut build_stream = self.docker.build_image(build_options, None, Some(tar_data.into()));
        
        // Process the complete Docker build stream
        use futures_util::stream::StreamExt;
        use tokio::time::{timeout, Duration};
        
        info!("Starting Docker build stream processing...");
        let mut build_success = false;
        let mut error_message: Option<String> = None;
        
        // Process the build stream with timeout
        let stream_result = timeout(Duration::from_secs(600), async {
            while let Some(build_result) = build_stream.next().await {
                match build_result {
                    Ok(output) => {
                        // Log stream content for debugging
                        if let Some(stream) = &output.stream {
                            let message = stream.trim();
                            if !message.is_empty() {
                                info!("Build: {}", message);
                                
                                // Check for successful completion
                                if message.contains("Successfully tagged") || message.contains("Successfully built") {
                                    build_success = true;
                                    info!("Build success detected: {}", message);
                                }
                            }
                        }
                        
                        // Check for errors
                        if let Some(error) = &output.error {
                            error_message = Some(error.clone());
                            error!("Docker build error: {}", error);
                        }
                        
                        if let Some(error_detail) = &output.error_detail {
                            error!("Docker build error detail: {:?}", error_detail);
                            if error_message.is_none() {
                                error_message = Some(format!("Build error: {:?}", error_detail));
                            }
                        }
                        
                        // Check for aux field which might contain completion info
                        if let Some(aux) = &output.aux {
                            info!("Build aux: {:?}", aux);
                        }
                    }
                    Err(e) => {
                        error!("Docker build stream error: {:?}", e);
                        error_message = Some(format!("Stream error: {}", e));
                        break;
                    }
                }
            }
            
            info!("Docker build stream processing completed. Success: {}", build_success);
            Ok::<(), anyhow::Error>(())
        }).await;
        
        // Handle timeout
        match stream_result {
            Ok(_) => {
                if let Some(err_msg) = error_message {
                    return Err(anyhow::anyhow!("Docker build failed: {}", err_msg));
                }
            },
            Err(_) => {
                return Err(anyhow::anyhow!("Docker build timed out after 10 minutes"));
            }
        }
        
        // Verify the image was actually created
        if !build_success {
            warn!("Build stream ended without success indicator, verifying image exists...");
            match self.docker.inspect_image(&image_tag).await {
                Ok(_) => {
                    info!("Image {} exists despite no success message, considering build successful", image_tag);
                }
                Err(e) => {
                    error!("Image {} not found and no success message: {}", image_tag, e);
                    return Err(anyhow::anyhow!("Docker build completed but image not found"));
                }
            }
        }
        
        info!("Docker image built successfully: {}", image_tag);
        
        Ok((image_tag, agents_deployed))
    }
    
    async fn create_space_dockerfile_with_base(&self, space: &str, base_image: &str) -> Result<String> {
        let dockerfile = format!(
            r#"FROM {}

# Set space
ENV RAWORC_SPACE_ID={}

# Create agent directories
RUN mkdir -p /session/agents /session /session/cache /session/state

# Copy agents to image with proper ownership
COPY --chown=host:host agents/ /session/agents/

# Auto-build agents by detected language/runtime
RUN cd /session/agents && \
    for agent_dir in */; do \
        if [ -d "$agent_dir" ]; then \
            echo "Processing agent in $agent_dir"; \
            cd "$agent_dir"; \
            if [ -f "Cargo.toml" ]; then \
                echo "  -> Building Rust project"; \
                sudo -u host cargo build --release; \
            elif [ -f "requirements.txt" ]; then \
                echo "  -> Setting up Python environment"; \
                if sudo -u host python3 -m venv venv 2>/dev/null && [ -x "./venv/bin/pip" ]; then \
                    echo "    -> Using virtual environment"; \
                    sudo -u host ./venv/bin/pip install -r requirements.txt; \
                else \
                    echo "    -> Virtual environment failed or pip missing, using global pip with --break-system-packages"; \
                    sudo -u host pip3 install -r requirements.txt --break-system-packages; \
                fi; \
            elif [ -f "package.json" ]; then \
                echo "  -> Installing Node.js dependencies"; \
                sudo -u host npm install; \
            else \
                echo "  -> No recognized build files, skipping auto-build"; \
            fi; \
            if [ -f "raworc.json" ]; then \
                build_cmd=$(sudo -u host python3 -c "import json; manifest=json.load(open('raworc.json')); print(manifest.get('build_command', ''))"); \
                if [ ! -z "$build_cmd" ]; then \
                    echo "  -> Running additional build command: $build_cmd"; \
                    sudo -u host bash -c "$build_cmd"; \
                fi; \
            fi; \
            cd ..; \
        fi; \
    done

# Set working directory
WORKDIR /session

# Default command (can be overridden)
CMD ["/usr/local/bin/raworc", "host"]
"#,
            base_image, space
        );
        
        
        Ok(dockerfile)
    }
    
    async fn prepare_agents(&self, space: &str, build_context: &std::path::Path) -> Result<Vec<String>> {
        let agents_dir = build_context.join("agents");
        fs::create_dir_all(&agents_dir).await?;
        
        // Fetch space agents
        let agents = self.fetch_space_agents(space).await?;
        let mut deployed_agents = Vec::new();
        
        info!("Preparing {} agents for space {}", agents.len(), space);
        
        for agent_data in agents {
            let agent_name = agent_data["name"].as_str()
                .ok_or_else(|| anyhow::anyhow!("Agent missing name field"))?;
            
            info!("Preparing agent: {}", agent_name);
            
            match self.checkout_and_build_agent(&agent_data, &agents_dir).await {
                Ok(_) => {
                    deployed_agents.push(agent_name.to_string());
                    info!("Agent {} prepared successfully", agent_name);
                }
                Err(e) => {
                    warn!("Failed to prepare agent {}: {}", agent_name, e);
                }
            }
        }
        
        // Create placeholder file if no agents were successfully deployed
        if deployed_agents.is_empty() {
            let placeholder_path = agents_dir.join(".placeholder");
            fs::write(&placeholder_path, "# Placeholder - no agents deployed\n").await?;
            info!("Created placeholder file for empty agents directory");
        }
        
        Ok(deployed_agents)
    }
    
    async fn fetch_space_agents(&self, space: &str) -> Result<Vec<serde_json::Value>> {
        info!("Fetching agents for space: {}", space);
        
        let agents = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String, Option<String>)>(
            r#"
            SELECT name, space, description, purpose, source_repo, source_branch
            FROM agents
            WHERE space = ? AND status IN ('configured', 'running')
            ORDER BY created_at DESC
            "#
        )
        .bind(space)
        .fetch_all(&*self.pool)
        .await?;
        
        let mut agent_list = Vec::new();
        for (name, space, description, purpose, source_repo, source_branch) in agents {
            let agent_json = serde_json::json!({
                "name": name,
                "space": space,
                "description": description,
                "purpose": purpose,
                "source_repo": source_repo,
                "source_branch": source_branch.unwrap_or_else(|| "main".to_string()),
                "runtime": "python3" // Default runtime, could be stored in DB
            });
            agent_list.push(agent_json);
        }
        
        info!("Found {} agents for space {}", agent_list.len(), space);
        Ok(agent_list)
    }
    
    async fn checkout_and_build_agent(
        &self,
        agent_data: &serde_json::Value,
        agents_dir: &std::path::Path,
    ) -> Result<()> {
        let name = agent_data["name"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Agent missing name"))?;
        let _space = agent_data["space"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Agent missing space"))?;
        let repo = agent_data["source_repo"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Agent missing source_repo"))?;
        let branch = agent_data["source_branch"].as_str().unwrap_or("main");
        
        // Create agent directory (no space prefix needed since we're in a space container)
        let agent_dir = agents_dir.join(name);
        
        // Clone repository
        self.clone_agent_repo(repo, branch, &agent_dir).await?;
        
        // Load and validate manifest (optional)
        let manifest_path = agent_dir.join("raworc.json");
        let manifest = if manifest_path.exists() {
            match fs::read_to_string(&manifest_path).await {
                Ok(content) => {
                    match serde_json::from_str(&content) {
                        Ok(manifest) => manifest,
                        Err(e) => {
                            warn!("Invalid raworc.json for agent {}: {}", name, e);
                            serde_json::json!({}) // Use empty manifest
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read raworc.json for agent {}: {}", name, e);
                    serde_json::json!({}) // Use empty manifest
                }
            }
        } else {
            info!("No raworc.json found for agent {}, using defaults", name);
            serde_json::json!({
                "runtime": "python3",
                "build_command": "pip install -r requirements.txt"
            })
        };
        
        // Build agent
        match self.build_agent(&agent_dir, &manifest).await {
            Ok(_) => info!("Agent {} built successfully", name),
            Err(e) => warn!("Failed to build agent {}: {}", name, e),
        }
        
        Ok(())
    }
    
    async fn clone_agent_repo(&self, repo: &str, branch: &str, target_dir: &std::path::Path) -> Result<()> {
        use std::process::Command;
        
        info!("Cloning {} (branch: {}) to {:?}", repo, branch, target_dir);
        
        let repo_url = if repo.starts_with("http") {
            repo.to_string()
        } else {
            format!("https://github.com/{}.git", repo)
        };
        
        // Ensure parent directory exists
        if let Some(parent) = target_dir.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        // First try to clone with specified branch
        let mut output = Command::new("git")
            .args(&["clone", "--branch", branch, "--depth", "1", &repo_url])
            .arg(target_dir)
            .output()?;
        
        // If branch-specific clone fails, try default branch
        if !output.status.success() {
            warn!("Failed to clone branch '{}', trying default branch", branch);
            output = Command::new("git")
                .args(&["clone", "--depth", "1", &repo_url])
                .arg(target_dir)
                .output()?;
        }
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Git clone failed for repo {}: {}", repo_url, stderr));
        }
        
        info!("Successfully cloned repository: {}", repo_url);
        Ok(())
    }
    
    async fn build_agent(&self, agent_dir: &std::path::Path, manifest: &serde_json::Value) -> Result<()> {
        use std::process::Command;
        
        info!("Auto-building agent in {:?}", agent_dir);
        
        // Auto-detect and build based on language/platform
        let cargo_toml = agent_dir.join("Cargo.toml");
        let requirements_txt = agent_dir.join("requirements.txt");
        let package_json = agent_dir.join("package.json");
        
        if cargo_toml.exists() {
            info!("Detected Rust project, building with cargo");
            let output = Command::new("cargo")
                .args(&["build", "--release"])
                .current_dir(agent_dir)
                .output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("Rust build failed: {}", stderr));
            }
            info!("Rust agent built successfully");
            
        } else if requirements_txt.exists() {
            info!("Detected Python project, setting up virtual environment");
            
            // Create virtual environment
            let venv_output = Command::new("python3")
                .args(&["-m", "venv", "venv"])
                .current_dir(agent_dir)
                .output()?;
            
            if !venv_output.status.success() {
                let stderr = String::from_utf8_lossy(&venv_output.stderr);
                return Err(anyhow::anyhow!("Failed to create virtual environment: {}", stderr));
            }
            
            // Install dependencies
            let pip_output = Command::new("sh")
                .args(&["-c", "./venv/bin/pip install -r requirements.txt"])
                .current_dir(agent_dir)
                .output()?;
            
            if !pip_output.status.success() {
                let stderr = String::from_utf8_lossy(&pip_output.stderr);
                return Err(anyhow::anyhow!("Python dependencies installation failed: {}", stderr));
            }
            info!("Python environment set up successfully");
            
        } else if package_json.exists() {
            info!("Detected Node.js project, installing dependencies");
            let output = Command::new("npm")
                .args(&["install"])
                .current_dir(agent_dir)
                .output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("npm install failed: {}", stderr));
            }
            info!("Node.js dependencies installed successfully");
            
        } else {
            info!("No recognized build files found, skipping automatic build");
        }
        
        // Run optional additional build command if specified
        if let Some(build_command) = manifest["build_command"].as_str() {
            if !build_command.trim().is_empty() {
                info!("Running additional build command: {}", build_command);
                let output = Command::new("sh")
                    .args(&["-c", build_command])
                    .current_dir(agent_dir)
                    .output()?;
                
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(anyhow::anyhow!("Additional build command failed: {}", stderr));
                }
                info!("Additional build command completed successfully");
            }
        }
        
        Ok(())
    }
    
    async fn create_tar_archive(&self, build_context: &std::path::Path) -> Result<Vec<u8>> {
        use tar::Builder;
        use std::io::Cursor;
        
        let mut tar_data = Vec::new();
        {
            let cursor = Cursor::new(&mut tar_data);
            let mut archive = Builder::new(cursor);
            
            // Use custom filtering to avoid macOS-specific tar header type 83 errors
            self.add_filtered_directory_to_tar(&mut archive, build_context, ".")?;
            archive.finish()?;
        }
        
        Ok(tar_data)
    }
    
    fn add_filtered_directory_to_tar(
        &self, 
        archive: &mut tar::Builder<std::io::Cursor<&mut Vec<u8>>>, 
        dir_path: &std::path::Path, 
        tar_path: &str
    ) -> Result<()> {
        use std::fs;
        
        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let file_path = entry.path();
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            
            // Skip problematic files and directories
            if self.should_skip_file(&file_name_str, &file_path)? {
                continue;
            }
            
            let tar_entry_path = if tar_path == "." {
                file_name_str.to_string()
            } else {
                format!("{}/{}", tar_path, file_name_str)
            };
            
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(e) => {
                    warn!("Skipping file {:?} due to metadata error: {}", file_path, e);
                    continue;
                }
            };
            
            // Platform-specific file type validation
            if !self.is_safe_file_type(&metadata, &file_path)? {
                continue;
            }
            
            if metadata.is_file() {
                match archive.append_path_with_name(&file_path, &tar_entry_path) {
                    Ok(_) => {},
                    Err(e) => {
                        warn!("Skipping file {:?} due to tar error: {}", file_path, e);
                        continue;
                    }
                }
            } else if metadata.is_dir() {
                match archive.append_dir(&tar_entry_path, &file_path) {
                    Ok(_) => {
                        if let Err(e) = self.add_filtered_directory_to_tar(archive, &file_path, &tar_entry_path) {
                            warn!("Error processing subdirectory {:?}: {}", file_path, e);
                        }
                    },
                    Err(e) => {
                        warn!("Skipping directory {:?} due to tar error: {}", file_path, e);
                        continue;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    fn should_skip_file(&self, file_name: &str, _file_path: &std::path::Path) -> Result<bool> {
        // Skip hidden files
        if file_name.starts_with('.') {
            return Ok(true);
        }
        
        // Skip build artifacts and cache directories
        if matches!(file_name, 
            "node_modules" | "target" | "__pycache__" | ".git" | ".svn" | ".hg" |
            "build" | "dist" | ".cache" | ".npm" | ".yarn"
        ) {
            return Ok(true);
        }
        
        // Skip macOS-specific files that cause tar header issues
        if matches!(file_name,
            ".DS_Store" | ".AppleDouble" | ".LSOverride" | "Icon\r" |
            ".DocumentRevisions-V100" | ".fseventsd" | ".Spotlight-V100" |
            ".TemporaryItems" | ".Trashes" | ".VolumeIcon.icns" |
            ".com.apple.timemachine.donotpresent"
        ) {
            return Ok(true);
        }
        
        // Skip macOS resource forks
        if file_name.starts_with("._") {
            return Ok(true);
        }
        
        Ok(false)
    }
    
    fn is_safe_file_type(&self, metadata: &std::fs::Metadata, file_path: &std::path::Path) -> Result<bool> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            
            let mode = metadata.mode();
            let file_type = mode & 0o170000;
            
            match file_type {
                0o100000 => Ok(true), // Regular file
                0o040000 => Ok(true), // Directory
                0o020000 => {         // Character device
                    warn!("Skipping character device: {:?}", file_path);
                    Ok(false)
                },
                0o060000 => {         // Block device
                    warn!("Skipping block device: {:?}", file_path);
                    Ok(false)
                },
                0o010000 => {         // Named pipe (FIFO)
                    warn!("Skipping named pipe: {:?}", file_path);
                    Ok(false)
                },
                0o140000 => {         // Socket
                    warn!("Skipping socket: {:?}", file_path);
                    Ok(false)
                },
                0o120000 => {         // Symbolic link
                    warn!("Skipping symbolic link: {:?}", file_path);
                    Ok(false)
                },
                _ => {
                    warn!("Skipping unknown file type {:o}: {:?}", file_type, file_path);
                    Ok(false)
                }
            }
        }
        
        #[cfg(not(unix))]
        {
            // On non-Unix systems, rely on standard file type checks
            if metadata.is_file() || metadata.is_dir() {
                Ok(true)
            } else {
                warn!("Skipping special file on non-Unix system: {:?}", file_path);
                Ok(false)
            }
        }
    }
    
    pub async fn update_build_status(
        &self,
        space: &str,
        build_id: &str,
        status: &str,
        image_tag: Option<&str>,
        agents_deployed: Option<&[String]>,
        error: Option<&str>,
    ) -> Result<()> {
        let completed_at = if status == "completed" || status == "failed" {
            Some(chrono::Utc::now())
        } else {
            None
        };
        
        let agents_json = agents_deployed.map(|agents| {
            serde_json::to_string(agents).unwrap_or_default()
        });
        
        sqlx::query(
            r#"
            UPDATE space_builds 
            SET status = ?, image_tag = ?, completed_at = ?, agents_deployed = ?, error = ?
            WHERE space = ? AND build_id = ?
            "#
        )
        .bind(status)
        .bind(image_tag)
        .bind(completed_at)
        .bind(agents_json)
        .bind(error)
        .bind(space)
        .bind(build_id)
        .execute(&*self.pool)
        .await?;
        
        Ok(())
    }
}