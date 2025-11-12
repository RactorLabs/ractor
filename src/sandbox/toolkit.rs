use anyhow::{Error as AnyError, Result};
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::builtin_tools;
use super::command::{CommandChild, CommandInvocation};

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, args: &Value) -> Result<Value>;
}

pub struct ExecutionResult {
    pub args: Value,
    pub output: Value,
}

pub struct ExecutionError {
    pub args: Value,
    pub message: String,
}

impl ExecutionError {
    fn from_error(args: Value, err: AnyError) -> Self {
        Self {
            args,
            message: err.to_string(),
        }
    }
}

pub struct ToolCatalog;

impl ToolCatalog {
    pub fn new() -> Self {
        Self
    }

    pub fn known_tools(&self) -> Vec<&'static str> {
        vec![
            "run_bash",
            "open_file",
            "create_file",
            "str_replace",
            "insert",
            "remove_str",
            "find_filecontent",
            "find_filename",
            "output",
        ]
    }

    pub fn has(&self, name: &str) -> bool {
        self.known_tools().iter().any(|n| *n == name)
    }

    pub fn command_catalog_prompt(&self) -> String {
        let mut guide = String::from("Tool Reference\n\n");
        guide.push_str("Use exactly one tool call per response until the task is complete. Prefer the purpose-built tools below over improvising with shell commands. Key details:\n");
        guide.push_str("- Paths must stay under `/sandbox` unless the user explicitly directs otherwise.\n");
        guide.push_str("- Always validate file or directory existence before operating on them.\n");
        guide.push_str("- Surface tool failures back to the user so the orchestrating model can react.\n\n");

        guide.push_str("### Tool: run_bash\n");
        guide.push_str(r#"Example: <run_bash commentary="Listing project" exec_dir="/sandbox" commands="echo 'Listing project'; ls -lah"/>"#);
        guide.push_str("\n- Execute a single shell command sequence. Echo the planned action first so the log is self-describing.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Short description of the action (no ellipses).\n");
        guide.push_str("  - `exec_dir` (required): Directory to run the command in; must be `/sandbox` or a child directory.\n");
        guide.push_str("  - `commands` (required): The shell command(s). Chain with `&&` only when each step depends on the previous result.\n");
        guide.push_str("- On failure, capture the exit code and last 20 stderr lines, then suggest a revised plan or retry with corrected parameters.\n\n");

        guide.push_str("### Tool: open_file\n");
        guide.push_str(r#"Example: <open_file commentary="Reading file" path="/sandbox/src/main.rs" start_line="1" end_line="40"/>"#);
        guide.push_str("\n- Read file contents for inspection. Use before editing so you never assume state.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Why you’re reading the file.\n");
        guide.push_str("  - `path` (required): Absolute path to the file (must be under `/sandbox`).\n");
        guide.push_str("  - `start_line` / `end_line` (optional): Limit output to a specific range (1-based, inclusive).\n\n");

        guide.push_str("### Tool: create_file\n");
        guide.push_str(r#"Example: <create_file commentary="Creating README" path="/sandbox/docs/readme.md"># Project Notes</create_file>"#);
        guide.push_str("\n- Create a brand-new file with the supplied body. Only use when the user explicitly requests a new file.\n");
        guide.push_str("- Before calling, confirm the parent directory exists; create directories only if the user asked you to.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Reason for creating the file.\n");
        guide.push_str("  - `path` (required): Absolute file path (under `/sandbox`).\n");
        guide.push_str("  - Body (CDATA): The exact file contents.\n\n");

        guide.push_str("### Tool: str_replace\n");
        guide.push_str(r#"Example: <str_replace commentary="Updating config" path="/sandbox/app/config.toml" many="false">
  <old_str><![CDATA[MODE = "dev"]]></old_str>
  <new_str><![CDATA[MODE = "prod"]]></new_str>
</str_replace>"#);
        guide.push_str("\n- Replace existing text with new content.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Why you’re replacing text.\n");
        guide.push_str("  - `path` (required): File to modify (under `/sandbox`).\n");
        guide.push_str("  - `many` (optional, defaults to `false`): Set to `true` to replace every occurrence.\n");
        guide.push_str("  - `<old_str>` / `<new_str>` (required child elements): The original and replacement strings.\n\n");

        guide.push_str("### Tool: insert\n");
        guide.push_str(r#"Example: <insert commentary="Adding import" path="/sandbox/app/main.rs" line="5"><![CDATA[use anyhow::Result;]]></insert>"#);
        guide.push_str("\n- Insert text before the specified 1-based line number.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Why you’re inserting text.\n");
        guide.push_str("  - `path` (required): File path (under `/sandbox`).\n");
        guide.push_str("  - `line` (required): 1-based line number to insert before.\n");
        guide.push_str("  - Body (CDATA): The content to insert.\n\n");

        guide.push_str("### Tool: remove_str\n");
        guide.push_str(r#"Example: <remove_str commentary="Removing debug log" path="/sandbox/app/main.rs" many="false"><![CDATA[println!(\"debug\");]]></remove_str>"#);
        guide.push_str("\n- Remove matching text snippets.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Why you’re removing text.\n");
        guide.push_str("  - `path` (required): File path (under `/sandbox`).\n");
        guide.push_str("  - `many` (optional, defaults to `false`): Set to `true` to remove all occurrences.\n");
        guide.push_str("  - Body (CDATA): The exact text to remove.\n\n");

        guide.push_str("### Tool: find_filecontent\n");
        guide.push_str(r#"Example: <find_filecontent commentary="Searching for TODOs" path="/sandbox/src" regex="TODO"/>"#);
        guide.push_str("\n- Search inside files for a regex pattern and return matching lines (Rust-style regex syntax).\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Purpose of the search.\n");
        guide.push_str("  - `path` (required): Root directory or file path (under `/sandbox`). Narrow the path when possible.\n");
        guide.push_str("  - `regex` (required): Pattern to match.\n\n");

        guide.push_str("### Tool: find_filename\n");
        guide.push_str(r#"Example: <find_filename commentary="Finding tests" path="/sandbox" glob="tests/**/*.rs; **/*_test.rs"/>"#);
        guide.push_str("\n- Locate files using glob patterns (semicolon-separated for multiple patterns). Searches recursively from `path`.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Why you’re searching.\n");
        guide.push_str("  - `path` (required): Directory to search (under `/sandbox`).\n");
        guide.push_str("  - `glob` (required): Glob pattern(s) to match file names.\n\n");

        guide.push_str("### Tool: output\n");
        guide.push_str(r#"Example: <output><![CDATA[All changes applied. Tests pass.]]></output>"#);
        guide.push_str("\n- Send the final user-facing message once the task is complete.\n");
        guide.push_str("- Body (CDATA) should contain plain text or markdown summarizing the outcome and next steps. Do not include additional tool calls or XML.\n");
        guide
    }

    pub async fn execute_invocation(
        &self,
        cmd: &CommandInvocation,
    ) -> std::result::Result<ExecutionResult, ExecutionError> {
        let args = match self.build_args(cmd) {
            Ok(value) => value,
            Err(err) => return Err(ExecutionError::from_error(Value::Null, err)),
        };
        let output = match cmd.name.as_str() {
            "run_bash" => builtin_tools::ShellTool::new().execute(&args).await,
            "open_file" => builtin_tools::OpenFileTool.execute(&args).await,
            "create_file" => builtin_tools::CreateFileTool.execute(&args).await,
            "str_replace" => builtin_tools::StrReplaceTool.execute(&args).await,
            "insert" => builtin_tools::InsertTool.execute(&args).await,
            "remove_str" => builtin_tools::RemoveStrTool.execute(&args).await,
            "find_filecontent" => builtin_tools::FindFilecontentTool.execute(&args).await,
            "find_filename" => builtin_tools::FindFilenameTool.execute(&args).await,
            "output" => builtin_tools::OutputTool.execute(&args).await,
            other => Err(anyhow::anyhow!("unknown tool '{}'", other)),
        };
        match output {
            Ok(output) => Ok(ExecutionResult { args, output }),
            Err(err) => Err(ExecutionError::from_error(args, err)),
        }
    }

    fn build_args(&self, cmd: &CommandInvocation) -> Result<Value> {
        let mut map = Map::new();
        let attrs = &cmd.attributes;
        let body = cmd.body.clone().unwrap_or_default();
        let child_map = children_to_map(&cmd.children);

        match cmd.name.as_str() {
            "run_bash" => {
                let commentary = require_attr(attrs, "commentary")?;
                let exec_dir = attrs
                    .get("exec_dir")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "/sandbox".to_string());
                if !exec_dir.starts_with("/sandbox") {
                    return Err(anyhow::anyhow!(
                        "run_bash exec_dir must start with /sandbox"
                    ));
                }
                let commands = require_attr(attrs, "commands")?;
                map.insert("commentary".into(), Value::String(commentary));
                map.insert("exec_dir".into(), Value::String(exec_dir));
                map.insert("commands".into(), Value::String(commands));
            }
            "open_file" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                if let Some(start) = attrs.get("start_line") {
                    map.insert("start_line".into(), Value::Number(parse_u64(start)?.into()));
                }
                if let Some(end) = attrs.get("end_line") {
                    map.insert("end_line".into(), Value::Number(parse_u64(end)?.into()));
                }
            }
            "create_file" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                map.insert("content".into(), Value::String(body));
            }
            "str_replace" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                let old_str = require_child(&child_map, "old_str")?;
                let new_str = require_child(&child_map, "new_str")?;
                map.insert("old_str".into(), Value::String(old_str));
                map.insert("new_str".into(), Value::String(new_str));
                if let Some(many) = attrs.get("many") {
                    map.insert("many".into(), Value::Bool(parse_bool(many)?));
                }
            }
            "insert" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                let line = require_attr(attrs, "line")?;
                map.insert(
                    "insert_line".into(),
                    Value::Number(parse_u64(&line)?.into()),
                );
                map.insert("content".into(), Value::String(body));
            }
            "remove_str" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                map.insert("content".into(), Value::String(body));
                if let Some(many) = attrs.get("many") {
                    map.insert("many".into(), Value::Bool(parse_bool(many)?));
                }
            }
            "find_filecontent" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                map.insert("regex".into(), Value::String(require_attr(attrs, "regex")?));
            }
            "find_filename" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                map.insert("glob".into(), Value::String(require_attr(attrs, "glob")?));
            }
            "output" => {
                map.insert("commentary".into(), Value::String("final response".into()));
                map.insert(
                    "content".into(),
                    Value::Array(vec![json!({
                        "type": "markdown",
                        "title": "Result",
                        "content": body
                    })]),
                );
            }
            other => return Err(anyhow::anyhow!("unknown tool '{}'", other)),
        }

        Ok(Value::Object(map))
    }
}

fn require_attr(attrs: &HashMap<String, String>, key: &str) -> Result<String> {
    attrs
        .get(key)
        .cloned()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing '{}' attribute", key))
}

fn parse_u64(value: &str) -> Result<u64> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| anyhow::anyhow!("invalid integer '{}'", value))
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        other => Err(anyhow::anyhow!("invalid boolean '{}'", other)),
    }
}

fn children_to_map(children: &[CommandChild]) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for child in children {
        map.entry(child.name.to_string())
            .or_default()
            .push(child.content.clone());
    }
    map
}

fn require_child(children: &HashMap<String, Vec<String>>, key: &str) -> Result<String> {
    children
        .get(key)
        .and_then(|vals| vals.first().cloned())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing <{}>...</{}> block", key, key))
}
