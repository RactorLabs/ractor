use anyhow::Result;
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::info;

const AGENT_ROOT: &str = "/agent";
const MAX_OUTPUT_BYTES: usize = 200_000; // cap tool outputs

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum TextEditAction {
    View {
        path: String,
        start_line: Option<usize>,
        end_line: Option<usize>,
    },
    Create {
        path: String,
        content: String,
    },
    StrReplace {
        path: String,
        target: String,
        replacement: String,
    },
    Insert {
        path: String,
        line: usize,
        content: String,
    },
}

fn normalize_path(p: &str) -> Result<PathBuf> {
    let mut full = PathBuf::from(AGENT_ROOT);
    let rel = Path::new(p);
    for c in rel.components() {
        match c {
            Component::Normal(seg) => full.push(seg),
            Component::CurDir => {}
            Component::ParentDir => {
                // prevent escaping the agent root
                if !full.starts_with(AGENT_ROOT) {
                    anyhow::bail!("Invalid path traversal");
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("Absolute paths not allowed")
            }
        }
    }
    // Ensure still within root
    let canon_parent = Path::new(AGENT_ROOT);
    if !full.starts_with(canon_parent) {
        anyhow::bail!("Path escapes agent root");
    }
    Ok(full)
}

fn truncate(mut s: String) -> String {
    if s.len() > MAX_OUTPUT_BYTES {
        s.truncate(MAX_OUTPUT_BYTES);
        s.push_str("\n[truncated]\n");
    }
    s
}

pub async fn run_bash(cmd: &str) -> Result<String> {
    let start_time = std::time::SystemTime::now();
    info!(tool = "bash", %cmd, "tool start");
    let out = Command::new("bash")
        .arg("-lc")
        .arg(cmd)
        .current_dir(AGENT_ROOT)
        .output()
        .await?;

    let mut s = String::new();
    if !out.stdout.is_empty() {
        s.push_str(&String::from_utf8_lossy(&out.stdout));
    }
    if !out.stderr.is_empty() {
        if !s.is_empty() {
            s.push_str("\n");
        }
        s.push_str("[stderr]\n");
        s.push_str(&String::from_utf8_lossy(&out.stderr));
    }
    if !out.status.success() {
        s.push_str(&format!(
            "\n[exit_code:{}]",
            out.status.code().unwrap_or(-1)
        ));
    }
    // Save bash log
    let _ = save_bash_log(cmd, &out, start_time).await;
    let dur_ms = start_time.elapsed().map(|d| d.as_millis()).unwrap_or(0);
    info!(
        tool = "bash",
        %cmd,
        exit = out.status.code().unwrap_or(-1),
        stdout_len = out.stdout.len(),
        stderr_len = out.stderr.len(),
        took_ms = dur_ms,
        "tool end"
    );
    Ok(truncate(s))
}

pub async fn text_edit(action: TextEditAction) -> Result<String> {
    let start_time = std::time::SystemTime::now();
    match action {
        TextEditAction::View {
            path,
            start_line,
            end_line,
        } => {
            info!(tool = "text_edit", action = "view", %path, start_line, end_line, "tool start");
            let full = normalize_path(&path)?;
            if full.is_dir() {
                let mut entries = fs::read_dir(&full).await?;
                let mut names = Vec::new();
                while let Some(e) = entries.next_entry().await? {
                    names.push(e.file_name().to_string_lossy().to_string());
                }
                names.sort();
                let out = names.join("\n");
                let _ = save_text_editor_log(
                    "view",
                    &path,
                    true,
                    serde_json::json!({"start_line":start_line, "end_line":end_line}),
                    &out,
                    start_time,
                )
                .await;
                info!(tool = "text_edit", action = "view", %path, is_dir = true, entries = names.len(), "tool end");
                return Ok(out);
            }
            let content = fs::read_to_string(&full).await?;
            if let (Some(s), Some(e)) = (start_line, end_line) {
                let lines: Vec<&str> = content.lines().collect();
                let s0 = s.saturating_sub(1);
                let e0 = e.min(lines.len());
                let slice = if s0 < e0 { &lines[s0..e0] } else { &[][..] };
                let out = slice.join("\n");
                let _ = save_text_editor_log(
                    "view",
                    &path,
                    true,
                    serde_json::json!({"start_line":s, "end_line":e}),
                    &out,
                    start_time,
                )
                .await;
                info!(tool = "text_edit", action = "view", %path, range = format!("{}-{}", s, e), bytes = out.len(), "tool end");
                Ok(out)
            } else {
                let out = truncate(content);
                let _ = save_text_editor_log(
                    "view",
                    &path,
                    true,
                    serde_json::json!({}),
                    &out,
                    start_time,
                )
                .await;
                info!(tool = "text_edit", action = "view", %path, bytes = out.len(), "tool end");
                Ok(out)
            }
        }
        TextEditAction::Create { path, content } => {
            info!(tool = "text_edit", action = "create", %path, len = content.len(), "tool start");
            let full = normalize_path(&path)?;
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).await?;
            }
            let mut f = fs::File::create(&full).await?;
            f.write_all(content.as_bytes()).await?;
            let msg = format!("created: {} ({} bytes)", full.display(), content.len());
            let _ = save_text_editor_log(
                "create",
                &path,
                true,
                serde_json::json!({"length":content.len()}),
                &msg,
                start_time,
            )
            .await;
            info!(tool = "text_edit", action = "create", %path, bytes = content.len(), "tool end");
            Ok(msg)
        }
        TextEditAction::StrReplace {
            path,
            target,
            replacement,
        } => {
            info!(tool = "text_edit", action = "str_replace", %path, target = target.as_str(), replacement_len = replacement.len(), "tool start");
            let full = normalize_path(&path)?;
            let content = fs::read_to_string(&full).await?;
            let count = content.matches(&target).count();
            if count != 1 {
                let err = format!("str_replace requires exactly 1 match, found {}", count);
                let _ = save_text_editor_log(
                    "str_replace",
                    &path,
                    false,
                    serde_json::json!({"target":target, "replacement":replacement, "count":count}),
                    &err,
                    start_time,
                )
                .await;
                info!(tool = "text_edit", action = "str_replace", %path, matches = count, success = false, "tool end");
                anyhow::bail!(err);
            }
            let new_content = content.replacen(&target, &replacement, 1);
            let mut f = fs::File::create(&full).await?;
            f.write_all(new_content.as_bytes()).await?;
            let msg = format!("replaced 1 occurrence in {}", full.display());
            let _ = save_text_editor_log(
                "str_replace",
                &path,
                true,
                serde_json::json!({"target":target}),
                &msg,
                start_time,
            )
            .await;
            info!(tool = "text_edit", action = "str_replace", %path, matches = 1, success = true, "tool end");
            Ok(msg)
        }
        TextEditAction::Insert {
            path,
            line,
            content,
        } => {
            info!(tool = "text_edit", action = "insert", %path, line, len = content.len(), "tool start");
            let full = normalize_path(&path)?;
            let existing = fs::read_to_string(&full).await.unwrap_or_default();
            let mut lines: Vec<&str> = existing.lines().collect();
            let idx = line.saturating_sub(1).min(lines.len());
            let mut new_lines: Vec<String> = Vec::with_capacity(lines.len() + 1);
            for (i, l) in lines.iter().enumerate() {
                if i == idx {
                    new_lines.push(content.clone());
                }
                new_lines.push((*l).to_string());
            }
            if idx == lines.len() {
                new_lines.push(content);
            }
            let final_content = new_lines.join("\n");
            let mut f = fs::File::create(&full).await?;
            f.write_all(final_content.as_bytes()).await?;
            let msg = format!("inserted at line {} in {}", line, full.display());
            let _ = save_text_editor_log(
                "insert",
                &path,
                true,
                serde_json::json!({"line":line}),
                &msg,
                start_time,
            )
            .await;
            info!(tool = "text_edit", action = "insert", %path, line, new_size = final_content.len(), "tool end");
            Ok(msg)
        }
    }
}

async fn save_bash_log(
    cmd: &str,
    out: &std::process::Output,
    start: std::time::SystemTime,
) -> Result<()> {
    use std::time::UNIX_EPOCH;
    let ts = start
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = format!("{}/logs/bash_{}.log", AGENT_ROOT, ts);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let exit = out.status.code().unwrap_or(-1);
    let content = format!(
        "=== BASH LOG ===\nTimestamp: {}\nCommand: {}\nExit: {}\n\n--- STDOUT ---\n{}\n\n--- STDERR ---\n{}\n=== END ===\n",
        ts, cmd, exit, stdout, stderr
    );
    if let Err(e) = fs::write(&path, content).await {
        let _ = e;
    }
    Ok(())
}

async fn save_text_editor_log(
    command: &str,
    path_rel: &str,
    success: bool,
    params: serde_json::Value,
    result_msg: &str,
    start: std::time::SystemTime,
) -> Result<()> {
    use std::time::UNIX_EPOCH;
    let ts = start
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = format!("{}/logs/text_editor_{}.log", AGENT_ROOT, ts);
    let content = format!(
        "=== TEXT EDITOR LOG ===\nTimestamp: {}\nCommand: {}\nPath: {}\nSuccess: {}\nParams: {}\n\nResult: {}\n=== END ===\n",
        ts, command, path_rel, success, params, result_msg
    );
    if let Err(e) = fs::write(&path, content).await {
        let _ = e;
    }
    Ok(())
}
