use super::error::{HostError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info};

#[derive(Debug, Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ClaudeMessage>,
    system: Option<String>,
    tools: Option<Vec<Tool>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ClaudeMessage {
    role: String,
    content: Value,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClaudeContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

#[derive(Debug, Serialize)]
struct Tool {
    name: String,
    description: String,
    input_schema: Value,
}

#[derive(Clone)]
pub struct ClaudeClient {
    client: Client,
    api_key: String,
}

impl ClaudeClient {
    pub fn new(api_key: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| HostError::Claude(format!("Failed to create client: {}", e)))?;
        
        Ok(Self {
            client,
            api_key: api_key.to_string(),
        })
    }

    fn get_bash_tool() -> Tool {
        // Bash tool implementation following Anthropic specification bash_20250124
        Tool {
            name: "bash".to_string(),
            description: "Execute shell commands in a persistent bash session, allowing system operations, script execution, and command-line automation. Commands are executed in the session directory (/session) and maintain state between calls.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    },
                    "restart": {
                        "type": "boolean",
                        "description": "Set to true to restart the bash session",
                        "default": false
                    }
                },
                "required": ["command"]
            }),
        }
    }
    
    pub async fn complete(
        &self,
        messages: Vec<(String, String)>, // (role, content)
        system_prompt: Option<String>,
    ) -> Result<String> {
        self.complete_with_tools(messages, system_prompt, true).await
    }

    pub async fn complete_with_tools(
        &self,
        messages: Vec<(String, String)>, // (role, content)
        system_prompt: Option<String>,
        enable_tools: bool,
    ) -> Result<String> {
        // Validate inputs before building request
        if messages.is_empty() {
            return Err(HostError::Claude("No messages provided".to_string()));
        }
        
        if self.api_key.is_empty() {
            return Err(HostError::Claude("API key is empty".to_string()));
        }

        let mut conversation_messages = Vec::new();
        
        // Convert messages to Claude format - use consistent array format for tool compatibility
        for (role, content) in messages {
            if content.trim().is_empty() {
                continue;
            }
            
            let claude_role = match role.as_str() {
                "user" | "USER" => "user".to_string(),
                "assistant" | "HOST" => "assistant".to_string(),
                _ => "user".to_string(),
            };
            
            // Always use array format for consistency with tools
            conversation_messages.push(ClaudeMessage {
                role: claude_role,
                content: serde_json::json!([{
                    "type": "text",
                    "text": content.trim()
                }]),
            });
        }
        
        // Ensure we have at least one valid message
        if conversation_messages.is_empty() {
            return Err(HostError::Claude("All messages are empty after filtering".to_string()));
        }

        // Tool execution loop - continue until we get a final text response
        let max_tool_iterations = 10;
        let mut iteration_count = 0;
        
        loop {
            if iteration_count >= max_tool_iterations {
                return Err(HostError::Claude("Too many tool iterations".to_string()));
            }
            iteration_count += 1;

            let tools = if enable_tools { Some(vec![Self::get_bash_tool()]) } else { None };
            
            let request = ClaudeRequest {
                model: "claude-3-5-sonnet-20241022".to_string(),
                max_tokens: 4096,
                messages: conversation_messages.clone(),
                system: system_prompt.clone(),
                tools,
            };
            
            debug!("Sending request to Claude API with {} messages (iteration {})", request.messages.len(), iteration_count);
            
            let response = self.client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&request)
                .send()
                .await
                .map_err(|e| HostError::Claude(format!("Request failed: {}", e)))?;
            
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                return Err(HostError::Claude(format!("API error ({}): {}", status, error_text)));
            }
            
            let claude_response: ClaudeResponse = response
                .json()
                .await
                .map_err(|e| HostError::Claude(format!("Failed to parse response: {}", e)))?;
            
            // Process response content
            let mut response_text = String::new();
            let mut tool_calls = Vec::new();
            
            for content in &claude_response.content {
                match content {
                    ClaudeContent::Text { text } => {
                        response_text.push_str(text);
                    }
                    ClaudeContent::ToolUse { id, name, input } => {
                        tool_calls.push((id.clone(), name.clone(), input.clone()));
                    }
                }
            }
            
            // If no tool calls, return the text response
            if tool_calls.is_empty() {
                info!("Received final response from Claude (length: {})", response_text.len());
                return Ok(response_text);
            }
            
            // Add assistant message with tool calls to conversation
            if !response_text.is_empty() || !tool_calls.is_empty() {
                let mut assistant_content = Vec::new();
                
                if !response_text.is_empty() {
                    assistant_content.push(serde_json::json!({
                        "type": "text",
                        "text": response_text
                    }));
                }
                
                for (id, name, input) in &tool_calls {
                    assistant_content.push(serde_json::json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": input
                    }));
                }
                
                conversation_messages.push(ClaudeMessage {
                    role: "assistant".to_string(),
                    content: serde_json::Value::Array(assistant_content),
                });
            }
            
            // Execute tool calls and add results to conversation
            for (tool_id, tool_name, tool_input) in tool_calls {
                debug!("Executing tool: {} with id: {}", tool_name, tool_id);
                
                let tool_result = match tool_name.as_str() {
                    "bash" => self.execute_bash_tool(&tool_input).await,
                    _ => Err(HostError::Claude(format!("Unknown tool: {}", tool_name))),
                };
                
                let result_content = match tool_result {
                    Ok(output) => serde_json::json!([{
                        "type": "tool_result",
                        "tool_use_id": tool_id,
                        "content": output
                    }]),
                    Err(e) => serde_json::json!([{
                        "type": "tool_result", 
                        "tool_use_id": tool_id,
                        "content": format!("Error executing tool: {}", e),
                        "is_error": true
                    }]),
                };
                
                conversation_messages.push(ClaudeMessage {
                    role: "user".to_string(),
                    content: result_content,
                });
            }
        }
    }

    async fn execute_bash_tool(&self, input: &Value) -> Result<String> {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::Claude("Missing or invalid 'command' parameter for bash tool".to_string()))?;

        let restart = input
            .get("restart")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        info!("Executing bash command: {} (restart: {})", command, restart);

        // Handle session restart if requested
        if restart {
            info!("Bash session restart requested - this is handled automatically per command");
            // Note: Our current implementation executes each command in a fresh process,
            // so restart doesn't change behavior. In a persistent shell implementation,
            // this would reset the shell state.
        }

        // Security validation
        self.validate_bash_command(command)?;

        // Execute the command in the session directory
        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir("/session")
            .output()
            .await
            .map_err(|e| HostError::Claude(format!("Failed to execute bash command: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let result = if output.status.success() {
            if stdout.is_empty() && stderr.is_empty() {
                "Command executed successfully (no output)".to_string()
            } else if stderr.is_empty() {
                stdout.to_string()
            } else {
                format!("{}\n[stderr]: {}", stdout, stderr)
            }
        } else {
            let exit_code = output.status.code().unwrap_or(-1);
            format!("Command failed with exit code {}\n[stdout]: {}\n[stderr]: {}", 
                    exit_code, stdout, stderr)
        };

        info!("Bash command result (length: {})", result.len());
        Ok(result)
    }

    fn validate_bash_command(&self, command: &str) -> Result<()> {
        // Basic security checks - prevent dangerous commands
        let dangerous_patterns = [
            "rm -rf /",
            "rm -rf /*",
            "format",
            "mkfs",
            "dd if=",
            ":(){ :|:& };:", // Fork bomb
            "chmod 777 /",
            "chown root /",
            "sudo rm",
            "killall -9",
            "reboot",
            "shutdown",
            "halt",
            "poweroff",
        ];

        for pattern in &dangerous_patterns {
            if command.contains(pattern) {
                return Err(HostError::Claude(format!("Command blocked: contains dangerous pattern '{}'", pattern)));
            }
        }

        // Prevent accessing sensitive system directories
        let restricted_paths = [
            "/etc/passwd",
            "/etc/shadow",
            "/root",
            "/boot",
            "/sys",
            "/proc/sys",
            "/dev/sd",
            "/dev/hd",
        ];

        for path in &restricted_paths {
            if command.contains(path) {
                return Err(HostError::Claude(format!("Command blocked: accesses restricted path '{}'", path)));
            }
        }

        // Command length check
        if command.len() > 10000 {
            return Err(HostError::Claude("Command too long".to_string()));
        }

        Ok(())
    }
    
}
