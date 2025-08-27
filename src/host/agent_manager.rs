use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tokio::fs;
use tracing::{error, info, warn};
use chrono::Utc;

use super::api::RaworcClient;
use super::claude::ClaudeClient;

#[derive(Debug, Clone)]
pub struct AgentInstance {
    pub repo_path: PathBuf,
    pub runtime: String,
    pub status: AgentStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    Running,
}

pub struct AgentManager {
    api_client: std::sync::Arc<RaworcClient>,
    claude_client: std::sync::Arc<ClaudeClient>,
    agents: HashMap<String, AgentInstance>,
    agent_metadata: HashMap<String, serde_json::Value>,
    agents_dir: PathBuf,
}

impl AgentManager {
    pub fn new(api_client: std::sync::Arc<RaworcClient>, claude_client: std::sync::Arc<ClaudeClient>) -> Self {
        let agents_dir = PathBuf::from(
            std::env::var("RAWORC_AGENTS_DIR")
                .unwrap_or_else(|_| "/session/agents".to_string())
        );
        
        Self {
            api_client,
            claude_client,
            agents: HashMap::new(),
            agent_metadata: HashMap::new(),
            agents_dir,
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        // Ensure agents directory exists
        fs::create_dir_all(&self.agents_dir).await?;
        
        // Fetch and deploy all space agents
        self.refresh_agents().await?;
        
        Ok(())
    }

    pub async fn refresh_agents(&mut self) -> Result<()> {
        info!("Loading space agents (pre-built during space build time)");
        
        // Check if we're running in a space container (agents pre-built)
        let agents_dir = std::path::Path::new("/session/agents");
        if agents_dir.exists() {
            info!("Found pre-built agents directory, loading agent metadata");
            self.load_prebuilt_agents().await?;
            
            // Log successful loading of pre-built agents
            if !self.agents.is_empty() {
                info!("Successfully loaded {} pre-built agents", self.agents.len());
                for agent_name in self.agents.keys() {
                    info!("  - Agent: {}", agent_name);
                }
                return Ok(());
            }
        }
        
        // Fallback: Only if no pre-built agents found, get agent metadata from API
        // but DON'T build them - they should have been built during space build
        warn!("No pre-built agents found - this may indicate a space build issue");
        let agents_response = self.api_client.get_space_agents().await?;
        
        for agent_data in agents_response {
            let agent_name = agent_data["name"].as_str()
                .ok_or_else(|| anyhow::anyhow!("Agent missing name field"))?;
            
            // Store agent metadata for delegation logic but don't build
            self.agent_metadata.insert(agent_name.to_string(), agent_data.clone());
            warn!("Agent {} metadata loaded but not built (should be pre-built)", agent_name);
        }
        
        Ok(())
    }
    
    async fn load_prebuilt_agents(&mut self) -> Result<()> {
        let agents_dir = std::path::Path::new("/session/agents");
        
        // Read all agent directories
        let mut entries = fs::read_dir(agents_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_dir() {
                if let Some(agent_name) = path.file_name().and_then(|n| n.to_str()) {
                    // Skip hidden directories and system directories
                    if !agent_name.starts_with('.') {
                        // Get space from environment (we're in a space container)
                        let space = std::env::var("RAWORC_SPACE_ID").unwrap_or_else(|_| "default".to_string());
                        info!("Loading pre-built agent: {} from space: {}", agent_name, space);
                        
                        // Load agent manifest
                        let manifest_path = path.join("raworc.json");
                        if manifest_path.exists() {
                            match self.load_agent_manifest(&manifest_path, agent_name, &space).await {
                                Ok(_) => info!("Loaded agent {} successfully", agent_name),
                                Err(e) => warn!("Failed to load agent {}: {}", agent_name, e),
                            }
                        } else {
                            warn!("Agent {} missing raworc.json manifest", agent_name);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn load_agent_manifest(&mut self, manifest_path: &std::path::Path, agent_name: &str, space: &str) -> Result<()> {
        let manifest_content = fs::read_to_string(manifest_path).await?;
        let manifest: serde_json::Value = serde_json::from_str(&manifest_content)?;
        
        let runtime = manifest["runtime"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Agent missing runtime in manifest"))?;
        
        // Create agent metadata
        let agent_metadata = serde_json::json!({
            "name": agent_name,
            "space": space,
            "purpose": manifest["purpose"].as_str().unwrap_or("General purpose agent"),
            "description": manifest["description"].as_str().unwrap_or("No description provided"),
            "capabilities": manifest["capabilities"].as_array().unwrap_or(&vec![]).clone(),
            "runtime": runtime
        });
        
        // Create agent instance
        let agent = AgentInstance {
            repo_path: manifest_path.parent().unwrap().to_path_buf(),
            runtime: runtime.to_string(),
            status: AgentStatus::Running,
        };
        
        self.agents.insert(agent_name.to_string(), agent);
        self.agent_metadata.insert(agent_name.to_string(), agent_metadata);
        
        Ok(())
    }



    pub async fn execute_agent(&self, agent_name: &str, message: &str, context: &Value) -> Result<String> {
        let agent = self.agents.get(agent_name)
            .ok_or_else(|| anyhow::anyhow!("Agent {} not found", agent_name))?;
        
        if agent.status != AgentStatus::Running {
            return Err(anyhow::anyhow!("Agent {} is not running (status: {:?})", agent_name, agent.status));
        }
        
        // Generate timestamp-based run ID for this execution
        let run_id = Utc::now().format("%Y%m%d_%H%M%S_%3f").to_string();
        info!("Executing agent {} with message (run_id: {})", agent_name, run_id);
        
        match agent.runtime.as_str() {
            "python" | "python3" | "python3.11" => {
                self.execute_python_agent(agent, agent_name, message, context, &run_id).await
            }
            "node" | "nodejs" => {
                self.execute_node_agent(agent, agent_name, message, context, &run_id).await
            }
            "rust" => {
                self.execute_rust_agent(agent, agent_name, message, context, &run_id).await
            }
            _ => {
                Err(anyhow::anyhow!("Unsupported runtime: {}", agent.runtime))
            }
        }
    }

    async fn prepare_agent_logs(&self, agent_name: &str, run_id: &str) -> Result<(PathBuf, PathBuf)> {
        // Create logs directory in session folder
        let logs_dir = PathBuf::from("/session/logs");
        fs::create_dir_all(&logs_dir).await?;
        
        let stdout_log = logs_dir.join(format!("{}_{}_stdout.log", agent_name, run_id));
        let stderr_log = logs_dir.join(format!("{}_{}_stderr.log", agent_name, run_id));
        
        // Create the log files
        fs::write(&stdout_log, "").await?;
        fs::write(&stderr_log, "").await?;
        
        info!("Agent {} logs prepared: stdout={:?}, stderr={:?}", agent_name, stdout_log, stderr_log);
        
        Ok((stdout_log, stderr_log))
    }

    async fn execute_python_agent(&self, agent: &AgentInstance, agent_name: &str, message: &str, context: &Value, run_id: &str) -> Result<String> {
        // Prepare log files
        let (stdout_log, stderr_log) = self.prepare_agent_logs(agent_name, run_id).await?;
        
        // For Python agents, use virtual environment
        let script = format!(
            r#"
import sys
import json
import os
sys.path.insert(0, "{}")

# Import the handler function
import main
result = main.process_message({}, {})
print(result)
"#,
            agent.repo_path.display(),
            serde_json::to_string(message)?,
            serde_json::to_string(context)?
        );

        let python_path = agent.repo_path.join("venv/bin/python");
        let python_exe = if python_path.exists() {
            python_path.to_string_lossy().to_string()
        } else {
            "python3".to_string()
        };

        let stdout_file = std::fs::File::create(&stdout_log)?;
        let stderr_file = std::fs::File::create(&stderr_log)?;

        let output = Command::new(&python_exe)
            .args(&["-c", &script])
            .current_dir(&agent.repo_path)
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file))
            .output()?;

        // Read the captured stdout
        let result = fs::read_to_string(&stdout_log).await?;

        if !output.status.success() {
            let stderr_content = fs::read_to_string(&stderr_log).await?;
            return Err(anyhow::anyhow!("Python agent execution failed: {}", stderr_content));
        }

        Ok(result.trim().to_string())
    }

    async fn execute_node_agent(&self, agent: &AgentInstance, agent_name: &str, message: &str, context: &Value, run_id: &str) -> Result<String> {
        // Prepare log files
        let (stdout_log, stderr_log) = self.prepare_agent_logs(agent_name, run_id).await?;
        
        let script = format!(
            r#"
const {{ processMessage }} = require('./index.js');

async function run() {{
    const message = {};
    const context = {};
    const result = await processMessage(message, context);
    console.log(result);
}}

run().catch(console.error);
"#,
            serde_json::to_string(message)?,
            serde_json::to_string(context)?
        );

        let stdout_file = std::fs::File::create(&stdout_log)?;
        let stderr_file = std::fs::File::create(&stderr_log)?;

        let output = Command::new("node")
            .args(&["-e", &script])
            .current_dir(&agent.repo_path)
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file))
            .output()?;

        // Read the captured stdout
        let result = fs::read_to_string(&stdout_log).await?;

        if !output.status.success() {
            let stderr_content = fs::read_to_string(&stderr_log).await?;
            return Err(anyhow::anyhow!("Node agent execution failed: {}", stderr_content));
        }

        Ok(result.trim().to_string())
    }

    async fn execute_rust_agent(&self, agent: &AgentInstance, agent_name: &str, message: &str, context: &Value, run_id: &str) -> Result<String> {
        // Prepare log files
        let (stdout_log, stderr_log) = self.prepare_agent_logs(agent_name, run_id).await?;
        
        // Read the agent manifest to get the handler
        let manifest_path = agent.repo_path.join("raworc.json");
        let handler = if manifest_path.exists() {
            let manifest_content = fs::read_to_string(&manifest_path).await?;
            let manifest: serde_json::Value = serde_json::from_str(&manifest_content)?;
            manifest["handler"].as_str().unwrap_or("main").to_string()
        } else {
            "main".to_string()
        };
        
        info!("Rust agent handler: {}", handler);
        
        // If handler specifies a library function, use dynamic library approach
        if handler.contains("lib.") || handler.contains("::") {
            info!("Using library function call for Rust agent: {}", handler);
            return self.execute_rust_library_function(agent, agent_name, message, context, run_id, &handler).await;
        }
        
        // Otherwise use binary execution (fallback)
        info!("Using binary execution for Rust agent");
        
        // First try to use pre-compiled binary (from space build)
        let release_binary_path = agent.repo_path.join("target/release");
        
        // Look for any binary in the release directory
        if release_binary_path.exists() {
            if let Ok(mut entries) = std::fs::read_dir(&release_binary_path) {
                while let Some(Ok(entry)) = entries.next() {
                    let path = entry.path();
                    if path.is_file() && self.is_executable(&path) {
                        info!("Using pre-compiled Rust binary: {:?}", path);
                        
                        let stdout_file = std::fs::File::create(&stdout_log)?;
                        let stderr_file = std::fs::File::create(&stderr_log)?;
                        
                        let output = Command::new(&path)
                            .args(&[message])
                            .current_dir(&agent.repo_path)
                            .env("AGENT_MESSAGE", message)
                            .env("AGENT_CONTEXT", serde_json::to_string(context)?)
                            .stdout(Stdio::from(stdout_file))
                            .stderr(Stdio::from(stderr_file))
                            .output()?;
                        
                        // Read the captured stdout
                        let result = fs::read_to_string(&stdout_log).await?;
                        
                        if !output.status.success() {
                            let stderr_content = fs::read_to_string(&stderr_log).await?;
                            return Err(anyhow::anyhow!("Rust agent execution failed: {}", stderr_content));
                        }

                        return Ok(result.trim().to_string());
                    }
                }
            }
        }
        
        // Fallback: Use cargo run (does not build, only runs pre-built code)
        let cargo_toml_path = agent.repo_path.join("Cargo.toml");
        if !cargo_toml_path.exists() {
            return Err(anyhow::anyhow!(
                "Rust agent '{}' has no pre-compiled binary and no Cargo.toml. Ensure the agent was built during space build time.",
                agent.repo_path.display()
            ));
        }

        info!("Using cargo run for runtime execution (no building) for agent at {:?}", agent.repo_path);
        
        let stdout_file = std::fs::File::create(&stdout_log)?;
        let stderr_file = std::fs::File::create(&stderr_log)?;
        
        let output = Command::new("cargo")
            .args(&["run", "--", message])
            .current_dir(&agent.repo_path)
            .env("AGENT_MESSAGE", message)
            .env("AGENT_CONTEXT", serde_json::to_string(context)?)
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file))
            .output()?;
        
        // Read the captured stdout
        let result = fs::read_to_string(&stdout_log).await?;
        
        if !output.status.success() {
            let stderr_content = fs::read_to_string(&stderr_log).await?;
            return Err(anyhow::anyhow!("Rust agent execution failed: {}", stderr_content));
        }

        Ok(result.trim().to_string())
    }
    
    async fn execute_rust_library_function(&self, agent: &AgentInstance, agent_name: &str, message: &str, context: &Value, run_id: &str, handler: &str) -> Result<String> {
        // For lib.function handlers, use the pre-built binary but set a special env var
        // The main.rs should be modified to check for this and call the library function
        let (stdout_log, stderr_log) = self.prepare_agent_logs(agent_name, run_id).await?;
        
        // Parse handler (e.g., "lib.process_message_sync")
        let function_name = if let Some(func) = handler.strip_prefix("lib.") {
            func
        } else {
            return Err(anyhow::anyhow!("Unsupported Rust handler format: {}", handler));
        };
        
        info!("Executing Rust library function: {}", function_name);
        
        // Call the specific binary for this agent with the handler function
        let binary_name = agent_name.replace('_', "-");
        let binary_path = agent.repo_path.join(format!("target/release/{}", binary_name));
        
        if binary_path.exists() {
            info!("Calling Rust binary with handler function: {:?}", binary_path);
            
            let stdout_file = std::fs::File::create(&stdout_log)?;
            let stderr_file = std::fs::File::create(&stderr_log)?;
            
            let output = Command::new(&binary_path)
                .args(&[message])
                .current_dir(&agent.repo_path)
                .env("RAWORC_HANDLER", handler)
                .env("AGENT_CONTEXT", serde_json::to_string(context)?)
                .stdout(Stdio::from(stdout_file))
                .stderr(Stdio::from(stderr_file))
                .output()?;
            
            let result = fs::read_to_string(&stdout_log).await?;
            
            if !output.status.success() {
                let stderr_content = fs::read_to_string(&stderr_log).await?;
                return Err(anyhow::anyhow!("Rust handler execution failed: {}", stderr_content));
            }

            return Ok(result.trim().to_string());
        }
        
        // Fallback to cargo run with library function environment
        info!("Using cargo run for library function execution");
        
        let stdout_file = std::fs::File::create(&stdout_log)?;
        let stderr_file = std::fs::File::create(&stderr_log)?;
        
        let output = Command::new("cargo")
            .args(&["run", "--", message])
            .current_dir(&agent.repo_path)
            .env("RAWORC_HANDLER", handler)
            .env("RAWORC_FUNCTION", function_name)
            .env("AGENT_MESSAGE", message)
            .env("AGENT_CONTEXT", serde_json::to_string(context)?)
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file))
            .output()?;
        
        let result = fs::read_to_string(&stdout_log).await?;
        
        if !output.status.success() {
            let stderr_content = fs::read_to_string(&stderr_log).await?;
            return Err(anyhow::anyhow!("Rust library function execution failed: {}", stderr_content));
        }

        Ok(result.trim().to_string())
    }
    
    fn is_executable(&self, path: &std::path::Path) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            
            // Get file name
            let file_name = match path.file_name() {
                Some(name) => name.to_string_lossy(),
                None => return false,
            };
            
            // Skip hidden files (starting with .)
            if file_name.starts_with('.') {
                return false;
            }
            
            // Exclude library files and metadata files
            if let Some(extension) = path.extension() {
                let ext_str = extension.to_string_lossy().to_lowercase();
                if matches!(ext_str.as_str(), "rlib" | "so" | "dylib" | "a" | "d" | "lock") {
                    return false;
                }
            }
            
            // Must be a regular file with significant size (not empty)
            if let Ok(metadata) = std::fs::metadata(path) {
                if !metadata.is_file() || metadata.len() == 0 {
                    return false;
                }
                
                let permissions = metadata.permissions();
                // Check if owner has execute permission
                return permissions.mode() & 0o100 != 0;
            }
            false
        }
        
        #[cfg(windows)]
        {
            // On Windows, check if the file exists and has a typical executable extension
            if let Some(extension) = path.extension() {
                let ext_str = extension.to_string_lossy().to_lowercase();
                matches!(ext_str.as_str(), "exe" | "bat" | "cmd" | "com" | "scr")
            } else {
                // If no extension, assume not executable on Windows
                false
            }
        }
        
        #[cfg(not(any(unix, windows)))]
        { 
            // For other platforms, just check if file exists
            path.exists()
        }
    }

    pub async fn get_agent_for_message(&self, message: &str) -> Option<String> {
        // Get running agents
        let running_agents: Vec<_> = self.agents
            .iter()
            .filter(|(_, agent)| agent.status == AgentStatus::Running)
            .collect();
            
        if running_agents.is_empty() {
            return None;
        }
        
        // Use Claude to determine best agent match
        match self.claude_delegate_agent(message, &running_agents).await {
            Ok(Some(agent_name)) => Some(agent_name),
            Ok(None) => None,
            Err(e) => {
                error!("Claude delegation failed, falling back to simple matching: {}", e);
                self.fallback_simple_delegation(message)
            }
        }
    }
    
    async fn claude_delegate_agent(&self, message: &str, running_agents: &[(&String, &AgentInstance)]) -> Result<Option<String>> {
        // Build agent information for Claude
        let mut agent_info = String::new();
        agent_info.push_str("Available agents:\n");
        
        for (name, _) in running_agents {
            if let Some(metadata) = self.agent_metadata.get(*name) {
                agent_info.push_str(&format!("- {}: ", name));
                
                if let Some(purpose) = metadata["purpose"].as_str() {
                    agent_info.push_str(&format!("Purpose: {}", purpose));
                }
                
                if let Some(description) = metadata["description"].as_str() {
                    agent_info.push_str(&format!(", Description: {}", description));
                }
                
                if let Some(capabilities) = metadata["capabilities"].as_array() {
                    let caps: Vec<String> = capabilities
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect();
                    if !caps.is_empty() {
                        agent_info.push_str(&format!(", Capabilities: {}", caps.join(", ")));
                    }
                }
                
                agent_info.push('\n');
            } else {
                agent_info.push_str(&format!("- {}: (metadata unavailable)\n", name));
            }
        }
        
        let system_prompt = format!(
            r#"You are an intelligent agent routing system. Given a user message and available agents, determine which agent (if any) would be best suited to handle the request.

Rules:
1. Only respond with the exact agent name if there's a clear match
2. Respond with "NONE" if no agent is suitable 
3. Consider the agent's purpose, description, and capabilities
4. Match based on the intent and domain of the user's message
5. Be conservative - only delegate if there's a strong match

{}"#,
            agent_info
        );
        
        let messages = vec![
            ("user".to_string(), format!("User message: \"{}\"", message))
        ];
        
        match self.claude_client.complete(messages, Some(system_prompt)).await {
            Ok(response) => {
                let agent_name = response.trim();
                if agent_name == "NONE" || agent_name.is_empty() {
                    Ok(None)
                } else {
                    // Verify the agent name exists and is running
                    if running_agents.iter().any(|(name, _)| name.as_str() == agent_name) {
                        Ok(Some(agent_name.to_string()))
                    } else {
                        info!("Claude suggested non-existent agent: {}", agent_name);
                        Ok(None)
                    }
                }
            }
            Err(e) => Err(e.into())
        }
    }
    
    fn fallback_simple_delegation(&self, message: &str) -> Option<String> {
        let message_lower = message.to_lowercase();
        
        for (name, agent) in &self.agents {
            if agent.status == AgentStatus::Running {
                if let Some(purpose) = self.get_agent_purpose(name) {
                    let purpose_lower = purpose.to_lowercase();
                    
                    if message_lower.contains("stock") || message_lower.contains("finance") || message_lower.contains("$") {
                        if purpose_lower.contains("financial") || purpose_lower.contains("stock") {
                            return Some(name.clone());
                        }
                    }
                    
                    if message_lower.contains("data") || message_lower.contains("analyze") {
                        if purpose_lower.contains("data") || purpose_lower.contains("analysis") {
                            return Some(name.clone());
                        }
                    }
                }
            }
        }
        
        None
    }

    fn get_agent_purpose(&self, agent_name: &str) -> Option<String> {
        self.agent_metadata.get(agent_name)
            .and_then(|metadata| metadata["purpose"].as_str())
            .map(|s| s.to_string())
    }

}