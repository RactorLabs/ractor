use anyhow::Result;
use async_trait::async_trait;

use super::api::RaworcClient;
use super::tool_registry::Tool;
use super::tools::{run_bash, text_edit, TextEditAction};
use anyhow::anyhow;
use globset::{GlobBuilder, GlobSetBuilder};
use regex::Regex;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use walkdir::WalkDir;

const AGENT_ROOT: &str = "/agent";

fn ensure_under_agent(path: &str) -> anyhow::Result<&Path> {
    let p = Path::new(path);
    if !p.starts_with(AGENT_ROOT) {
        return Err(anyhow!(format!("path must be under {}", AGENT_ROOT)));
    }
    Ok(p)
}

fn to_rel_under_agent(path: &str) -> anyhow::Result<String> {
    let p = ensure_under_agent(path)?;
    let rel = p.strip_prefix(AGENT_ROOT).unwrap_or(p);
    let s = rel.to_string_lossy();
    let s = s.strip_prefix('/').unwrap_or(&s).to_string();
    Ok(s)
}

/// Shell tool — simplified one-shot execution

pub struct ShellTool;

impl ShellTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "run_bash"
    }

    fn description(&self) -> &str {
        "Run command(s) in a bash shell and return the output. Long outputs may be truncated and written to a log. Do not use this command to create, view, or edit files — use editor commands instead."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "exec_dir": {"type": "string", "description": "Absolute path to directory where command should be executed"},
                "commands": {"type": "string", "description": "Command(s) to execute. Use && for multi-step."}
            },
            "required": ["exec_dir", "commands"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let exec_dir = args
            .get("exec_dir")
            .and_then(|v| v.as_str())
            .unwrap_or("/agent");
        let commands = args.get("commands").and_then(|v| v.as_str()).unwrap_or("");
        // safety: restrict to /agent
        if !exec_dir.starts_with("/agent") {
            return Ok(
                json!({"status":"error","tool":"run_bash","error":"exec_dir must be under /agent","exec_dir":exec_dir}),
            );
        }
        // emulate working dir via cd then run
        let cmd = format!(
            "export PATH=\"/agent/bin:$PATH\"; cd '{}' && {}",
            exec_dir.replace("'", "'\\''"),
            commands
        );
        match run_bash(&cmd).await {
            Ok(out) => {
                let exit_code = parse_exit_code(&out);
                let truncated = out.contains("[truncated]");
                let clean = strip_exit_marker(&out);
                let (stdout, stderr) = split_stdout_stderr(&clean);
                Ok(json!({
                    "status":"ok","tool":"run_bash",
                    "exit_code": exit_code,
                    "truncated": truncated,
                    "stdout": stdout,
                    "stderr": stderr
                }))
            }
            Err(e) => Ok(json!({"status":"error","tool":"run_bash","error":e.to_string()})),
        }
    }
}

/// Editor: open_file
pub struct OpenFileTool;

#[async_trait]
impl Tool for OpenFileTool {
    fn name(&self) -> &str {
        "open_file"
    }

    fn description(&self) -> &str {
        "Open a file and view its contents. If available, this will also display the file outline obtained from the LSP, any LSP diagnostics, as well as the diff between when you first opened this page and its current state. Long file contents will be truncated to a range of about 500 lines. You can also use this command open and view .png, .jpg, or .gif images. Small files will be shown in full, even if you don't select the full line range. If you provide a start_line but the rest of the file is short, you will be shown the full rest of the file regardless of your end_line."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "path": {"type":"string","description":"Absolute path to the file."},
                "start_line": {"type":"integer","description":"Start line (optional)"},
                "end_line": {"type":"integer","description":"End line (optional)"},
                "sudo": {"type":"boolean","description":"Ignored"}
            },
            "required":["path"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let start_line = args
            .get("start_line")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let end_line = args
            .get("end_line")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let rel = to_rel_under_agent(path).map_err(|e| anyhow::anyhow!(e))?;
        let action = TextEditAction::View {
            path: rel,
            start_line,
            end_line,
        };
        match text_edit(action).await {
            Ok(s) => Ok(json!({
                "status":"ok","tool":"open_file",
                "content": s
            })),
            Err(e) => Ok(json!({"status":"error","tool":"open_file","error":e.to_string()})),
        }
    }
}

/// Editor: create_file
pub struct CreateFileTool;

#[async_trait]
impl Tool for CreateFileTool {
    fn name(&self) -> &str {
        "create_file"
    }

    fn description(&self) -> &str {
        "Use this to create a new file. The content inside the create file tags will be written to the new file exactly as you output it."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "path": {"type":"string","description":"Absolute path to the file. File must not exist yet."},
                "content": {"type":"string","description":"Content of the new file. Don't start with backticks."},
                "sudo": {"type":"boolean","description":"Ignored"}
            },
            "required":["path","content"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let p = ensure_under_agent(path)?;
        if p.exists() {
            return Ok(
                json!({"status":"error","tool":"create_file","error":"file already exists"}),
            );
        }
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).await.ok();
        }
        let mut f = fs::File::create(p).await?;
        f.write_all(content.as_bytes()).await?;
        Ok(json!({"status":"ok","tool":"create_file","bytes":content.len()}))
    }
}

/// Editor: str_replace
pub struct StrReplaceTool;

#[async_trait]
impl Tool for StrReplaceTool {
    fn name(&self) -> &str {
        "str_replace"
    }

    fn description(&self) -> &str {
        "Edits a file by replacing the old string with a new string. The command returns a view of the updated file contents. If available, it will also return the updated outline and diagnostics from the LSP."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "path": {"type":"string","description":"Absolute path to the file"},
                "old_str": {"type":"string","description":"Original text to replace (exact match)"},
                "new_str": {"type":"string","description":"Replacement text"},
                "many": {"type":"boolean","description":"Whether to replace all occurrences (default false)"},
                "sudo": {"type":"boolean","description":"Ignored"}
            },
            "required":["path","old_str","new_str"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let old_str = args.get("old_str").and_then(|v| v.as_str()).unwrap_or("");
        let new_str = args.get("new_str").and_then(|v| v.as_str()).unwrap_or("");
        let many = args.get("many").and_then(|v| v.as_bool()).unwrap_or(false);

        let p = ensure_under_agent(path)?;
        let content = fs::read_to_string(p).await?;
        let count = content.matches(old_str).count();
        if count == 0 {
            return Ok(json!({"status":"error","tool":"str_replace","error":"no matches found"}));
        }
        if !many && count != 1 {
            return Ok(
                json!({"status":"error","tool":"str_replace","error":format!("requires exactly 1 match, found {}", count)}),
            );
        }
        let new_content = if many {
            content.replace(old_str, new_str)
        } else {
            content.replacen(old_str, new_str, 1)
        };
        let mut f = fs::File::create(p).await?;
        f.write_all(new_content.as_bytes()).await?;
        Ok(json!({"status":"ok","tool":"str_replace","replaced": if many {count} else {1}}))
    }
}

/// Editor: insert
pub struct InsertTool;

#[async_trait]
impl Tool for InsertTool {
    fn name(&self) -> &str {
        "insert"
    }

    fn description(&self) -> &str {
        "Inserts a new string in a file at a provided line number."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "path": {"type":"string","description":"Absolute path to the file"},
                "insert_line": {"type":"integer","description":"Line number to insert at (1-based)"},
                "content": {"type":"string","description":"Content to insert"},
                "sudo": {"type":"boolean","description":"Ignored"}
            },
            "required":["path","insert_line","content"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let line = args
            .get("insert_line")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let rel = to_rel_under_agent(path).map_err(|e| anyhow::anyhow!(e))?;
        let action = TextEditAction::Insert {
            path: rel,
            line,
            content: content.to_string(),
        };
        match text_edit(action).await {
            Ok(msg) => Ok(json!({"status":"ok","tool":"insert","result":msg})),
            Err(e) => Ok(json!({"status":"error","tool":"insert","error":e.to_string()})),
        }
    }
}

/// Editor: remove_str
pub struct RemoveStrTool;

#[async_trait]
impl Tool for RemoveStrTool {
    fn name(&self) -> &str {
        "remove_str"
    }

    fn description(&self) -> &str {
        "Deletes the provided string from the file."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "path": {"type":"string","description":"Absolute path to the file"},
                "content": {"type":"string","description":"Exact string to remove (may be multi-line)"},
                "many": {"type":"boolean","description":"Whether to remove all instances (default false)"},
                "sudo": {"type":"boolean","description":"Ignored"}
            },
            "required":["path","content"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let remove = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let many = args.get("many").and_then(|v| v.as_bool()).unwrap_or(false);
        let p = ensure_under_agent(path)?;
        let content = fs::read_to_string(p).await?;
        let count = content.matches(remove).count();
        if count == 0 {
            return Ok(json!({"status":"error","tool":"remove_str","error":"no matches found"}));
        }
        if !many && count != 1 {
            return Ok(
                json!({"status":"error","tool":"remove_str","error":format!("requires exactly 1 match, found {}", count)}),
            );
        }
        let new_content = if many {
            content.replace(remove, "")
        } else {
            content.replacen(remove, "", 1)
        };
        let mut f = fs::File::create(p).await?;
        f.write_all(new_content.as_bytes()).await?;
        Ok(json!({"status":"ok","tool":"remove_str","removed": if many {count} else {1}}))
    }
}

/// Search: find_filecontent
pub struct FindFilecontentTool;

#[async_trait]
impl Tool for FindFilecontentTool {
    fn name(&self) -> &str {
        "find_filecontent"
    }

    fn description(&self) -> &str {
        "Returns file content matches for the provided regex at the given path. The response will cite the files and line numbers of the matches along with some surrounding content. Never use grep but use this command instead since it is optimized for your machine."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "path": {"type":"string","description":"Absolute path to a file or directory"},
                "regex": {"type":"string","description":"Regex to search for"}
            },
            "required":["path","regex"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let root = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let pattern = args.get("regex").and_then(|v| v.as_str()).unwrap_or("");
        let re = Regex::new(pattern).map_err(|e| anyhow::anyhow!(e))?;
        let mut hits = Vec::new();
        let _ = ensure_under_agent(root)?;
        let meta = std::fs::metadata(root);
        if meta.as_ref().map(|m| m.is_file()).unwrap_or(false) {
            scan_file(Path::new(root), &re, &mut hits).await?;
        } else {
            let walker = WalkDir::new(root).into_iter().filter_entry(|e| {
                if e.file_type().is_dir() {
                    let name = e.file_name().to_string_lossy();
                    !matches_default_ignored_dir(&name)
                } else {
                    true
                }
            });
            for entry in walker.filter_map(|e| e.ok()) {
                if entry.file_type().is_file() && !matches_default_ignored_file(entry.path()) {
                    scan_file(entry.path(), &re, &mut hits).await.ok();
                }
            }
        }
        Ok(json!({"status":"ok","tool":"find_filecontent","matches":hits}))
    }
}

async fn scan_file(path: &Path, re: &Regex, out: &mut Vec<String>) -> Result<()> {
    let content = fs::read_to_string(path).await?;
    for (i, line) in content.lines().enumerate() {
        if re.is_match(line) {
            let ctx = line;
            out.push(format!("{}:{}:{}", path.display(), i + 1, ctx));
        }
    }
    Ok(())
}

/// Search: find_filename
pub struct FindFilenameTool;

#[async_trait]
impl Tool for FindFilenameTool {
    fn name(&self) -> &str {
        "find_filename"
    }

    fn description(&self) -> &str {
        "Searches the directory at the specified path recursively for file names matching at least one of the given glob patterns."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "path": {"type":"string","description":"Absolute path of the directory to search in."},
                "glob": {"type":"string","description":"Patterns to search for; separate multiple with '; '"}
            },
            "required":["path","glob"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let root = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let glob_str = args.get("glob").and_then(|v| v.as_str()).unwrap_or("");
        let _ = ensure_under_agent(root)?;
        let mut builder = GlobSetBuilder::new();
        for pat in glob_str
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            let g = GlobBuilder::new(pat).case_insensitive(true).build()?;
            builder.add(g);
        }
        let set = builder.build()?;
        let mut matches = Vec::new();
        let walker = WalkDir::new(root).into_iter().filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                !matches_default_ignored_dir(&name)
            } else {
                true
            }
        });
        for entry in walker.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_file() && !matches_default_ignored_file(p) && set.is_match(p) {
                matches.push(p.display().to_string());
            }
        }
        Ok(json!({"status":"ok","tool":"find_filename","matches":matches}))
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
        "publish_agent"
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

    async fn execute(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
        match self.api.publish_agent().await {
            Ok(_) => Ok(
                json!({"status":"ok","tool":"publish_agent","message":"Publish request submitted"}),
            ),
            Err(e) => Ok(json!({"status":"error","tool":"publish_agent","error":e.to_string()})),
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
        "sleep_agent"
    }

    fn description(&self) -> &str {
        "Schedule the agent to sleep (stop runtime but preserve data) after a short delay. Optionally include a note (shown in chat)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "note": { "type": "string", "description": "Optional reason or note (shown in chat)" },
                "delay_seconds": { "type": "integer", "description": "Delay in seconds before sleeping (min/default 5)" }
            }
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let mut delay = args
            .get("delay_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(5);
        if delay < 5 {
            delay = 5;
        }
        let note = args
            .get("note")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        match self.api.sleep_agent(Some(delay), note.clone()).await {
            Ok(_) => Ok(
                json!({"status":"ok","tool":"sleep_agent","message":"Sleep request submitted","delay_seconds": delay, "note": note}),
            ),
            Err(e) => Ok(json!({"status":"error","tool":"sleep_agent","error":e.to_string()})),
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

fn parse_exit_code(out: &str) -> Option<i32> {
    if let Some(idx) = out.rfind("[exit_code:") {
        let rest = &out[idx + 11..];
        if let Some(end) = rest.find(']') {
            return rest[..end].trim().parse::<i32>().ok();
        }
    }
    None
}

fn strip_exit_marker(out: &str) -> String {
    if let Some(idx) = out.rfind("[exit_code:") {
        let mut s = out[..idx].to_string();
        while s.ends_with(['\n', '\r']) {
            s.pop();
        }
        s
    } else {
        out.to_string()
    }
}

fn split_stdout_stderr(out: &str) -> (String, String) {
    let marker = "[stderr]\n";
    if let Some(pos) = out.find(marker) {
        let stdout = out[..pos].to_string();
        let stderr = out[pos + marker.len()..].to_string();
        (stdout, stderr)
    } else {
        (out.to_string(), String::new())
    }
}

/// Unified Output tool: output
/// Accepts an array of content items; each item must include a type and content.
/// Supported types: "markdown" (content:string), "json" (content:any JSON value)
pub struct OutputTool;

#[async_trait]
impl Tool for OutputTool {
    fn name(&self) -> &str {
        "output"
    }

    fn description(&self) -> &str {
        "Send final user-facing outputs. Accepts an array of items where each item has { type: 'markdown'|'json'|'url', content }. This concludes the current response."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "content": {
                    "type":"array",
                    "description":"List of outputs to present to the user",
                    "items": {
                        "type":"object",
                        "properties":{
                            "type": {"type":"string","enum":["markdown","json","url"],"description":"Output type"},
                            "title": {"type":"string","description":"Title heading for this item (required)"},
                            "content": {"description":"For markdown: string; for json: any JSON value; for url: string (http/https)"}
                        },
                        "required":["type","title","content"]
                    }
                }
            },
            "required":["content"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let items_in = args
            .get("content")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut items_out: Vec<Value> = Vec::new();
        for (idx, it) in items_in.iter().enumerate() {
            let typ = it
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let title = it.get("title").and_then(|v| v.as_str()).ok_or_else(|| {
                anyhow!(format!(
                    "content[{}].title is required and must be string",
                    idx
                ))
            })?;
            match typ.as_str() {
                "markdown" => {
                    let content = it.get("content").and_then(|v| v.as_str()).ok_or_else(|| {
                        anyhow!(format!(
                            "content[{}].content must be string for markdown",
                            idx
                        ))
                    })?;
                    items_out.push(json!({"type":"markdown","title": title, "content": content}));
                }
                "json" => {
                    let content = it.get("content").cloned().unwrap_or(Value::Null);
                    items_out.push(json!({"type":"json","title": title, "content": content}));
                }
                "url" => {
                    let url = it.get("content").and_then(|v| v.as_str()).ok_or_else(|| {
                        anyhow!(format!("content[{}].content must be string for url", idx))
                    })?;
                    let url_trim = url.trim();
                    if !(url_trim.starts_with("http://") || url_trim.starts_with("https://")) {
                        return Ok(json!({
                            "status":"error","tool":"output",
                            "error": format!("invalid url scheme at index {}: must start with http:// or https://", idx)
                        }));
                    }
                    items_out.push(json!({"type":"url","title": title, "content": url_trim}));
                }
                _ => {
                    return Ok(json!({
                        "status":"error",
                        "tool":"output",
                        "error": format!("unsupported type '{}' at index {}", typ, idx),
                        "supported_types": ["markdown","json","url"]
                    }));
                }
            }
        }
        Ok(json!({
            "status":"ok",
            "tool":"output",
            "items": items_out,
            "supported_types": ["markdown","json","url"]
        }))
    }
}

// (ShowAndTellTool removed) — tools may include an optional 'commentary' field in their args.

/// Validation tool: validate_response
/// Checks the current response used the 'output' tool at least once.
pub struct ValidateResponseTool {
    api: Arc<RaworcClient>,
}

impl ValidateResponseTool {
    pub fn new(api: Arc<RaworcClient>) -> Self {
        Self { api }
    }
}

#[async_trait]
impl Tool for ValidateResponseTool {
    fn name(&self) -> &str {
        "validate_response"
    }

    fn description(&self) -> &str {
        "Validate the current response: ensure an 'output' tool was called."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({ "type":"object", "properties":{} })
    }

    async fn execute(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
        // Heuristic: pick the most recent response with status 'processing' (this flow),
        // otherwise fall back to the most recent response.
        let list = self.api.get_responses(None, None).await.unwrap_or_default();
        let mut current: Option<super::api::ResponseView> = None;
        for r in list.iter().rev() {
            let s = r.status.to_lowercase();
            if s == "processing" {
                current = Some(r.clone());
                break;
            }
        }
        if current.is_none() {
            current = list.last().cloned();
        }

        let mut has_output = false;
        let mut last_output_tool: Option<String> = None;
        if let Some(r) = &current {
            if let Some(items) = r.segments.as_ref() {
                for it in items {
                    let t = it.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if t == "tool_result" {
                        let tool = it.get("tool").and_then(|v| v.as_str()).unwrap_or("");
                        if tool == "output" {
                            has_output = true;
                            last_output_tool = Some(tool.to_string());
                        }
                    }
                }
            }
        }

        if !has_output {
            return Ok(json!({
                "status": "error",
                "tool": "validate_response",
                "has_output": has_output,
                "last_output_tool": last_output_tool,
                "response_id": current.as_ref().map(|r| r.id.clone())
            }));
        }

        Ok(json!({
            "status": "ok",
            "tool": "validate_response",
            "has_output": true,
            "last_output_tool": last_output_tool,
            "response_id": current.as_ref().map(|r| r.id.clone())
        }))
    }
}

// (planner tools removed)

// Normalize a task title for comparison so we can detect duplicates even if
// the model copied decorations from the system prompt (checkboxes, IDs, etc.).
// (planner helpers removed)

// (PlannerCreatePlanTool removed)

// (PlannerAddTaskTool removed)

// (PlannerReadPlanTool removed)
// Purged legacy planner tools (complete_task, clear_plan). Planning is now managed via /agent/plan.md.

fn matches_default_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules"
            | ".venv"
            | "venv"
            | "target"
            | "dist"
            | "build"
            | ".cache"
            | "__pycache__"
            | ".svelte-kit"
            | ".next"
            | "logs"
            | ".pytest_cache"
            | ".mypy_cache"
            | ".ruff_cache"
            | "pip-wheel-metadata"
            | ".tox"
            | ".git"
    )
}

fn matches_default_ignored_file(path: &std::path::Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let ext = ext.to_lowercase();
        return matches!(
            ext.as_str(),
            "pyc" | "pyo" | "o" | "so" | "a" | "d" | "class"
        );
    }
    false
}
