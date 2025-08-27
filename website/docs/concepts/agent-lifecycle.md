---
sidebar_position: 4
title: Agent Lifecycle & Architecture
---

# Agent Lifecycle & Architecture

Raworc's agent system provides a sophisticated multi-runtime environment where AI agents are pre-built into space images and executed within isolated session containers. The system supports Python, Node.js, and Rust agents with an LLM-powered orchestration layer.

## Agent Architecture Overview

### Complete System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Raworc Agent System                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐  │
│  │   API Server    │────│    Operator     │────│ Session Container│  │
│  │  (REST API)     │    │ (Space Builder) │    │  (Host Agent)   │  │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘  │
│           │                       │                       │         │
│           └─────────────────────────┼───────────────────────┘         │
│                                   │                                 │
│                      ┌─────────────────┐                            │
│                      │     MySQL       │                            │
│                      │ (Agent Metadata)│                            │
│                      └─────────────────┘                            │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                     Session Container Internals                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐  │
│  │   Host Agent    │────│  Claude API     │────│  Custom Agents  │  │
│  │ (Orchestrator)  │    │  (Intelligence) │    │ (Specialized)   │  │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘  │
│           │                       │                       │         │
│           │                       │              ┌────────┼───────┐ │
│           │                       │              │        │       │ │
│       ┌───▼────┐              ┌───▼────┐    ┌───▼────┐ ┌─▼─┐ ┌───▼─┐│
│       │Message │              │Function│    │Python  │ │Rust│ │Node │││
│       │Polling │              │Calling │    │Agent   │ │Agt │ │Agent│││
│       └────────┘              └────────┘    └────────┘ └────┘ └─────┘│
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

**Host Agent (Core Orchestrator):**
- Message polling from API server (every 2 seconds)
- Claude API integration for intelligence
- Agent delegation and execution
- Response aggregation and delivery
- Session state management

**Custom Agents (Specialized Workers):**
- Domain-specific functionality (data analysis, file processing, etc.)
- Language-specific implementations (Python, Rust, Node.js)
- Stateless execution with standardized interfaces
- Pre-compiled for fast startup

**Space Builder (Build-time):**
- Agent repository cloning and compilation
- Dependency installation and caching
- Multi-runtime support and optimization
- Immutable image creation

## Agent Types

### 1. Host Agent (Built-in)

The Host Agent is the core orchestrator that runs in every session container, providing the intelligence layer through Claude API integration.

**Core Responsibilities:**
```rust
pub struct HostAgent {
    session_id: String,
    space: String,
    claude_client: ClaudeClient,
    agent_manager: AgentManager,
    message_poller: MessagePoller,
}

impl HostAgent {
    async fn run(&self) -> Result<()> {
        // Main execution loop
        loop {
            // 1. Poll for new messages
            let messages = self.message_poller.poll().await?;
            
            for message in messages {
                // 2. Update session state to busy
                self.update_session_state("busy").await?;
                
                // 3. Process with Claude API
                let response = self.process_with_claude(&message).await?;
                
                // 4. Execute any agent calls
                let final_response = self.execute_agent_calls(response).await?;
                
                // 5. Send response back
                self.send_response(&final_response).await?;
                
                // 6. Update session state to idle
                self.update_session_state("idle").await?;
            }
            
            // Wait before next poll
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}
```

**Claude Integration:**
```rust
async fn process_with_claude(&self, message: &str) -> Result<ClaudeResponse> {
    let system_prompt = self.build_system_prompt().await?;
    
    let request = ClaudeRequest {
        model: "claude-3-5-sonnet-20241022",
        messages: vec![Message {
            role: "user".to_string(),
            content: message.to_string(),
        }],
        system: Some(system_prompt),
        max_tokens: 4096,
        tools: Some(self.build_agent_tools().await?),
    };
    
    self.claude_client.create_message(request).await
}
```

**System Prompt Construction:**
```rust
async fn build_system_prompt(&self) -> Result<String> {
    let agents = self.get_available_agents().await?;
    let agent_descriptions = agents.iter()
        .map(|a| format!("- {}: {}", a.name, a.description))
        .collect::<Vec<_>>()
        .join("\n");
    
    Ok(format!(r#"
You are an AI assistant running in a Raworc session container. You have access to specialized agents for various tasks.

Available agents in this space:
{}

When you need to perform specialized tasks, use the appropriate agent function calls. Each agent has specific capabilities and should be used for their domain expertise.

Session context:
- Session ID: {}
- Space: {}
- Working directory: /session
"#, agent_descriptions, self.session_id, self.space))
}
```

### 2. Custom Agents (Specialized)

Custom agents are domain-specific tools that implement standardized interfaces for execution within the session container.

**Agent Interface Standard:**
```
Agent Repository Structure:
my-agent/
├── raworc.json         # Agent manifest (required)
├── requirements.txt    # Python dependencies
├── package.json        # Node.js dependencies  
├── Cargo.toml         # Rust dependencies
└── main.py/index.js/src/lib.rs  # Implementation
```

**Agent Manifest (raworc.json):**
```json
{
  "runtime": "python",
  "handler": "main.process_message",
  "description": "Data analysis agent with pandas and matplotlib",
  "capabilities": [
    "data_analysis",
    "visualization", 
    "statistical_modeling"
  ],
  "build_command": "pip install jupyter",
  "metadata": {
    "version": "1.0.0",
    "author": "team@company.com"
  }
}
```

## Agent Execution Model

### Runtime Implementations

#### Python Agent Execution
```rust
async fn execute_python_agent(&self, agent: &AgentInstance, message: &str, context: &Value, run_id: &str) -> Result<String> {
    let (stdout_log, stderr_log) = self.prepare_agent_logs(&agent.name, run_id).await?;
    
    // Use virtual environment if available
    let python_cmd = if agent.repo_path.join("venv/bin/python").exists() {
        agent.repo_path.join("venv/bin/python").to_string_lossy().to_string()
    } else {
        "python3".to_string()
    };
    
    // Build Python execution command
    let script = format!(
        r#"
import sys
import json
sys.path.insert(0, '{}')
import {}

context = json.loads('{}')
result = {}.{}('{}', context)
print(result)
"#,
        agent.repo_path.to_string_lossy(),
        agent.handler_module,
        serde_json::to_string(context)?,
        agent.handler_module,
        agent.handler_function,
        message.replace("'", "\\'")
    );
    
    let stdout_file = std::fs::File::create(&stdout_log)?;
    let stderr_file = std::fs::File::create(&stderr_log)?;
    
    let output = Command::new(&python_cmd)
        .args(&["-c", &script])
        .current_dir(&agent.repo_path)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .output()?;
    
    let result = fs::read_to_string(&stdout_log).await?;
    
    if !output.status.success() {
        let stderr_content = fs::read_to_string(&stderr_log).await?;
        return Err(anyhow::anyhow!("Python agent execution failed: {}", stderr_content));
    }
    
    Ok(result.trim().to_string())
}
```

#### Rust Agent Execution
```rust
async fn execute_rust_agent(&self, agent: &AgentInstance, message: &str, context: &Value, run_id: &str) -> Result<String> {
    let (stdout_log, stderr_log) = self.prepare_agent_logs(&agent.name, run_id).await?;
    
    // Parse handler (e.g., "lib.process_message_sync")  
    let parts: Vec<&str> = agent.handler.split('.').collect();
    if parts.len() != 2 || parts[0] != "lib" {
        return Err(anyhow::anyhow!("Unsupported Rust handler format: {}", agent.handler));
    }
    let function_name = parts[1];
    
    // First try pre-compiled binary with environment variables
    let release_binary_path = agent.repo_path.join("target/release");
    
    if release_binary_path.exists() {
        if let Ok(mut entries) = std::fs::read_dir(&release_binary_path) {
            while let Some(Ok(entry)) = entries.next() {
                let path = entry.path();
                if path.is_file() && self.is_executable(&path) {
                    let stdout_file = std::fs::File::create(&stdout_log)?;
                    let stderr_file = std::fs::File::create(&stderr_log)?;
                    
                    let output = Command::new(&path)
                        .args(&[message])
                        .current_dir(&agent.repo_path)
                        .env("RAWORC_HANDLER", &agent.handler)
                        .env("RAWORC_FUNCTION", function_name)
                        .env("AGENT_CONTEXT", serde_json::to_string(context)?)
                        .stdout(Stdio::from(stdout_file))
                        .stderr(Stdio::from(stderr_file))
                        .output()?;
                    
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
    
    // Fallback to cargo run
    let stdout_file = std::fs::File::create(&stdout_log)?;
    let stderr_file = std::fs::File::create(&stderr_log)?;
    
    let output = Command::new("cargo")
        .args(&["run", "--", message])
        .current_dir(&agent.repo_path)
        .env("RAWORC_HANDLER", &agent.handler)
        .env("AGENT_CONTEXT", serde_json::to_string(context)?)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .output()?;
    
    let result = fs::read_to_string(&stdout_log).await?;
    
    if !output.status.success() {
        let stderr_content = fs::read_to_string(&stderr_log).await?;
        return Err(anyhow::anyhow!("Rust agent execution failed: {}", stderr_content));
    }
    
    Ok(result.trim().to_string())
}
```

#### Node.js Agent Execution
```rust
async fn execute_node_agent(&self, agent: &AgentInstance, message: &str, context: &Value, run_id: &str) -> Result<String> {
    let (stdout_log, stderr_log) = self.prepare_agent_logs(&agent.name, run_id).await?;
    
    // Build Node.js execution script
    let script = format!(
        r#"
const agent = require('./{}');
const context = {};
const result = agent.{}('{}', context);
console.log(result);
"#,
        agent.handler_file,
        serde_json::to_string(context)?,
        agent.handler_function,
        message.replace("'", "\\'")
    );
    
    let stdout_file = std::fs::File::create(&stdout_log)?;
    let stderr_file = std::fs::File::create(&stderr_log)?;
    
    let output = Command::new("node")
        .args(&["-e", &script])
        .current_dir(&agent.repo_path)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .output()?;
    
    let result = fs::read_to_string(&stdout_log).await?;
    
    if !output.status.success() {
        let stderr_content = fs::read_to_string(&stderr_log).await?;
        return Err(anyhow::anyhow!("Node.js agent execution failed: {}", stderr_content));
    }
    
    Ok(result.trim().to_string())
}
```

## Agent Build Process

### Multi-Runtime Build Pipeline

The space builder handles different agent runtimes through auto-detection and optimized build processes:

```rust
async fn build_agent(&self, agent_dir: &Path, agent_data: &AgentData) -> Result<()> {
    let cargo_toml = agent_dir.join("Cargo.toml");
    let requirements_txt = agent_dir.join("requirements.txt");
    let package_json = agent_dir.join("package.json");
    
    if cargo_toml.exists() {
        // Rust agent build
        self.build_rust_agent(agent_dir, agent_data).await?;
    } else if requirements_txt.exists() {
        // Python agent build
        self.build_python_agent(agent_dir, agent_data).await?;
    } else if package_json.exists() {
        // Node.js agent build
        self.build_node_agent(agent_dir, agent_data).await?;
    } else {
        return Err(anyhow::anyhow!("No recognized build files found for agent {}", agent_data.name));
    }
    
    Ok(())
}
```

### Rust Agent Build
```rust
async fn build_rust_agent(&self, agent_dir: &Path, agent_data: &AgentData) -> Result<()> {
    info!("Building Rust agent: {}", agent_data.name);
    
    // Set up cargo cache
    let cargo_cache = agent_dir.join("../cache/cargo");
    std::env::set_var("CARGO_HOME", &cargo_cache);
    
    // Build in release mode for performance
    let output = Command::new("cargo")
        .args(&["build", "--release"])
        .current_dir(agent_dir)
        .output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Rust build failed: {}", stderr));
    }
    
    // Verify binary was created
    let binary_path = agent_dir.join("target/release");
    if !binary_path.exists() {
        return Err(anyhow::anyhow!("Rust binary not found after build"));
    }
    
    info!("Rust agent {} built successfully", agent_data.name);
    Ok(())
}
```

### Python Agent Build
```rust
async fn build_python_agent(&self, agent_dir: &Path, agent_data: &AgentData) -> Result<()> {
    info!("Building Python agent: {}", agent_data.name);
    
    let venv_path = agent_dir.join("venv");
    let requirements_path = agent_dir.join("requirements.txt");
    
    // Try to create virtual environment
    if Command::new("python3")
        .args(&["-m", "venv", "venv"])
        .current_dir(agent_dir)
        .status()?
        .success()
    {
        // Use virtual environment pip
        let pip_path = venv_path.join("bin/pip");
        if pip_path.exists() {
            info!("Installing Python dependencies in virtual environment");
            let output = Command::new(&pip_path)
                .args(&["install", "-r", "requirements.txt"])
                .current_dir(agent_dir)
                .output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("pip install failed: {}", stderr));
            }
        }
    } else {
        // Fallback to global pip with --break-system-packages
        warn!("Virtual environment creation failed, using global pip");
        let output = Command::new("pip3")
            .args(&["install", "-r", "requirements.txt", "--break-system-packages"])
            .current_dir(agent_dir)
            .output()?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Global pip install failed: {}", stderr));
        }
    }
    
    info!("Python agent {} built successfully", agent_data.name);
    Ok(())
}
```

### Node.js Agent Build
```rust
async fn build_node_agent(&self, agent_dir: &Path, agent_data: &AgentData) -> Result<()> {
    info!("Building Node.js agent: {}", agent_data.name);
    
    // Set up npm cache
    let npm_cache = agent_dir.join("../cache/npm");
    std::env::set_var("NPM_CONFIG_CACHE", &npm_cache);
    
    // Install dependencies
    let output = Command::new("npm")
        .args(&["install"])
        .current_dir(agent_dir)
        .output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("npm install failed: {}", stderr));
    }
    
    // Verify node_modules was created
    let node_modules = agent_dir.join("node_modules");
    if !node_modules.exists() {
        return Err(anyhow::anyhow!("node_modules not found after npm install"));
    }
    
    info!("Node.js agent {} built successfully", agent_data.name);
    Ok(())
}
```

## Agent Management

### Agent Registration
```rust
async fn register_agent(&self, space: &str, agent_request: &CreateAgentRequest) -> Result<Agent> {
    // Validate agent repository
    self.validate_agent_repo(&agent_request.source_repo, &agent_request.source_branch).await?;
    
    // Create agent record
    let agent = sqlx::query_as!(Agent, r#"
        INSERT INTO space_agents (
            space, name, description, purpose, source_repo, source_branch, 
            metadata, created_by, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NOW())
        RETURNING *
    "#, 
        space, 
        agent_request.name,
        agent_request.description,
        agent_request.purpose,
        agent_request.source_repo,
        agent_request.source_branch.as_deref().unwrap_or("main"),
        serde_json::to_string(&agent_request.metadata)?,
        created_by
    ).fetch_one(&self.pool).await?;
    
    // Trigger space rebuild
    self.trigger_space_build(space, false).await?;
    
    Ok(agent)
}
```

### Agent Discovery
```rust
async fn discover_agents_in_container(&self) -> Result<Vec<AgentInstance>> {
    let agents_dir = Path::new("/session/agents");
    let mut discovered_agents = Vec::new();
    
    if !agents_dir.exists() {
        return Ok(discovered_agents);
    }
    
    for entry in std::fs::read_dir(agents_dir)? {
        let agent_dir = entry?.path();
        if !agent_dir.is_dir() {
            continue;
        }
        
        let manifest_path = agent_dir.join("raworc.json");
        if !manifest_path.exists() {
            warn!("Agent directory {} missing raworc.json", agent_dir.display());
            continue;
        }
        
        let manifest_content = std::fs::read_to_string(&manifest_path)?;
        let manifest: AgentManifest = serde_json::from_str(&manifest_content)?;
        
        let agent_instance = AgentInstance {
            name: agent_dir.file_name().unwrap().to_string_lossy().to_string(),
            repo_path: agent_dir,
            runtime: manifest.runtime,
            handler: manifest.handler,
            description: manifest.description.unwrap_or_default(),
            capabilities: manifest.capabilities.unwrap_or_default(),
        };
        
        discovered_agents.push(agent_instance);
    }
    
    info!("Discovered {} agents in container", discovered_agents.len());
    Ok(discovered_agents)
}
```

## Agent Logging & Monitoring

### Execution Logging
```rust
async fn prepare_agent_logs(&self, agent_name: &str, run_id: &str) -> Result<(PathBuf, PathBuf)> {
    let logs_dir = Path::new("/session/logs");
    tokio::fs::create_dir_all(logs_dir).await?;
    
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let stdout_log = logs_dir.join(format!("{}_{}_{}_{}_stdout.log", agent_name, timestamp, run_id[..8].to_string(), ""));
    let stderr_log = logs_dir.join(format!("{}_{}_{}_{}_stderr.log", agent_name, timestamp, run_id[..8].to_string(), ""));
    
    Ok((stdout_log, stderr_log))
}
```

### Performance Monitoring
```rust
async fn execute_agent_with_metrics(&self, agent_name: &str, message: &str, context: &Value) -> Result<AgentExecutionResult> {
    let start_time = Instant::now();
    let run_id = Uuid::new_v4().to_string();
    
    // Execute agent
    let result = self.execute_agent(agent_name, message, context, &run_id).await;
    
    let duration = start_time.elapsed();
    
    // Log execution metrics
    let metrics = AgentExecutionMetrics {
        agent_name: agent_name.to_string(),
        run_id: run_id.clone(),
        duration_ms: duration.as_millis() as u64,
        success: result.is_ok(),
        message_length: message.len(),
        response_length: result.as_ref().map(|r| r.len()).unwrap_or(0),
        timestamp: chrono::Utc::now(),
    };
    
    self.record_agent_metrics(metrics).await?;
    
    Ok(AgentExecutionResult {
        run_id,
        result,
        duration,
        logs_available: true,
    })
}
```

## Error Handling & Recovery

### Agent Execution Failures
```rust
async fn handle_agent_error(&self, agent_name: &str, error: &anyhow::Error, run_id: &str) -> Result<String> {
    error!("Agent {} execution failed (run_id: {}): {}", agent_name, run_id, error);
    
    // Check if it's a common recoverable error
    match error.downcast_ref::<AgentError>() {
        Some(AgentError::TimeoutError) => {
            warn!("Agent {} timed out, sending timeout response", agent_name);
            Ok(format!("Agent {} execution timed out. Please try with a simpler request.", agent_name))
        },
        Some(AgentError::DependencyError(dep)) => {
            warn!("Agent {} missing dependency: {}", agent_name, dep);
            Ok(format!("Agent {} is missing required dependency: {}. Please contact administrator.", agent_name, dep))
        },
        Some(AgentError::RuntimeError(msg)) => {
            warn!("Agent {} runtime error: {}", agent_name, msg);
            Ok(format!("Agent {} encountered a runtime error: {}", agent_name, msg))
        },
        _ => {
            // Generic error handling
            Ok(format!("Agent {} is currently unavailable. Error details have been logged.", agent_name))
        }
    }
}
```

### Agent Health Checks
```rust
async fn check_agent_health(&self, agent: &AgentInstance) -> Result<AgentHealthStatus> {
    let test_message = "health_check";
    let test_context = serde_json::json!({"type": "health_check"});
    
    match tokio::time::timeout(
        Duration::from_secs(10),
        self.execute_agent(&agent.name, test_message, &test_context, &Uuid::new_v4().to_string())
    ).await {
        Ok(Ok(_)) => Ok(AgentHealthStatus::Healthy),
        Ok(Err(e)) => {
            warn!("Agent {} health check failed: {}", agent.name, e);
            Ok(AgentHealthStatus::Unhealthy(e.to_string()))
        },
        Err(_) => {
            warn!("Agent {} health check timed out", agent.name);
            Ok(AgentHealthStatus::Timeout)
        }
    }
}
```

## Best Practices

### Agent Development
- Implement standardized error handling
- Use timeouts for long-running operations
- Provide meaningful error messages
- Include health check endpoints
- Document agent capabilities clearly

### Performance Optimization
- Pre-compile agents during space builds
- Use efficient serialization formats
- Implement proper caching strategies
- Monitor resource usage patterns
- Optimize dependency management

### Security
- Validate all input parameters
- Sanitize file system operations
- Limit network access appropriately
- Use secure dependency sources
- Implement proper authentication

### Monitoring
- Log all agent executions
- Track performance metrics
- Monitor error rates
- Set up health check alerts
- Implement distributed tracing

## Future Enhancements

### Planned Features
- **Agent Hot Reloading**: Update agents without rebuilding spaces
- **Agent Pools**: Load balancing across multiple agent instances
- **Agent Versioning**: Multiple versions of agents in same space
- **Agent Marketplace**: Sharing and discovering agents

### Advanced Capabilities
- **Agent Composition**: Chaining multiple agents for complex workflows
- **Agent State Management**: Persistent state across executions
- **Agent Scheduling**: Time-based and event-driven execution
- **Agent Monitoring Dashboard**: Real-time performance insights