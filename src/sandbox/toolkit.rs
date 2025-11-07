use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::command::{CommandChild, CommandInvocation};
use super::{builtin_tools, environment_tools, package_tools};

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
            "environment_info",
            "python_package",
        ]
    }

    pub fn has(&self, name: &str) -> bool {
        self.known_tools().iter().any(|n| *n == name)
    }

    pub fn command_catalog_prompt(&self) -> String {
        let mut guide = String::from(
            "Command Reference (always respond with ONE of the XML elements below):\n",
        );
        guide.push_str(r#"<run_bash commentary="..." exec_dir="/sandbox/..." commands="..."/>"#);
        guide.push_str(
            "\n  • Execute shell commands. Use `exec_dir=\"/sandbox/...\"` for the working directory.\n",
        );
        guide.push_str(
            r#"<open_file commentary="..." path="/sandbox/..." start_line="optional" end_line="optional"/>"#,
        );
        guide.push_str("\n  • View file contents. Omit start/end for full file.\n");
        guide.push_str(
            r#"<create_file commentary="..." path="/sandbox/...">FILE CONTENT HERE</create_file>"#,
        );
        guide.push_str("\n  • Create a brand new file with the given body.\n");
        guide.push_str(
            r#"<str_replace commentary="..." path="/sandbox/..." many="false">
  <old_str><![CDATA[EXISTING TEXT]]></old_str>
  <new_str><![CDATA[UPDATED TEXT]]></new_str>
</str_replace>"#,
        );
        guide.push_str("\n  • Replace text. Set many=\"true\" to replace all matches.\n");
        guide.push_str(
            r#"<insert commentary="..." path="/sandbox/..." line="42"><![CDATA[TEXT TO INSERT]]></insert>"#,
        );
        guide.push_str("\n  • Insert text at the 1-based line before existing content.\n");
        guide.push_str(
            r#"<remove_str commentary="..." path="/sandbox/..." many="false"><![CDATA[TEXT TO REMOVE]]></remove_str>"#,
        );
        guide.push_str("\n  • Remove text (set many=\"true\" to delete all matches).\n");
        guide.push_str(
            r#"<find_filecontent commentary="..." path="/sandbox/..." regex="pattern"/>"#,
        );
        guide.push_str("\n  • Regex search for matching lines.\n");
        guide
            .push_str(r#"<find_filename commentary="..." path="/sandbox/..." glob="*.rs; *.ts"/>"#);
        guide.push_str("\n  • Glob search for file names.\n");
        guide.push_str(r#"<environment_info info_type="system"/>"#);
        guide.push_str("\n  • Inspect the runtime environment (info_type: system | python_packages | python_version | pip_list | all).\n");
        guide.push_str(
            r#"<python_package action="check" upgrade="false">
  <package>requests</package>
  <package>numpy</package>
</python_package>"#,
        );
        guide.push_str(
            "\n  • Check/install Python packages. Use <package> children for each package.\n",
        );
        guide.push_str(r#"<output><![CDATA[FINAL RESPONSE TO USER]]></output>"#);
        guide.push_str(
            "\n  • Send the final user-facing message. Only use markdown/plain text inside the body.\n",
        );
        guide
    }

    pub async fn execute_invocation(&self, cmd: &CommandInvocation) -> Result<ExecutionResult> {
        let args = self.build_args(cmd)?;
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
            "environment_info" => environment_tools::EnvironmentInfoTool.execute(&args).await,
            "python_package" => package_tools::PythonPackageTool.execute(&args).await,
            other => Err(anyhow::anyhow!("unknown command '{}'", other)),
        }?;
        Ok(ExecutionResult { args, output })
    }

    fn build_args(&self, cmd: &CommandInvocation) -> Result<Value> {
        let mut map = Map::new();
        let attrs = &cmd.attributes;
        let body = cmd.body.clone().unwrap_or_default();
        let child_map = children_to_map(&cmd.children);

        match cmd.name.as_str() {
            "run_bash" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert(
                    "exec_dir".into(),
                    Value::String(
                        attrs
                            .get("exec_dir")
                            .cloned()
                            .unwrap_or_else(|| "/sandbox".to_string()),
                    ),
                );
                map.insert(
                    "commands".into(),
                    Value::String(require_attr(attrs, "commands")?),
                );
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
            "environment_info" => {
                map.insert(
                    "info_type".into(),
                    Value::String(
                        attrs
                            .get("info_type")
                            .cloned()
                            .unwrap_or_else(|| "all".to_string()),
                    ),
                );
            }
            "python_package" => {
                map.insert(
                    "action".into(),
                    Value::String(
                        attrs
                            .get("action")
                            .cloned()
                            .unwrap_or_else(|| "check".to_string()),
                    ),
                );
                if let Some(upgrade) = attrs.get("upgrade") {
                    map.insert("upgrade".into(), Value::Bool(parse_bool(upgrade)?));
                }
                let packages = if let Some(items) = child_map.get("package") {
                    items.clone()
                } else if !body.trim().is_empty() {
                    body.split_whitespace().map(|s| s.to_string()).collect()
                } else {
                    Vec::new()
                };
                map.insert(
                    "packages".into(),
                    Value::Array(packages.into_iter().map(Value::String).collect()),
                );
            }
            other => return Err(anyhow::anyhow!("unknown command '{}'", other)),
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
