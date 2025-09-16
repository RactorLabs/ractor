use anyhow::Result;
use async_trait::async_trait;

use super::api::RaworcClient;
use super::tool_registry::Tool;
use super::tools::{run_bash, text_edit, TextEditAction};
use std::sync::Arc;

/// Built-in bash tool implementation
pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash shell command in the /agent directory"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let cmd = args
            .get("command")
            .and_then(|v| v.as_str())
            .or_else(|| args.get("cmd").and_then(|v| v.as_str()))
            .unwrap_or("");

        match run_bash(cmd).await {
            Ok(output) => {
                // Check if the command actually succeeded by looking for exit_code in output
                if output.contains("[exit_code:") && !output.contains("[exit_code:0]") {
                    let summary =
                        format!("Result for tool 'bash' (command: {})", truncate(cmd, 120));
                    Ok(format!("{}\n{}", summary, output))
                } else {
                    let summary =
                        format!("Result for tool 'bash' (command: {})", truncate(cmd, 120));
                    Ok(format!("{}\n{}", summary, output))
                }
            }
            Err(e) => Ok(format!("Result for tool 'bash' — error: {}", e)),
        }
    }
}

/// Built-in text editor tool implementation
pub struct TextEditorTool;

#[async_trait]
impl Tool for TextEditorTool {
    fn name(&self) -> &str {
        "text_editor"
    }

    fn description(&self) -> &str {
        "Perform text editing operations on files in the /agent directory"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["view", "create", "str_replace", "insert"],
                    "description": "The editing action to perform"
                },
                "path": {
                    "type": "string",
                    "description": "The file path relative to /agent"
                },
                "content": {
                    "type": "string",
                    "description": "Content for create/insert operations"
                },
                "target": {
                    "type": "string",
                    "description": "Text to find for str_replace operation"
                },
                "replacement": {
                    "type": "string",
                    "description": "Replacement text for str_replace operation"
                },
                "line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Line number for insert operation"
                },
                "start_line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Start line for view operation"
                },
                "end_line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "End line for view operation"
                }
            },
            "required": ["action", "path"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let action = parse_text_edit(args)?;

        match text_edit(action.clone()).await {
            Ok(output) => {
                let summary = match &action {
                    TextEditAction::View { path, .. } => {
                        format!("Result for tool 'text_editor' (view {} )", path)
                    }
                    TextEditAction::Create { path, .. } => {
                        format!("Result for tool 'text_editor' (create {} )", path)
                    }
                    TextEditAction::StrReplace { path, .. } => {
                        format!("Result for tool 'text_editor' (str_replace {} )", path)
                    }
                    TextEditAction::Insert { path, line, .. } => format!(
                        "Result for tool 'text_editor' (insert {} at line {} )",
                        path, line
                    ),
                };
                Ok(format!("{}\n{}", summary, output))
            }
            Err(e) => Ok(format!("Result for tool 'text_editor' — error: {}", e)),
        }
    }
}

/// Publish tool (no confirmation required)
pub struct PublishTool {
    api: Arc<RaworcClient>,
}

impl PublishTool {
    pub fn new(api: Arc<RaworcClient>) -> Self {
        Self { api }
    }
}

#[async_trait]
impl Tool for PublishTool {
    fn name(&self) -> &str {
        "publish"
    }

    fn description(&self) -> &str {
        "Publish the agent's current content to its public URL."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "note": { "type": "string", "description": "Optional reason or note" }
            }
        })
    }

    async fn execute(&self, _args: &serde_json::Value) -> Result<String> {
        match self.api.publish_agent().await {
            Ok(_) => Ok("Publish request submitted successfully.".to_string()),
            Err(e) => Ok(format!("Failed to publish: {}", e)),
        }
    }
}

/// Sleep tool (explicit user confirmation required)
pub struct SleepTool {
    api: Arc<RaworcClient>,
}

impl SleepTool {
    pub fn new(api: Arc<RaworcClient>) -> Self {
        Self { api }
    }
}

#[async_trait]
impl Tool for SleepTool {
    fn name(&self) -> &str {
        "sleep"
    }

    fn description(&self) -> &str {
        "Put the agent to sleep (stops its runtime but preserves data)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "note": { "type": "string", "description": "Optional reason or note" }
            }
        })
    }

    async fn execute(&self, _args: &serde_json::Value) -> Result<String> {
        match self.api.sleep_agent().await {
            Ok(_) => Ok("Sleep request submitted successfully.".to_string()),
            Err(e) => Ok(format!("Failed to sleep: {}", e)),
        }
    }
}

/// Parse text edit arguments into TextEditAction
fn parse_text_edit(input: &serde_json::Value) -> anyhow::Result<TextEditAction> {
    // Normalize common alias keys before deserializing to the enum
    let mut v = input.clone();
    if let Some(obj) = v.as_object_mut() {
        // Accept "file" or "file_path" as alias for "path"
        if !obj.contains_key("path") {
            if let Some(p) = obj
                .get("file")
                .cloned()
                .or_else(|| obj.get("file_path").cloned())
            {
                obj.insert("path".to_string(), p);
            }
        }
    }
    let action: TextEditAction = serde_json::from_value(v)?;
    Ok(action)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bash_tool_parameters() {
        let tool = BashTool;
        let params = tool.parameters();

        assert_eq!(tool.name(), "bash");
        assert!(params["properties"]["command"]["type"].as_str() == Some("string"));
        assert!(params["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("command".to_string())));
    }

    #[tokio::test]
    async fn test_text_editor_tool_parameters() {
        let tool = TextEditorTool;
        let params = tool.parameters();

        assert_eq!(tool.name(), "text_editor");
        assert!(params["properties"]["action"]["enum"].as_array().is_some());
        assert!(params["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("action".to_string())));
        assert!(params["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("path".to_string())));
    }

    #[test]
    fn test_parse_text_edit_with_alias() {
        let input = serde_json::json!({
            "action": "view",
            "file": "test.txt"  // Using "file" alias instead of "path"
        });

        let result = parse_text_edit(&input);
        assert!(result.is_ok());

        match result.unwrap() {
            TextEditAction::View { path, .. } => {
                assert_eq!(path, "test.txt");
            }
            _ => panic!("Expected View action"),
        }
    }
}
