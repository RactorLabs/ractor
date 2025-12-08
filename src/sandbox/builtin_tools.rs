use anyhow::Result;
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use futures::StreamExt;

use super::toolkit::Tool;
use super::tools::{run_bash, text_edit, TextEditAction};
use anyhow::anyhow;
use globset::{GlobBuilder, GlobSetBuilder};
use regex::Regex;
use reqwest::{header::CONTENT_TYPE, redirect::Policy, Client};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use walkdir::WalkDir;

const SESSION_ROOT: &str = "/sandbox";
const MAX_WEB_FETCH_BYTES_DEFAULT: usize = 200_000;
const MAX_WEB_FETCH_BYTES_HARD: usize = 1_000_000;
const MAX_WEB_FETCH_TIMEOUT_SECS: u64 = 60;

#[derive(Clone, Copy)]
enum FetchSegment {
    Head { offset: usize },
    Tail,
}

fn ensure_under_sandbox(path: &str) -> anyhow::Result<&Path> {
    let p = Path::new(path);
    if !p.starts_with(SESSION_ROOT) {
        return Err(anyhow!(format!("path must be under {}", SESSION_ROOT)));
    }
    Ok(p)
}

fn to_rel_under_sandbox(path: &str) -> anyhow::Result<String> {
    let p = ensure_under_sandbox(path)?;
    let rel = p.strip_prefix(SESSION_ROOT).unwrap_or(p);
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
                "commentary": {"type": "string", "description": "Plain-text explanation of what you are doing (paths/commands/why)"},
                "exec_dir": {"type": "string", "description": "Absolute path to directory where command should be executed"},
                "commands": {"type": "string", "description": "Command(s) to execute. Use && for multi-step."}
            },
            "required": ["commentary", "exec_dir", "commands"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let commentary_present = args
            .get("commentary")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .is_some();
        if !commentary_present {
            return Ok(
                json!({"status":"error","tool":"run_bash","error":"commentary is required"}),
            );
        }
        let exec_dir = args
            .get("exec_dir")
            .and_then(|v| v.as_str())
            .unwrap_or("/sandbox");
        let commands = args.get("commands").and_then(|v| v.as_str()).unwrap_or("");
        // safety: restrict to /sandbox
        if !exec_dir.starts_with("/sandbox") {
            return Ok(
                json!({"status":"error","tool":"run_bash","error":"exec_dir must be under /sandbox","exec_dir":exec_dir}),
            );
        }
        // emulate working dir via cd then run
        let cmd = format!("cd '{}' && {}", exec_dir.replace("'", "'\\''"), commands);
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
                "commentary": {"type":"string","description":"Plain-text explanation of what you are doing (paths/why)"},
                "path": {"type":"string","description":"Absolute path to the file."},
                "start_line": {"type":"integer","description":"Start line (optional)"},
                "end_line": {"type":"integer","description":"End line (optional)"},
                "sudo": {"type":"boolean","description":"Ignored"}
            },
            "required":["commentary","path"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        if args
            .get("commentary")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Ok(
                json!({"status":"error","tool":"open_file","error":"commentary is required"}),
            );
        }
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let start_line = args
            .get("start_line")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let end_line = args
            .get("end_line")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let rel = to_rel_under_sandbox(path).map_err(|e| anyhow::anyhow!(e))?;
        let action = TextEditAction::View {
            path: rel,
            start_line,
            end_line,
        };
        match text_edit(action).await {
            Ok(s) => Ok(json!({
                "status":"ok","tool":"open_file",
                "content": s,
                "truncated": s.contains("[truncated]")
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
                "commentary": {"type":"string","description":"Plain-text explanation of what you are creating and why"},
                "path": {"type":"string","description":"Absolute path to the file. File must not exist yet."},
                "content": {"type":"string","description":"Content of the new file. Don't start with backticks."},
                "sudo": {"type":"boolean","description":"Ignored"}
            },
            "required":["commentary","path","content"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        if args
            .get("commentary")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Ok(
                json!({"status":"error","tool":"create_file","error":"commentary is required"}),
            );
        }
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let p = ensure_under_sandbox(path)?;
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
                "commentary": {"type":"string","description":"Plain-text explanation of the replacement and why"},
                "path": {"type":"string","description":"Absolute path to the file"},
                "old_str": {"type":"string","description":"Original text to replace (exact match)"},
                "new_str": {"type":"string","description":"Replacement text"},
                "many": {"type":"boolean","description":"Whether to replace all occurrences (default false)"},
                "sudo": {"type":"boolean","description":"Ignored"}
            },
            "required":["commentary","path","old_str","new_str"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        if args
            .get("commentary")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Ok(
                json!({"status":"error","tool":"str_replace","error":"commentary is required"}),
            );
        }
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let old_str = args.get("old_str").and_then(|v| v.as_str()).unwrap_or("");
        let new_str = args.get("new_str").and_then(|v| v.as_str()).unwrap_or("");
        let many = args.get("many").and_then(|v| v.as_bool()).unwrap_or(false);

        let p = ensure_under_sandbox(path)?;
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
                "commentary": {"type":"string","description":"Plain-text explanation of the insertion and why"},
                "path": {"type":"string","description":"Absolute path to the file"},
                "insert_line": {"type":"integer","description":"Line number to insert at (1-based)"},
                "content": {"type":"string","description":"Content to insert"},
                "sudo": {"type":"boolean","description":"Ignored"}
            },
            "required":["commentary","path","insert_line","content"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        if args
            .get("commentary")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Ok(json!({"status":"error","tool":"insert","error":"commentary is required"}));
        }
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let line = args
            .get("insert_line")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let rel = to_rel_under_sandbox(path).map_err(|e| anyhow::anyhow!(e))?;
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
                "commentary": {"type":"string","description":"Plain-text explanation of the removal and why"},
                "path": {"type":"string","description":"Absolute path to the file"},
                "content": {"type":"string","description":"Exact string to remove (may be multi-line)"},
                "many": {"type":"boolean","description":"Whether to remove all instances (default false)"},
                "sudo": {"type":"boolean","description":"Ignored"}
            },
            "required":["commentary","path","content"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        if args
            .get("commentary")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Ok(
                json!({"status":"error","tool":"remove_str","error":"commentary is required"}),
            );
        }
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let remove = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let many = args.get("many").and_then(|v| v.as_bool()).unwrap_or(false);
        let p = ensure_under_sandbox(path)?;
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
        "Returns file content matches for the provided regex at the given path. The task output will cite the files and line numbers of the matches along with some surrounding content. Never use grep but use this command instead since it is optimized for your machine."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "commentary": {"type":"string","description":"Plain-text explanation of the search and why"},
                "path": {"type":"string","description":"Absolute path to a file or directory"},
                "regex": {"type":"string","description":"Regex to search for"}
            },
            "required":["commentary","path","regex"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        if args
            .get("commentary")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Ok(
                json!({"status":"error","tool":"find_filecontent","error":"commentary is required"}),
            );
        }
        let root = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let pattern = args.get("regex").and_then(|v| v.as_str()).unwrap_or("");
        let re = Regex::new(pattern).map_err(|e| anyhow::anyhow!(e))?;
        let mut hits = Vec::new();
        let _ = ensure_under_sandbox(root)?;
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
                let entry_path = entry.path();
                if entry.file_type().is_file() && !matches_default_ignored_file(entry_path) {
                    scan_file(entry_path, &re, &mut hits).await.ok();
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
                "commentary": {"type":"string","description":"Plain-text explanation of the filename search and why"},
                "path": {"type":"string","description":"Absolute path of the directory to search in."},
                "glob": {"type":"string","description":"Patterns to search for; separate multiple with '; '"}
            },
            "required":["commentary","path","glob"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        if args
            .get("commentary")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Ok(
                json!({"status":"error","tool":"find_filename","error":"commentary is required"}),
            );
        }
        let root = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let glob_str = args.get("glob").and_then(|v| v.as_str()).unwrap_or("");
        let _ = ensure_under_sandbox(root)?;
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

/// Fetch remote HTTP/HTTPS content without shelling out.
pub struct WebFetchTool;

impl WebFetchTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch HTTP/HTTPS content (GET) without invoking a shell. Prefer this over run_bash+curl when you need to read web resources."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "commentary":{"type":"string","description":"Plain-text explanation of why this URL is being fetched."},
                "url":{"type":"string","description":"HTTP or HTTPS URL to fetch."},
                "timeout_seconds":{"type":"integer","description":"Optional timeout (1-60 seconds)."},
                "max_bytes":{"type":"integer","description":"Maximum response bytes to return (1kB-1MB)."},
                "segment":{"type":"string","enum":["head","tail"],"description":"Which portion of the response to capture (default head)."},
                "offset_bytes":{"type":"integer","description":"When segment=head, skip this many bytes before capturing the next max_bytes chunk."}
            },
            "required":["commentary","url"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        if args
            .get("commentary")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Ok(
                json!({"status":"error","tool":"web_fetch","error":"commentary is required"}),
            );
        }
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if url.is_empty() {
            return Ok(json!({"status":"error","tool":"web_fetch","error":"url is required"}));
        }
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Ok(
                json!({"status":"error","tool":"web_fetch","error":"url must start with http:// or https://"}),
            );
        }
        let timeout_secs = args
            .get("timeout_seconds")
            .and_then(|v| v.as_u64())
            .map(|v| v.clamp(1, MAX_WEB_FETCH_TIMEOUT_SECS))
            .unwrap_or(20);
        let max_bytes_raw = args
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(MAX_WEB_FETCH_BYTES_DEFAULT as u64);
        let max_bytes = max_bytes_raw.clamp(1024, MAX_WEB_FETCH_BYTES_HARD as u64) as usize;
        let offset_bytes = args
            .get("offset_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            .min(MAX_WEB_FETCH_BYTES_HARD as u64) as usize;
        let segment_mode = args
            .get("segment")
            .and_then(|v| v.as_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_else(|| "head".to_string());
        let segment = if segment_mode == "tail" {
            FetchSegment::Tail
        } else {
            FetchSegment::Head {
                offset: offset_bytes,
            }
        };

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .redirect(Policy::limited(3))
            .user_agent("tsbx-web-fetch/1.0 (+https://github.com/RactorLabs/tsbx)")
            .build()?;

        let response = client.get(&url).send().await;
        let response = match response {
            Ok(resp) => resp,
            Err(err) => {
                return Ok(
                    json!({"status":"error","tool":"web_fetch","error":format!("request failed: {}", err)}),
                );
            }
        };

        let status = response.status().as_u16();
        let final_url = response.url().to_string();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let mut body: Vec<u8> = Vec::with_capacity(max_bytes.min(8192));
        let mut stream = response.bytes_stream();
        let mut truncated_head = matches!(segment, FetchSegment::Head { .. }) && offset_bytes > 0;
        let mut truncated_tail = false;
        let mut bytes_skipped_total: usize = 0;
        let mut total_bytes = 0usize;
        let mut remaining_offset = offset_bytes;

        match segment {
            FetchSegment::Head { .. } => {
                while let Some(chunk) = stream.next().await {
                    let chunk = match chunk {
                        Ok(bytes) => bytes,
                        Err(err) => {
                            return Ok(
                                json!({"status":"error","tool":"web_fetch","error":format!("read failed: {}", err)}),
                            );
                        }
                    };
                    total_bytes += chunk.len();
                    if remaining_offset > 0 {
                        if chunk.len() <= remaining_offset {
                            remaining_offset -= chunk.len();
                            bytes_skipped_total += chunk.len();
                            continue;
                        } else {
                            bytes_skipped_total += remaining_offset;
                            let remainder = chunk.len() - remaining_offset;
                            let start = remaining_offset;
                            remaining_offset = 0;
                            let take = remainder.min(max_bytes.saturating_sub(body.len()));
                            body.extend_from_slice(&chunk[start..start + take]);
                            if take < remainder {
                                truncated_tail = true;
                                break;
                            }
                            continue;
                        }
                    }
                    if body.len() >= max_bytes {
                        truncated_tail = true;
                        break;
                    }
                    let available = max_bytes.saturating_sub(body.len());
                    if available == 0 {
                        truncated_tail = true;
                        break;
                    }
                    let take = available.min(chunk.len());
                    body.extend_from_slice(&chunk[..take]);
                    if take < chunk.len() {
                        truncated_tail = true;
                        break;
                    }
                }
            }
            FetchSegment::Tail => {
                while let Some(chunk) = stream.next().await {
                    let chunk = match chunk {
                        Ok(bytes) => bytes,
                        Err(err) => {
                            return Ok(
                                json!({"status":"error","tool":"web_fetch","error":format!("read failed: {}", err)}),
                            );
                        }
                    };
                    total_bytes += chunk.len();
                    body.extend_from_slice(&chunk);
                    if body.len() > max_bytes {
                        let excess = body.len() - max_bytes;
                        body.drain(0..excess);
                        bytes_skipped_total += excess;
                        truncated_head = true;
                    }
                }
            }
        }

        if matches!(segment, FetchSegment::Head { .. }) && offset_bytes == 0 {
            truncated_head = false;
        }

        let applied_offset = if matches!(segment, FetchSegment::Head { .. }) {
            bytes_skipped_total.min(offset_bytes)
        } else {
            0
        };
        let truncated = truncated_head || truncated_tail;

        let (encoding, body_value) = match String::from_utf8(body.clone()) {
            Ok(text) => ("utf-8", Value::String(text)),
            Err(_) => {
                let encoded = general_purpose::STANDARD.encode(&body);
                ("base64", Value::String(encoded))
            }
        };

        Ok(json!({
            "status":"ok",
            "tool":"web_fetch",
            "url": url,
            "final_url": final_url,
            "http_status": status,
            "content_type": content_type,
            "encoding": encoding,
            "body": body_value,
            "segment": match segment {
                FetchSegment::Head { .. } => "head",
                FetchSegment::Tail => "tail",
            },
            "max_bytes": max_bytes,
            "offset_bytes": applied_offset,
            "bytes_collected": body.len(),
            "bytes_skipped": bytes_skipped_total,
            "truncated_head": truncated_head,
            "truncated_tail": truncated_tail,
            "truncated": truncated,
            "total_bytes_read": total_bytes
        }))
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
/// Supported types: "md" (markdown string), "text" (plain string), "json" (any JSON value)
pub struct OutputTool;

#[async_trait]
impl Tool for OutputTool {
    fn name(&self) -> &str {
        "output"
    }

    fn description(&self) -> &str {
        "Send final user-facing outputs. Accepts an array of items where each item has { type: 'md'|'text'|'json', content }. This concludes the current task run."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "commentary": {"type":"string","description":"Plain-text explanation of the final outputs"},
                "content": {
                    "type":"array",
                    "description":"List of outputs to present to the user",
                    "items": {
                        "type":"object",
                        "properties":{
                            "type": {"type":"string","enum":["md","text","json"],"description":"Output type"},
                            "title": {"type":"string","description":"Title heading for this item (required)"},
                            "content": {"description":"For md/text: string; for json: any JSON value"}
                        },
                        "required":["type","title","content"]
                    }
                }
            },
            "required":["commentary","content"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        if args
            .get("commentary")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Ok(json!({"status":"error","tool":"output","error":"commentary is required"}));
        }
        let items_in = args
            .get("content")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut items_out: Vec<Value> = Vec::new();
        for (idx, it) in items_in.iter().enumerate() {
            let typ_val = it.get("type");
            if typ_val.is_none() {
                return Ok(json!({
                    "status":"error",
                    "tool":"output",
                    "error": format!("Developer note: content[{}] is missing required field 'type'. Use one of: markdown, json, url, text.", idx),
                    "supported_types": ["markdown","json","url","text"]
                }));
            }
            let typ = typ_val
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
                "markdown" | "md" => {
                    let content = it.get("content").and_then(|v| v.as_str()).ok_or_else(|| {
                        anyhow!(format!(
                            "content[{}].content must be string for markdown",
                            idx
                        ))
                    })?;
                    items_out.push(json!({"type":"md","title": title, "content": content}));
                }
                "text" => {
                    let content = it.get("content").and_then(|v| v.as_str()).ok_or_else(|| {
                        anyhow!(format!("content[{}].content must be string for text", idx))
                    })?;
                    items_out.push(json!({"type":"text","title": title, "content": content}));
                }
                "json" => {
                    let content = it.get("content").cloned().unwrap_or(Value::Null);
                    items_out.push(json!({"type":"json","title": title, "content": content}));
                }
                _ => {
                    return Ok(json!({
                        "status":"error",
                        "tool":"output",
                        "error": format!("unsupported type '{}' at index {}", typ, idx),
                        "supported_types": ["md","text","json"]
                    }));
                }
            }
        }
        Ok(json!({
            "status":"ok",
            "tool":"output",
            "items": items_out,
            "supported_types": ["md","text","json"]
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::WebFetchTool;
    use crate::sandbox::toolkit::Tool;
    use serde_json::json;

    #[tokio::test]
    async fn web_fetch_reads_example_domain() {
        let tool = WebFetchTool::new();
        let args = json!({
            "commentary": "fetch example domain",
            "url": "https://example.com",
            "timeout_seconds": 10,
            "max_bytes": 5000
        });
        let result = tool.execute(&args).await.expect("tool runs");
        assert_eq!(result.get("status").and_then(|v| v.as_str()), Some("ok"));
        assert_eq!(
            result
                .get("http_status")
                .and_then(|v| v.as_u64())
                .unwrap_or_default(),
            200
        );
        assert_eq!(
            result.get("encoding").and_then(|v| v.as_str()),
            Some("utf-8")
        );
        let body = result
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        assert!(
            body.contains("Example Domain"),
            "expected Example Domain in body, got {}",
            body
        );
        assert_eq!(result.get("segment").and_then(|v| v.as_str()), Some("head"));
        assert_eq!(
            result
                .get("bytes_skipped")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            0
        );
        assert_eq!(
            result.get("truncated_head").and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[tokio::test]
    async fn web_fetch_fetches_multiple_sites() {
        let tool = WebFetchTool::new();
        let targets = [
            ("https://example.com", "Example Domain"),
            ("https://www.rust-lang.org", "Rust"),
            ("https://developer.mozilla.org", "MDN"),
        ];
        for (url, expected_hint) in targets {
            let args = json!({
                "commentary": format!("fetch {}", url),
                "url": url,
                "timeout_seconds": 20,
                "max_bytes": 100_000
            });
            let result = tool
                .execute(&args)
                .await
                .unwrap_or_else(|e| panic!("web_fetch failed for {}: {}", url, e));
            assert_eq!(
                result.get("status").and_then(|v| v.as_str()),
                Some("ok"),
                "status not ok for {}: {:?}",
                url,
                result
            );
            let http_status = result
                .get("http_status")
                .and_then(|v| v.as_u64())
                .unwrap_or_default();
            assert!(
                (200..400).contains(&http_status),
                "unexpected HTTP status {} for {}",
                http_status,
                url
            );
            let encoding = result
                .get("encoding")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            assert!(
                matches!(encoding, "utf-8" | "base64"),
                "unexpected encoding {} for {}",
                encoding,
                url
            );
            let body = result
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            assert!(!body.is_empty(), "empty body for {} ({})", url, encoding);
            if encoding == "utf-8" {
                assert!(
                    body.contains(expected_hint),
                    "body missing hint '{}' for {}",
                    expected_hint,
                    url
                );
            }
            assert_eq!(result.get("segment").and_then(|v| v.as_str()), Some("head"));
        }
    }

    #[tokio::test]
    async fn web_fetch_head_with_offset() {
        let tool = WebFetchTool::new();
        let offset = 1_000;
        let args = json!({
            "commentary": "fetch segment of example domain",
            "url": "https://example.com",
            "timeout_seconds": 20,
            "max_bytes": 2_000,
            "segment": "head",
            "offset_bytes": offset
        });
        let result = tool.execute(&args).await.expect("tool runs");
        assert_eq!(result.get("segment").and_then(|v| v.as_str()), Some("head"));
        let applied_offset = result
            .get("offset_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert!(
            applied_offset as usize <= offset,
            "applied offset should not exceed requested"
        );
        assert!(
            result
                .get("bytes_collected")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                <= 2_000,
            "collected bytes should respect max_bytes"
        );
        assert_eq!(result.get("segment").and_then(|v| v.as_str()), Some("head"));
        assert_eq!(
            result.get("truncated_head").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn web_fetch_tail_segment() {
        let tool = WebFetchTool::new();
        let args = json!({
            "commentary": "fetch tail of example domain",
            "url": "https://example.com",
            "timeout_seconds": 20,
            "max_bytes": 512,
            "segment": "tail"
        });
        let result = tool.execute(&args).await.expect("tool runs");
        assert_eq!(result.get("segment").and_then(|v| v.as_str()), Some("tail"));
        let max_bytes = result
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let collected = result
            .get("bytes_collected")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert!(
            collected <= max_bytes,
            "collected {} exceeds advertised max {}",
            collected,
            max_bytes
        );
        assert_eq!(
            result.get("truncated_tail").and_then(|v| v.as_bool()),
            Some(false)
        );
        // Tail mode should report truncated_head true when response exceeds limit.
        let truncated_head = result
            .get("truncated_head")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(
            truncated_head
                || result
                    .get("bytes_skipped")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
                    == 0,
            "tail mode should either truncate head or indicate no skipped bytes"
        );
    }
}

// (ShowAndTellTool removed) — tools may include an optional 'commentary' field in their args.
// (planner tools removed)

// Normalize a task title for comparison so we can detect duplicates even if
// the model copied decorations from the system prompt (checkboxes, IDs, etc.).
// (planner helpers removed)

// (PlannerCreatePlanTool removed)

// (PlannerAddTaskTool removed)

// (PlannerReadPlanTool removed)
// Purged legacy planner tools (complete_task, clear_plan). Planning is now managed via /sandbox/plan.md.

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
