use super::api::RaworcClient;
use super::error::{HostError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, warn};

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
    api_client: Option<Arc<RaworcClient>>,
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
            api_client: None,
        })
    }

    pub fn set_api_client(&mut self, api_client: Arc<RaworcClient>) {
        self.api_client = Some(api_client);
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

    fn get_text_editor_tool() -> Tool {
        // Text editor tool implementation following Anthropic specification text_editor_20250728
        Tool {
            name: "text_editor".to_string(),
            description: "Edit text files by viewing, creating, and making targeted changes. Supports viewing file contents, creating new files, and making precise edits using string replacement.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "enum": ["view", "create", "str_replace", "insert"],
                        "description": "The editing command to execute"
                    },
                    "path": {
                        "type": "string",
                        "description": "Path to the file (relative to /session/)"
                    },
                    "file_text": {
                        "type": "string",
                        "description": "Content for new file (create command only)"
                    },
                    "old_str": {
                        "type": "string",
                        "description": "Text to replace (str_replace command only)"
                    },
                    "new_str": {
                        "type": "string",
                        "description": "Replacement text (str_replace and insert commands)"
                    },
                    "insert_line": {
                        "type": "integer",
                        "description": "Line number to insert at (insert command only)"
                    },
                    "view_range": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "minItems": 2,
                        "maxItems": 2,
                        "description": "Line range [start, end] for viewing (view command only)"
                    },
                    "max_characters": {
                        "type": "integer",
                        "description": "Maximum number of characters to display in view command output"
                    }
                },
                "required": ["command", "path"]
            }),
        }
    }

    fn get_web_search_tool() -> Tool {
        // Web search tool implementation following Anthropic specification web_search_20250305
        Tool {
            name: "web_search".to_string(),
            description: "Search the web for real-time information beyond my knowledge cutoff. Automatically provides citations and sources.".to_string(),
            input_schema: serde_json::json!({
                "type": "web_search_20250305",
                "name": "web_search",
                "max_uses": 10
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

            let tools = if enable_tools { 
                Some(vec![
                    Self::get_bash_tool(), 
                    Self::get_text_editor_tool(),
                    Self::get_web_search_tool()
                ]) 
            } else { 
                None 
            };
            
            let request = ClaudeRequest {
                model: "claude-sonnet-4-20250514".to_string(),
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
                
                // Send tool execution notification to user
                let tool_description = match tool_name.as_str() {
                    "bash" => {
                        let cmd = tool_input.get("command").and_then(|v| v.as_str()).unwrap_or("unknown");
                        format!("Executing bash command: {}", cmd)
                    },
                    "text_editor" => {
                        let cmd = tool_input.get("command").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let path = tool_input.get("path").and_then(|v| v.as_str()).unwrap_or("unknown");
                        format!("Text editor {}: {}", cmd, path)
                    },
                    "web_search" => "Searching the web for current information".to_string(),
                    _ => format!("Executing {} tool", tool_name),
                };

                self.send_tool_message(&tool_description, &tool_name).await?;
                
                let tool_result = match tool_name.as_str() {
                    "bash" => self.execute_bash_tool(&tool_input).await,
                    "text_editor" => self.execute_text_editor_tool(&tool_input).await,
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

        // Ensure logs directory exists
        if let Err(e) = tokio::fs::create_dir_all("/session/logs").await {
            warn!("Failed to create logs directory: {}", e);
        }

        // Execute the command in the session directory
        let start_time = std::time::SystemTime::now();
        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir("/session")
            .output()
            .await
            .map_err(|e| HostError::Claude(format!("Failed to execute bash command: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);
        let success = output.status.success();

        let result = if success {
            if stdout.is_empty() && stderr.is_empty() {
                "Command executed successfully (no output)".to_string()
            } else if stderr.is_empty() {
                stdout.to_string()
            } else {
                format!("{}\n[stderr]: {}", stdout, stderr)
            }
        } else {
            format!("Command failed with exit code {}\n[stdout]: {}\n[stderr]: {}", 
                    exit_code, stdout, stderr)
        };

        // Log to Docker logs with structured format
        println!("BASH_EXECUTION: command={:?} exit_code={} success={} output_length={}", 
                command, exit_code, success, result.len());

        // Save individual log file
        self.save_bash_log(command, &stdout, &stderr, exit_code, success, start_time).await;

        info!("Bash command result (length: {})", result.len());
        Ok(result)
    }

    async fn save_bash_log(&self, command: &str, stdout: &str, stderr: &str, exit_code: i32, success: bool, start_time: std::time::SystemTime) {
        use std::time::UNIX_EPOCH;
        
        let timestamp = start_time.duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        
        let log_filename = format!("/session/logs/bash_{}.log", timestamp);
        
        let log_content = format!(
            "=== BASH COMMAND LOG ===\n\
            Timestamp: {}\n\
            Command: {}\n\
            Exit Code: {}\n\
            Success: {}\n\
            \n\
            === STDOUT ===\n\
            {}\n\
            \n\
            === STDERR ===\n\
            {}\n\
            \n\
            === END LOG ===\n",
            chrono::DateTime::from_timestamp(timestamp as i64, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "Unknown".to_string()),
            command,
            exit_code,
            success,
            stdout,
            stderr
        );

        if let Err(e) = tokio::fs::write(&log_filename, log_content).await {
            warn!("Failed to save bash log to {}: {}", log_filename, e);
        } else {
            debug!("Saved bash log to {}", log_filename);
        }
    }

    async fn save_text_editor_log(&self, command: &str, path: &str, result_msg: &str, success: bool, input: &Value, start_time: std::time::SystemTime) {
        use std::time::UNIX_EPOCH;
        
        let timestamp = start_time.duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        
        let log_filename = format!("/session/logs/text_editor_{}.log", timestamp);
        
        // Extract additional parameters for logging
        let mut params = Vec::new();
        if let Some(old_str) = input.get("old_str").and_then(|v| v.as_str()) {
            params.push(format!("old_str: {:?}", old_str));
        }
        if let Some(new_str) = input.get("new_str").and_then(|v| v.as_str()) {
            params.push(format!("new_str: {:?}", new_str));
        }
        if let Some(file_text) = input.get("file_text").and_then(|v| v.as_str()) {
            params.push(format!("file_text_length: {}", file_text.len()));
        }
        if let Some(insert_line) = input.get("insert_line").and_then(|v| v.as_u64()) {
            params.push(format!("insert_line: {}", insert_line));
        }
        if let Some(view_range) = input.get("view_range") {
            params.push(format!("view_range: {:?}", view_range));
        }
        if let Some(max_chars) = input.get("max_characters").and_then(|v| v.as_u64()) {
            params.push(format!("max_characters: {}", max_chars));
        }

        let log_content = format!(
            "=== TEXT EDITOR COMMAND LOG ===\n\
            Timestamp: {}\n\
            Command: {}\n\
            Path: {}\n\
            Success: {}\n\
            Parameters: {}\n\
            \n\
            === RESULT ===\n\
            {}\n\
            \n\
            === END LOG ===\n",
            chrono::DateTime::from_timestamp(timestamp as i64, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "Unknown".to_string()),
            command,
            path,
            success,
            if params.is_empty() { "None".to_string() } else { params.join(", ") },
            result_msg
        );

        if let Err(e) = tokio::fs::write(&log_filename, log_content).await {
            warn!("Failed to save text_editor log to {}: {}", log_filename, e);
        } else {
            debug!("Saved text_editor log to {}", log_filename);
        }
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

    async fn execute_text_editor_tool(&self, input: &Value) -> Result<String> {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::Claude("Missing or invalid 'command' parameter for text_editor tool".to_string()))?;

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::Claude("Missing or invalid 'path' parameter for text_editor tool".to_string()))?;

        // Validate and normalize path (must be within /session/)
        let full_path = self.validate_and_normalize_path(path)?;

        info!("Executing text_editor command: {} on path: {}", command, full_path.display());

        // Ensure logs directory exists
        if let Err(e) = tokio::fs::create_dir_all("/session/logs").await {
            warn!("Failed to create logs directory: {}", e);
        }

        let start_time = std::time::SystemTime::now();
        let result = match command {
            "view" => self.text_editor_view(&full_path, input).await,
            "create" => self.text_editor_create(&full_path, input).await,
            "str_replace" => self.text_editor_str_replace(&full_path, input).await,
            "insert" => self.text_editor_insert(&full_path, input).await,
            _ => Err(HostError::Claude(format!("Unknown text_editor command: {}", command))),
        };

        // Log the text editor operation
        let success = result.is_ok();
        let result_msg = match &result {
            Ok(msg) => msg.clone(),
            Err(e) => e.to_string(),
        };

        // Log to Docker logs with structured format
        println!("TEXT_EDITOR_EXECUTION: command={:?} path={:?} success={} result_length={}", 
                command, path, success, result_msg.len());

        // Save individual log file
        self.save_text_editor_log(command, path, &result_msg, success, input, start_time).await;

        result
    }

    fn validate_and_normalize_path(&self, path: &str) -> Result<std::path::PathBuf> {
        use std::path::Path;

        // Security: prevent path traversal attacks
        if path.contains("..") || path.starts_with('/') {
            return Err(HostError::Claude(format!("Invalid path: path traversal not allowed ({})", path)));
        }

        // Normalize path relative to /session/
        let session_root = Path::new("/session");
        let full_path = session_root.join(path);

        // Ensure the resolved path is still within /session/
        if !full_path.starts_with(session_root) {
            return Err(HostError::Claude(format!("Invalid path: must be within /session/ ({})", path)));
        }

        Ok(full_path)
    }

    async fn text_editor_view(&self, path: &std::path::Path, input: &Value) -> Result<String> {
        use tokio::fs;

        // Get max_characters parameter if specified
        let max_characters = input.get("max_characters")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        // Check if path is a directory
        if path.is_dir() {
            // List directory contents
            let mut entries = fs::read_dir(path).await
                .map_err(|e| HostError::Claude(format!("Failed to read directory {}: {}", path.display(), e)))?;
            
            let mut items = Vec::new();
            while let Some(entry) = entries.next_entry().await
                .map_err(|e| HostError::Claude(format!("Failed to read directory entry: {}", e)))? {
                let name = entry.file_name().to_string_lossy().to_string();
                let file_type = if entry.file_type().await.map_err(|e| HostError::Claude(format!("Failed to get file type: {}", e)))?.is_dir() {
                    "directory"
                } else {
                    "file"
                };
                items.push(format!("{} ({})", name, file_type));
            }

            if items.is_empty() {
                return Ok(format!("Directory {} is empty", path.display()));
            }

            items.sort();
            let result = format!("Directory contents of {}:\n{}", path.display(), items.join("\n"));
            
            // Apply max_characters truncation if specified
            if let Some(max_chars) = max_characters {
                if result.len() > max_chars {
                    return Ok(format!("{}...\n[Output truncated at {} characters]", &result[..max_chars], max_chars));
                }
            }
            
            return Ok(result);
        }

        // Read file contents
        let content = fs::read_to_string(path).await
            .map_err(|e| HostError::Claude(format!("Failed to read file {}: {}", path.display(), e)))?;

        // Handle view_range if specified
        if let Some(view_range) = input.get("view_range") {
            if let Some(range_array) = view_range.as_array() {
                if range_array.len() == 2 {
                    let start = range_array[0].as_u64().unwrap_or(1) as usize;
                    let end = range_array[1].as_u64().unwrap_or(1) as usize;
                    
                    let lines: Vec<&str> = content.lines().collect();
                    let start_idx = start.saturating_sub(1);
                    let end_idx = std::cmp::min(end, lines.len());
                    
                    if start_idx < lines.len() {
                        let selected_lines = &lines[start_idx..end_idx];
                        let numbered_lines: Vec<String> = selected_lines.iter()
                            .enumerate()
                            .map(|(i, line)| format!("{:4}: {}", start_idx + i + 1, line))
                            .collect();
                        let result = format!("File: {} (lines {}-{})\n{}", path.display(), start, end_idx, numbered_lines.join("\n"));
                        
                        // Apply max_characters truncation if specified
                        if let Some(max_chars) = max_characters {
                            if result.len() > max_chars {
                                return Ok(format!("{}...\n[Output truncated at {} characters]", &result[..max_chars], max_chars));
                            }
                        }
                        
                        return Ok(result);
                    } else {
                        return Ok(format!("File: {} - line range {}-{} is beyond file length ({})", path.display(), start, end, lines.len()));
                    }
                }
            }
        }

        // Return full file with line numbers
        let lines: Vec<&str> = content.lines().collect();
        let numbered_lines: Vec<String> = lines.iter()
            .enumerate()
            .map(|(i, line)| format!("{:4}: {}", i + 1, line))
            .collect();
        
        let result = format!("File: {} ({} lines)\n{}", path.display(), lines.len(), numbered_lines.join("\n"));
        
        // Apply max_characters truncation if specified
        if let Some(max_chars) = max_characters {
            if result.len() > max_chars {
                return Ok(format!("{}...\n[Output truncated at {} characters]", &result[..max_chars], max_chars));
            }
        }
        
        Ok(result)
    }

    async fn text_editor_create(&self, path: &std::path::Path, input: &Value) -> Result<String> {
        use tokio::fs;

        let file_text = input
            .get("file_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::Claude("Missing 'file_text' parameter for create command".to_string()))?;

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| HostError::Claude(format!("Failed to create parent directories for {}: {}", path.display(), e)))?;
        }

        // Check if file already exists
        if path.exists() {
            return Err(HostError::Claude(format!("File {} already exists. Use str_replace to modify existing files.", path.display())));
        }

        // Write the file
        fs::write(path, file_text).await
            .map_err(|e| HostError::Claude(format!("Failed to create file {}: {}", path.display(), e)))?;

        let line_count = file_text.lines().count();
        Ok(format!("Created file {} with {} lines", path.display(), line_count))
    }

    async fn text_editor_str_replace(&self, path: &std::path::Path, input: &Value) -> Result<String> {
        use tokio::fs;

        let old_str = input
            .get("old_str")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::Claude("Missing 'old_str' parameter for str_replace command".to_string()))?;

        let new_str = input
            .get("new_str")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::Claude("Missing 'new_str' parameter for str_replace command".to_string()))?;

        // Read current file content
        let content = fs::read_to_string(path).await
            .map_err(|e| HostError::Claude(format!("Failed to read file {}: {}", path.display(), e)))?;

        // Check if old_str exists in the file
        if !content.contains(old_str) {
            return Err(HostError::Claude(format!("String not found in file {}: {:?}", path.display(), old_str)));
        }

        // Check for multiple occurrences
        let occurrences = content.matches(old_str).count();
        if occurrences > 1 {
            return Err(HostError::Claude(format!("Multiple occurrences ({}) of string found in file {}. Please use a more specific string to ensure unique replacement.", occurrences, path.display())));
        }

        // Perform replacement
        let new_content = content.replace(old_str, new_str);

        // Write the modified content back
        fs::write(path, &new_content).await
            .map_err(|e| HostError::Claude(format!("Failed to write modified file {}: {}", path.display(), e)))?;

        let old_lines = content.lines().count();
        let new_lines = new_content.lines().count();
        Ok(format!("Replaced 1 occurrence in {} (lines: {} -> {})", path.display(), old_lines, new_lines))
    }

    async fn text_editor_insert(&self, path: &std::path::Path, input: &Value) -> Result<String> {
        use tokio::fs;

        let insert_line = input
            .get("insert_line")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| HostError::Claude("Missing or invalid 'insert_line' parameter for insert command".to_string()))? as usize;

        let new_str = input
            .get("new_str")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::Claude("Missing 'new_str' parameter for insert command".to_string()))?;

        // Read current file content
        let content = fs::read_to_string(path).await
            .map_err(|e| HostError::Claude(format!("Failed to read file {}: {}", path.display(), e)))?;

        let mut lines: Vec<&str> = content.lines().collect();
        
        // Validate insert position
        if insert_line == 0 || insert_line > lines.len() + 1 {
            return Err(HostError::Claude(format!("Invalid insert line {}. File has {} lines (valid range: 1-{})", insert_line, lines.len(), lines.len() + 1)));
        }

        // Insert new content at the specified line (1-based)
        let insert_idx = insert_line - 1;
        lines.insert(insert_idx, new_str);

        // Reconstruct file content
        let new_content = lines.join("\n");

        // Write the modified content back
        fs::write(path, &new_content).await
            .map_err(|e| HostError::Claude(format!("Failed to write modified file {}: {}", path.display(), e)))?;

        let old_line_count = content.lines().count();
        let new_line_count = new_content.lines().count();
        Ok(format!("Inserted text at line {} in {} (lines: {} -> {})", insert_line, path.display(), old_line_count, new_line_count))
    }

    async fn send_tool_message(&self, description: &str, tool_name: &str) -> Result<()> {
        // Log tool execution notification to Docker logs
        println!("TOOL_EXECUTION: {}: {}", tool_name, description);

        // Send tool execution message to user if api client is available
        if let Some(api_client) = &self.api_client {
            let metadata = serde_json::json!({
                "type": "tool_execution",
                "tool_type": tool_name
            });
            
            api_client.send_message(description.to_string(), Some(metadata)).await?;
        }
        
        Ok(())
    }
    
}
