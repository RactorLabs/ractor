use anyhow::Result;
use async_trait::async_trait;

use super::tool_registry::Tool;
use super::api::RaworcClient;
use std::sync::Arc;
use super::tools::{text_edit, TextEditAction, run_bash};
use regex::Regex;
use walkdir::WalkDir;
use globset::{GlobBuilder, GlobSetBuilder};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use std::path::Path;
use anyhow::anyhow;
use serde_json::json;
use chrono::Utc;
use rand::Rng;
use std::collections::HashSet;

const AGENT_ROOT: &str = "/agent";
const CURRENT_PLAN_MARKER: &str = "/agent/logs/current_plan.json";

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

impl ShellTool { pub fn new() -> Self { Self } }

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str { "run_bash" }

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
        let exec_dir = args.get("exec_dir").and_then(|v| v.as_str()).unwrap_or("/agent");
        let commands = args.get("commands").and_then(|v| v.as_str()).unwrap_or("");
        // safety: restrict to /agent
        if !exec_dir.starts_with("/agent") {
            return Ok(json!({"status":"error","tool":"run_bash","error":"exec_dir must be under /agent","exec_dir":exec_dir}));
        }
        // emulate working dir via cd then run
        let cmd = format!("export PATH=\"/agent/bin:$PATH\"; cd '{}' && {}", exec_dir.replace("'", "'\\''"), commands);
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
    fn name(&self) -> &str { "open_file" }

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
        let start_line = args.get("start_line").and_then(|v| v.as_u64()).map(|n| n as usize);
        let end_line = args.get("end_line").and_then(|v| v.as_u64()).map(|n| n as usize);
        let rel = to_rel_under_agent(path).map_err(|e| anyhow::anyhow!(e))?;
        let action = TextEditAction::View { path: rel, start_line, end_line };
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
    fn name(&self) -> &str { "create_file" }

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
        if p.exists() { return Ok(json!({"status":"error","tool":"create_file","error":"file already exists"})); }
        if let Some(parent) = p.parent() { fs::create_dir_all(parent).await.ok(); }
        let mut f = fs::File::create(p).await?;
        f.write_all(content.as_bytes()).await?;
        Ok(json!({"status":"ok","tool":"create_file","bytes":content.len()}))
    }
}

/// Editor: str_replace
pub struct StrReplaceTool;

#[async_trait]
impl Tool for StrReplaceTool {
    fn name(&self) -> &str { "str_replace" }

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
        if count == 0 { return Ok(json!({"status":"error","tool":"str_replace","error":"no matches found"})); }
        if !many && count != 1 { return Ok(json!({"status":"error","tool":"str_replace","error":format!("requires exactly 1 match, found {}", count)})); }
        let new_content = if many { content.replace(old_str, new_str) } else { content.replacen(old_str, new_str, 1) };
        let mut f = fs::File::create(p).await?;
        f.write_all(new_content.as_bytes()).await?;
        Ok(json!({"status":"ok","tool":"str_replace","replaced": if many {count} else {1}}))
    }
}

/// Editor: insert
pub struct InsertTool;

#[async_trait]
impl Tool for InsertTool {
    fn name(&self) -> &str { "insert" }

    fn description(&self) -> &str { "Inserts a new string in a file at a provided line number." }

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
        let line = args.get("insert_line").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let rel = to_rel_under_agent(path).map_err(|e| anyhow::anyhow!(e))?;
        let action = TextEditAction::Insert { path: rel, line, content: content.to_string() };
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
    fn name(&self) -> &str { "remove_str" }

    fn description(&self) -> &str { "Deletes the provided string from the file." }

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
        if count == 0 { return Ok(json!({"status":"error","tool":"remove_str","error":"no matches found"})); }
        if !many && count != 1 { return Ok(json!({"status":"error","tool":"remove_str","error":format!("requires exactly 1 match, found {}", count)})); }
        let new_content = if many { content.replace(remove, "") } else { content.replacen(remove, "", 1) };
        let mut f = fs::File::create(p).await?;
        f.write_all(new_content.as_bytes()).await?;
        Ok(json!({"status":"ok","tool":"remove_str","removed": if many {count} else {1}}))
    }
}

/// Search: find_filecontent
pub struct FindFilecontentTool;

#[async_trait]
impl Tool for FindFilecontentTool {
    fn name(&self) -> &str { "find_filecontent" }

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
                } else { true }
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
            out.push(format!("{}:{}:{}", path.display(), i+1, ctx));
        }
    }
    Ok(())
}

/// Search: find_filename
pub struct FindFilenameTool;

#[async_trait]
impl Tool for FindFilenameTool {
    fn name(&self) -> &str { "find_filename" }

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
        for pat in glob_str.split(';').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let g = GlobBuilder::new(pat).case_insensitive(true).build()?;
            builder.add(g);
        }
        let set = builder.build()?;
        let mut matches = Vec::new();
        let walker = WalkDir::new(root).into_iter().filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                !matches_default_ignored_dir(&name)
            } else { true }
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
    pub fn new(api: Arc<RaworcClient>) -> Self { Self { api } }
}

#[async_trait]
impl Tool for PublishTool {
    fn name(&self) -> &str { "publish_agent" }

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
            Ok(_) => Ok(json!({"status":"ok","tool":"publish_agent","message":"Publish request submitted"})),
            Err(e) => Ok(json!({"status":"error","tool":"publish_agent","error":e.to_string()})),
        }
    }
}

/// Sleep tool (explicit user confirmation required)
pub struct SleepTool {
    api: Arc<RaworcClient>,
}

impl SleepTool {
    pub fn new(api: Arc<RaworcClient>) -> Self { Self { api } }
}

#[async_trait]
impl Tool for SleepTool {
    fn name(&self) -> &str { "sleep_agent" }

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
        let mut delay = args.get("delay_seconds").and_then(|v| v.as_u64()).unwrap_or(5);
        if delay < 5 { delay = 5; }
        let note = args.get("note").and_then(|v| v.as_str()).map(|s| s.to_string());
        match self.api.sleep_agent(Some(delay), note.clone()).await {
            Ok(_) => Ok(json!({"status":"ok","tool":"sleep_agent","message":"Sleep request submitted","delay_seconds": delay, "note": note})),
            Err(e) => Ok(json!({"status":"error","tool":"sleep_agent","error":e.to_string()})),
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max]) }
}

fn parse_exit_code(out: &str) -> Option<i32> {
    if let Some(idx) = out.rfind("[exit_code:") {
        let rest = &out[idx+11..];
        if let Some(end) = rest.find(']') {
            return rest[..end].trim().parse::<i32>().ok();
        }
    }
    None
}

fn strip_exit_marker(out: &str) -> String {
    if let Some(idx) = out.rfind("[exit_code:") {
        let mut s = out[..idx].to_string();
        while s.ends_with(['\n', '\r']) { s.pop(); }
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

// ------------------------------
// Planner tools
// ------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
struct PlanTask {
    id: u32,
    title: String,
    status: String, // "pending" | "completed"
    created_at: String,
    completed_at: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
struct PlanFile {
    plan_id: String,
    title: String,
    created_at: String,
    tasks: Vec<PlanTask>,
    #[serde(default)]
    completed_at: Option<String>,
}

// Normalize a task title for comparison so we can detect duplicates even if
// the model copied decorations from the system prompt (checkboxes, IDs, etc.).
fn normalize_task_title(s: &str) -> String {
    let s = s.trim();
    // Match and strip optional leading decorations then capture the core title.
    // Covers:
    // - optional "Next Task:" prefix
    // - optional list checkbox like "- [ ]" or "- [x]"
    // - optional id markers like "(#12)" or "#12 "
    let re = Regex::new(r"^\s*(?:Next Task:\s*)?(?:-\s*\[(?: |x|X)\]\s*)?(?:\(#\d+\)\s*|#\d+\s+)?(.*)$").unwrap();
    let core = if let Some(caps) = re.captures(s) {
        caps.get(1).map(|m| m.as_str()).unwrap_or(s)
    } else { s };
    // Collapse internal whitespace and lowercase
    core.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn next_task_id(tasks: &[PlanTask]) -> u32 { tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1 }

async fn write_plan(path: &Path, plan: &PlanFile) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).await.ok(); }
    let data = serde_json::to_string_pretty(plan)?;
    let mut f = fs::File::create(path).await?;
    f.write_all(data.as_bytes()).await?;
    Ok(())
}

async fn read_plan(path: &Path) -> anyhow::Result<PlanFile> {
    let s = fs::read_to_string(path).await?;
    let plan: PlanFile = serde_json::from_str(&s)?;
    Ok(plan)
}

async fn current_plan_path() -> anyhow::Result<String> {
    let s = fs::read_to_string(CURRENT_PLAN_MARKER).await?;
    let v: serde_json::Value = serde_json::from_str(&s)?;
    let p = v.get("path").and_then(|x| x.as_str()).ok_or_else(|| anyhow!("no active plan"))?;
    Ok(p.to_string())
}

fn ensure_logs_dir(p: &str) -> anyhow::Result<&Path> {
    let path = ensure_under_agent(p)?;
    // additionally enforce under /agent/logs
    if !path.starts_with("/agent/logs") {
        return Err(anyhow!("planner path must be under /agent/logs"));
    }
    Ok(path)
}

// New tool set with explicit names

pub struct PlannerCreatePlanTool;

#[async_trait]
impl Tool for PlannerCreatePlanTool {
    fn name(&self) -> &str { "create_plan" }

    fn description(&self) -> &str {
        "Creates a plan. Generates a plan file under /agent/logs and sets it as the active plan."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "title": {"type":"string","description":"Plan title (optional)"},
                "tasks": {"type":"array","items":{"type":"string"},"description":"Initial task list (optional)"}
            }
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        // If an active (non-completed) plan already exists, be idempotent: do not create another
        if let Ok(marker_str) = fs::read_to_string(CURRENT_PLAN_MARKER).await {
            if let Ok(marker_json) = serde_json::from_str::<serde_json::Value>(&marker_str) {
                if let Some(path) = marker_json.get("path").and_then(|v| v.as_str()) {
                    let p = ensure_logs_dir(path)?;
                    if p.exists() {
                        if let Ok(existing) = read_plan(p).await {
                            if existing.completed_at.is_none() {
                                return Ok(json!({
                                    "status":"ok","tool":"create_plan",
                                    "note":"plan already exists",
                                    "path": path,
                                    "tasks": existing.tasks.len()
                                }));
                            }
                        }
                    }
                }
            }
        }

        let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("Work Plan").to_string();
        let initial_tasks: Vec<String> = args.get("tasks")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_else(|| vec![]);

        // Generate unique plan path under /agent/logs
        let ts = Utc::now();
        let suffix: u32 = rand::thread_rng().gen_range(10000..99999);
        let filename = format!("plan_{}_{:05}.json", ts.format("%Y%m%d_%H%M%S"), suffix);
        let full_path = format!("/agent/logs/{}", filename);
        let path = Path::new(&full_path);

        let mut tasks = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for t in initial_tasks {
            let norm = normalize_task_title(&t);
            if seen.contains(&norm) { continue; }
            seen.insert(norm);
            tasks.push(PlanTask{ id: next_task_id(&tasks), title: t, status: "pending".to_string(), created_at: Utc::now().to_rfc3339(), completed_at: None });
        }

        let plan = PlanFile {
            plan_id: filename.clone(),
            title,
            created_at: ts.to_rfc3339(),
            tasks,
            completed_at: None,
        };
        write_plan(path, &plan).await?;
        // Record as current active plan
        let marker = serde_json::json!({ "path": full_path });
        let _ = fs::write(CURRENT_PLAN_MARKER, marker.to_string()).await;

        Ok(json!({"status":"ok","tool":"create_plan","path": full_path, "tasks": plan.tasks.len()}))
    }
}

pub struct PlannerAddTaskTool;

#[async_trait]
impl Tool for PlannerAddTaskTool {
    fn name(&self) -> &str { "add_task" }
    fn description(&self) -> &str { "Adds a task to the active plan. Returns error if no active plan." }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "task": {"type":"string","description":"Task description"}
            },
            "required":["task"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("");
        let plan_path = match current_plan_path().await {
            Ok(p) => p,
            Err(_) => return Ok(json!({"status":"error","tool":"add_task","error":"no active plan"}))
        };
        let path = ensure_logs_dir(&plan_path)?;
        let mut plan = read_plan(path).await?;
        // Reject duplicate task titles (robust normalization)
        let norm_new = normalize_task_title(task);
        if let Some(existing) = plan.tasks.iter().find(|t| normalize_task_title(&t.title) == norm_new) {
            // Treat as idempotent no-op to avoid error loops
            return Ok(json!({"status":"ok","tool":"add_task","note":"task already exists","task_id": existing.id}));
        }
        let id = next_task_id(&plan.tasks);
        plan.tasks.push(PlanTask{ id, title: task.to_string(), status: "pending".to_string(), created_at: Utc::now().to_rfc3339(), completed_at: None });
        write_plan(path, &plan).await?;
        Ok(json!({"status":"ok","tool":"add_task","task_id": id}))
    }
}

pub struct PlannerCompleteTaskTool {
    api_client: Arc<RaworcClient>,
}

impl PlannerCompleteTaskTool {
    pub fn new(api_client: Arc<RaworcClient>) -> Self { Self { api_client } }
}

#[async_trait]
impl Tool for PlannerCompleteTaskTool {
    fn name(&self) -> &str { "complete_task" }
    fn description(&self) -> &str { "Completes one task in the active plan. For publish-related tasks, verify a content URL returns 200 under /content/{agent}. Returns error if no active plan." }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "task_id": {"type":"integer","description":"Task ID to complete"},
                "verify_paths": {"type":"array","items":{"type":"string"},"description":"Optional list of files expected to exist when done"},
                "verify_url": {"type":"string","description":"For publish tasks: full published URL to verify (must be under /content/{agent_name}/)"},
                "force": {"type":"boolean","description":"Complete even if verification fails (default false)"}
            },
            "required":["task_id"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<serde_json::Value> {
        let plan_path = match current_plan_path().await {
            Ok(p) => p,
            Err(_) => return Ok(json!({"status":"error","tool":"complete_task","error":"no active plan"}))
        };
        let path = ensure_logs_dir(&plan_path)?;
        let mut plan = read_plan(path).await?;
        let tid = args.get("task_id").and_then(|v| v.as_u64()).map(|n| n as u32).ok_or_else(|| anyhow!("task_id required"))?;
        let verify: Vec<String> = args.get("verify_paths").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect()).unwrap_or_default();
        let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

        let mut missing: Vec<String> = Vec::new();
        for p in verify.iter() {
            if ensure_under_agent(p).is_err() { missing.push(p.clone()); continue; }
            if !Path::new(p).exists() { missing.push(p.clone()); }
        }
        if !missing.is_empty() && !force {
            return Ok(json!({"status":"error","tool":"complete_task","error":"verification failed","missing":missing}));
        }

        // Special handling for publish tasks: verify a published URL returns HTTP 200 under /content/{agent}
        if let Some(this_task) = plan.tasks.iter().find(|t| t.id == tid) {
            let title_lc = this_task.title.to_lowercase();
            if title_lc.contains("publish") {
                // Build expected prefix using RAWORC_HOST_URL and agent name from env or API
                let base = std::env::var("RAWORC_HOST_URL").unwrap_or_else(|_| "".to_string());
                let base = base.trim_end_matches('/').to_string();
                // fallback if RAWORC_HOST_URL is missing
                if base.is_empty() {
                    return Ok(json!({"status":"error","tool":"complete_task","error":"RAWORC_HOST_URL not set; cannot verify published URL"}));
                }
                let agent_name = self.api_client.agent_name().to_string();
                // If still empty, we cannot verify agent path properly
                if agent_name.is_empty() {
                    return Ok(json!({"status":"error","tool":"complete_task","error":"Agent name not found; set RAWORC_AGENT_NAME to enable publish URL verification"}));
                }
                let prefix = format!("{}/content/{}/", base, agent_name);
                let verify_url = args.get("verify_url").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| format!("{}index.html", prefix));
                if !verify_url.starts_with(&prefix) {
                    return Ok(json!({"status":"error","tool":"complete_task","error":"verify_url must be under the agent content prefix","expected_prefix":prefix,"verify_url":verify_url}));
                }
                // Perform HTTP GET
                match reqwest::Client::new().get(&verify_url).send().await {
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        if status != 200 && !force {
                            return Ok(json!({"status":"error","tool":"complete_task","error":"publish verification failed","verify_url":verify_url,"status":status}));
                        }
                    }
                    Err(e) => {
                        if !force {
                            return Ok(json!({"status":"error","tool":"complete_task","error":"failed to fetch verify_url","detail": e.to_string()}));
                        }
                    }
                }
            }
        }

        let mut updated = false;
        for t in plan.tasks.iter_mut() {
            if t.id == tid {
                t.status = "completed".to_string();
                t.completed_at = Some(Utc::now().to_rfc3339());
                updated = true;
                break;
            }
        }
        if !updated { return Ok(json!({"status":"error","tool":"complete_task","error":"task not found"})); }
        write_plan(path, &plan).await?;
        Ok(json!({"status":"ok","tool":"complete_task","task_id": tid, "verified_missing": missing}))
    }
}

pub struct PlannerClearPlanTool;

#[async_trait]
impl Tool for PlannerClearPlanTool {
    fn name(&self) -> &str { "clear_plan" }
    fn description(&self) -> &str { "Remove plan marker and mark the plan complete so it no longer appears in the system prompt." }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type":"object",
            "properties":{}
        })
    }

    async fn execute(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
        let plan_path = match current_plan_path().await {
            Ok(p) => p,
            Err(_) => return Ok(json!({"status":"error","tool":"clear_plan","error":"no active plan"}))
        };
        let path = ensure_logs_dir(&plan_path)?;
        let mut plan = read_plan(path).await?;
        // Fail if there are any open (non-completed) tasks
        let mut open: Vec<serde_json::Value> = Vec::new();
        for t in plan.tasks.iter() {
            if t.status.as_str() != "completed" {
                open.push(json!({"id": t.id, "title": t.title}));
            }
        }
        if !open.is_empty() {
            return Ok(json!({
                "status":"error",
                "tool":"clear_plan",
                "error":"cannot clear plan with open tasks",
                "open_tasks": open
            }));
        }
        plan.completed_at = Some(Utc::now().to_rfc3339());
        write_plan(path, &plan).await?;
        let _ = fs::remove_file(CURRENT_PLAN_MARKER).await;
        Ok(json!({"status":"ok","tool":"clear_plan","path": plan_path}))
    }
}


fn matches_default_ignored_dir(name: &str) -> bool {
    matches!(name,
        "node_modules"|".venv"|"venv"|"target"|"dist"|"build"|".cache"|"__pycache__"|
        ".svelte-kit"|".next"|"logs"|".pytest_cache"|".mypy_cache"|".ruff_cache"|
        "pip-wheel-metadata"|".tox"|".git"
    )
}

fn matches_default_ignored_file(path: &std::path::Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let ext = ext.to_lowercase();
        return matches!(ext.as_str(), "pyc"|"pyo"|"o"|"so"|"a"|"d"|"class");
    }
    false
}
