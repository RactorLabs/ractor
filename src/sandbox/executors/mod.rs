use crate::sandbox::api::{TSBXClient, TaskSummary};
use crate::sandbox::error::Result;
use crate::sandbox::shared_task::TaskType;
use serde_json::json;
use std::sync::Arc;
use tokio::process::Command;
use tracing::warn;

const MAX_STREAM_CHARS: usize = 8_192;

pub struct TaskExecutorContext<'a> {
    pub api_client: &'a Arc<TSBXClient>,
}

impl<'a> TaskExecutorContext<'a> {
    pub fn new(api_client: &'a Arc<TSBXClient>) -> Self {
        Self { api_client }
    }
}

pub async fn run_shell_task(ctx: &TaskExecutorContext<'_>, task: &TaskSummary) -> Result<()> {
    let command_text = extract_text_input(task);
    let spec = CommandSpec {
        label: "shell",
        language: "bash",
        task_type: TaskType::SH,
        display: command_text.clone(),
        program: "sh",
        args: vec!["-lc".to_string(), command_text],
    };
    execute_command(ctx, task, spec).await
}

pub async fn run_python_task(ctx: &TaskExecutorContext<'_>, task: &TaskSummary) -> Result<()> {
    let snippet = extract_text_input(task);
    let spec = CommandSpec {
        label: "python",
        language: "python",
        task_type: TaskType::PY,
        display: snippet.clone(),
        program: "python3",
        args: vec!["-c".to_string(), snippet],
    };
    execute_command(ctx, task, spec).await
}

pub async fn run_javascript_task(ctx: &TaskExecutorContext<'_>, task: &TaskSummary) -> Result<()> {
    let snippet = extract_text_input(task);
    let spec = CommandSpec {
        label: "javascript",
        language: "javascript",
        task_type: TaskType::JS,
        display: snippet.clone(),
        program: "node",
        args: vec!["-e".to_string(), snippet],
    };
    execute_command(ctx, task, spec).await
}

struct CommandSpec<'a> {
    label: &'a str,
    language: &'a str,
    task_type: TaskType,
    display: String,
    program: &'a str,
    args: Vec<String>,
}

async fn execute_command(
    ctx: &TaskExecutorContext<'_>,
    task: &TaskSummary,
    spec: CommandSpec<'_>,
) -> Result<()> {
    if spec.display.trim().is_empty() {
        return record_failure(
            ctx,
            task,
            spec.task_type,
            spec.label,
            "Task input was empty; provide instructions for this executor.",
        )
        .await;
    }

    let output = match Command::new(spec.program).args(&spec.args).output().await {
        Ok(result) => result,
        Err(err) => {
            let message = format!("Failed to run {} task: {}", spec.label, err);
            warn!("{}", message);
            record_failure(ctx, task, spec.task_type, spec.label, &message).await?;
            return Ok(());
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let (stdout_excerpt, _stdout_truncated) = clip(&stdout);
    let (stderr_excerpt, _stderr_truncated) = clip(&stderr);

    let exit_code = output.status.code().unwrap_or(-1);
    let success = output.status.success();

    // Build output items for non-NL tasks
    let mut output_items = Vec::new();

    // Commentary about execution
    output_items.push(json!({
        "type": "commentary",
        "content": if success {
            format!("{} executed successfully", spec.label)
        } else {
            format!("{} execution failed", spec.label)
        }
    }));

    // stdout if present
    if !stdout_excerpt.is_empty() {
        output_items.push(json!({
            "type": "stdout",
            "content": stdout_excerpt
        }));
    }

    // stderr if present
    if !stderr_excerpt.is_empty() {
        output_items.push(json!({
            "type": "stderr",
            "content": stderr_excerpt
        }));
    }

    // exit code
    output_items.push(json!({
        "type": "exit_code",
        "content": exit_code.to_string()
    }));

    // For non-NL tasks, store output directly without steps
    ctx.api_client
        .update_task(
            &task.id,
            Some(if success { "completed" } else { "failed" }.to_string()),
            Some(output_items),
            None,  // No steps for non-NL tasks
            Some(task.context_length),
            None,
        )
        .await?;

    Ok(())
}

async fn record_failure(
    ctx: &TaskExecutorContext<'_>,
    task: &TaskSummary,
    _task_type: TaskType,
    _executor: &str,
    message: &str,
) -> Result<()> {
    let failure_step = json!({
        "type": "final",
        "executor": "task_executor",
        "content": message,
        "status": "failed"
    });
    ctx.api_client
        .update_task(
            &task.id,
            Some("failed".to_string()),
            Some(vec![json!({"type": "text", "content": message })]),
            Some(vec![failure_step]),
            Some(task.context_length),
            None,
        )
        .await?;
    Ok(())
}

fn extract_text_input(task: &TaskSummary) -> String {
    let mut parts = Vec::new();
    for item in &task.input {
        if item
            .get("type")
            .and_then(|t| t.as_str())
            .map(|t| t.eq_ignore_ascii_case("text"))
            .unwrap_or(false)
        {
            if let Some(value) = item.get("content").and_then(|v| v.as_str()) {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
            }
        }
    }
    parts.join("\n\n")
}

fn clip(value: &str) -> (String, bool) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return (String::new(), false);
    }
    let mut iter = trimmed.chars();
    let collected: String = iter.by_ref().take(MAX_STREAM_CHARS).collect();
    let truncated = iter.next().is_some();
    (collected, truncated)
}
