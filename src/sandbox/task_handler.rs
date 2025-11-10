use super::api::{TaskSandboxClient, TaskView};
use super::command::parse_command_xml;
use super::error::{HostError, Result};
use super::guardrails::Guardrails;
use super::inference::{ChatMessage, InferenceClient};
use super::toolkit::{ExecutionResult, ToolCatalog};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

const MAX_TOOL_OUTPUT_CHARS: usize = 8_000;

pub struct TaskHandler {
    api_client: Arc<TaskSandboxClient>,
    inference_client: Arc<InferenceClient>,
    guardrails: Arc<Guardrails>,
    toolkit: Arc<ToolCatalog>,
    processed_task_ids: Arc<Mutex<HashSet<String>>>,
    request_created_at: DateTime<Utc>,
}

impl TaskHandler {
    pub fn new(
        api_client: Arc<TaskSandboxClient>,
        inference_client: Arc<InferenceClient>,
        guardrails: Arc<Guardrails>,
    ) -> Self {
        let request_created_at = std::env::var("TSBX_REQUEST_CREATED_AT")
            .ok()
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|| {
                warn!("TSBX_REQUEST_CREATED_AT not found, using current time");
                Utc::now()
            });

        Self {
            api_client,
            inference_client,
            guardrails,
            toolkit: Arc::new(ToolCatalog::new()),
            processed_task_ids: Arc::new(Mutex::new(HashSet::new())),
            request_created_at,
        }
    }

    pub async fn initialize_processed_tracking(&self) -> Result<()> {
        info!(
            "Initializing task tracking; request created at {}",
            self.request_created_at
        );
        let total = self.api_client.get_task_count().await.unwrap_or(0);
        let limit: u32 = 500;
        let offset = if total > limit as u64 {
            (total - limit as u64) as u32
        } else {
            0
        };
        let all = self.api_client.get_tasks(Some(limit), Some(offset)).await?;
        let mut processed = HashSet::new();
        for r in &all {
            if let Ok(t) = DateTime::parse_from_rfc3339(&r.created_at) {
                if t.with_timezone(&Utc) < self.request_created_at {
                    processed.insert(r.id.clone());
                }
            }
        }
        let mut guard = self.processed_task_ids.lock().await;
        *guard = processed;
        Ok(())
    }

    pub async fn poll_and_process(&self) -> Result<usize> {
        let total = self.api_client.get_task_count().await.unwrap_or(0);
        let window: u32 = 50;
        let offset = if total > window as u64 {
            (total - window as u64) as u32
        } else {
            0
        };
        let recent = self
            .api_client
            .get_tasks(Some(window), Some(offset))
            .await?;
        if recent.is_empty() {
            return Ok(0);
        }

        let mut pending: Vec<TaskView> = Vec::new();
        for task in &recent {
            if task.status.eq_ignore_ascii_case("pending") {
                if let Ok(created) = DateTime::parse_from_rfc3339(&task.created_at) {
                    if created.with_timezone(&Utc) >= self.request_created_at {
                        if !self.processed_task_ids.lock().await.contains(&task.id) {
                            pending.push(task.clone());
                        }
                    }
                }
            }
        }
        if pending.is_empty() {
            return Ok(0);
        }
        pending.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        self.ensure_busy_state().await?;
        for task in &pending {
            match self.process_task(task).await {
                Ok(()) => {
                    let mut processed = self.processed_task_ids.lock().await;
                    processed.insert(task.id.clone());
                }
                Err(err) => {
                    warn!("Deferring task {} due to error: {}", task.id, err);
                }
            }
        }
        if let Err(e) = self.api_client.update_sandbox_to_idle().await {
            warn!("Failed to set sandbox idle state: {}", e);
        }
        Ok(pending.len())
    }

    async fn ensure_busy_state(&self) -> Result<()> {
        let mut attempt: u32 = 0;
        loop {
            match self.api_client.update_sandbox_to_busy().await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    attempt += 1;
                    warn!("Failed to set busy (attempt {}): {}", attempt, e);
                    if attempt >= 3 {
                        return Err(HostError::Api(format!(
                            "Failed to set sandbox busy after {} attempts: {}",
                            attempt, e
                        )));
                    }
                    tokio::time::sleep(std::time::Duration::from_millis((attempt * 200) as u64))
                        .await;
                }
            }
        }
    }

    async fn process_task(&self, task: &TaskView) -> Result<()> {
        let input_text = extract_first_text(&task.input_content);
        self.guardrails.validate_input(&input_text)?;

        let mut conversation = Vec::new();
        if let Some(msg) = render_task_input(task) {
            conversation.push(msg);
        }

        loop {
            if !self.is_task_active(&task.id).await? {
                return Ok(());
            }

            let system_prompt = self.build_system_prompt().await;
            let response = match self
                .inference_client
                .complete(conversation.clone(), Some(system_prompt))
                .await
            {
                Ok(resp) => resp,
                Err(e) => return Err(e),
            };

            if let Some(total_tokens) = response.total_tokens {
                if let Err(e) = self
                    .api_client
                    .update_sandbox_context_length(total_tokens)
                    .await
                {
                    warn!("Failed to update context length: {}", e);
                }
            }

            let raw = response.content.trim();
            if raw.is_empty() {
                Self::push_system_note(
                    &mut conversation,
                    "Developer note: Response must be a single XML command. Try again.",
                );
                continue;
            }

            let command = match parse_command_xml(raw) {
                Ok(cmd) => cmd,
                Err(err) => {
                    warn!("Invalid XML from model: {}", err);
                    Self::push_system_note(
                        &mut conversation,
                        "Developer note: The reply was not valid XML. Respond with exactly one command such as <run_bash .../>.",
                    );
                    continue;
                }
            };

            let command_name = command.name.to_lowercase();
            let command_text = raw.to_string();
            conversation.push(ChatMessage {
                role: "assistant".to_string(),
                content: command_text.clone(),
                name: None,
                tool_call_id: None,
            });

            if command_name == "output" {
                let final_text = command
                    .body
                    .unwrap_or_else(|| String::from("No content provided."));
                let sanitized = self.guardrails.validate_output(&final_text)?;
                let segment = json!({
                    "type": "final",
                    "tool": "output",
                    "content": sanitized,
                });
                let _ = self
                    .api_client
                    .update_task(
                        &task.id,
                        Some("completed".to_string()),
                        Some(sanitized.clone()),
                        Some(vec![segment]),
                    )
                    .await;
                conversation.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: sanitized,
                    name: None,
                    tool_call_id: None,
                });
                return Ok(());
            }

            if !self.toolkit.has(&command_name) {
                let allowed = self.toolkit.known_tools().join(", ");
                Self::push_system_note(
                    &mut conversation,
                    format!(
                        "Developer note: Unknown command '{}'. Use one of: {} or <output>.",
                        command_name, allowed
                    ),
                );
                continue;
            }

            let ExecutionResult { args, output } =
                match self.toolkit.execute_invocation(&command).await {
                    Ok(result) => result,
                    Err(err) => {
                        warn!("Tool execution error: {}", err);
                        Self::push_system_note(
                            &mut conversation,
                            format!("Developer note: Tool error – {}.", err),
                        );
                        continue;
                    }
                };

            let mut truncated = false;
            let preview_text =
                truncate_output_preview(&output, MAX_TOOL_OUTPUT_CHARS, &mut truncated);

            let tool_call_segment = json!({
                "type": "tool_call",
                "tool": command_name,
                "xml": command_text,
                "arguments": args,
            });
            let tool_result_segment = json!({
                "type": "tool_result",
                "tool": command_name,
                "output": output.clone(),
                "preview": preview_text,
                "truncated": truncated,
            });

            let _ = self
                .api_client
                .update_task(
                    &task.id,
                    Some("processing".to_string()),
                    None,
                    Some(vec![tool_call_segment.clone(), tool_result_segment.clone()]),
                )
                .await;

            let arguments_xml = if args.is_null() {
                String::new()
            } else {
                let body = value_to_xml(&args);
                if body.is_empty() {
                    String::new()
                } else {
                    format!("<arguments>{}</arguments>", body)
                }
            };

            let output_xml = if output.is_null() {
                String::new()
            } else {
                let body = value_to_xml(&output);
                if body.is_empty() {
                    String::new()
                } else {
                    format!("<output>{}</output>", body)
                }
            };

            let preview_xml = if preview_text.is_empty() {
                String::new()
            } else {
                format!(
                    "<preview truncated=\"{}\"><![CDATA[{}]]></preview>",
                    truncated,
                    escape_cdata(&preview_text)
                )
            };

            let combined_segments = format!("{arguments_xml}{output_xml}{preview_xml}");
            let result_message = format!(
                "<tool_result tool=\"{}\">{}</tool_result>",
                command_name, combined_segments
            );

            conversation.push(ChatMessage {
                role: "tool".to_string(),
                content: result_message,
                name: None,
                tool_call_id: None,
            });

            if command_name == "output" {
                let fallback = command.body.as_deref().unwrap_or("No content provided.");
                let summary_raw = render_output_summary(&output, fallback);
                let sanitized = self.guardrails.validate_output(&summary_raw)?;
                let final_segment = json!({
                    "type": "final",
                    "tool": "output",
                    "content": sanitized,
                });

                let _ = self
                    .api_client
                    .update_task(
                        &task.id,
                        Some("completed".to_string()),
                        Some(sanitized.clone()),
                        Some(vec![
                            tool_call_segment.clone(),
                            tool_result_segment.clone(),
                            final_segment.clone(),
                        ]),
                    )
                    .await;

                conversation.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: sanitized.clone(),
                    name: None,
                    tool_call_id: None,
                });
                return Ok(());
            }

            Self::push_system_note(
                &mut conversation,
                "Developer note: Tool execution finished. Respond with another XML command; only use <output> when you are ready to deliver the final result.",
            );
        }
    }

    async fn is_task_active(&self, task_id: &str) -> Result<bool> {
        match self.api_client.get_task_by_id(task_id).await {
            Ok(task) => {
                let status = task.status.to_lowercase();
                Ok(status == "pending" || status == "processing")
            }
            Err(err) => {
                warn!("Failed to fetch task {}: {}", task_id, err);
                Ok(false)
            }
        }
    }

    async fn build_system_prompt(&self) -> String {
        let host_name =
            std::env::var("TSBX_HOST_NAME").unwrap_or_else(|_| "TaskSandbox".to_string());
        let sandbox_id = match self.api_client.get_sandbox().await {
            Ok(sandbox) => sandbox.id,
            Err(_) => "unknown".to_string(),
        };
        let current_time_utc = chrono::Utc::now().to_rfc3339();

        let mut prompt = String::new();
        prompt.push_str(&format!(
            "You are TaskSandbox, a secure delegated workspace that other agents invoke to execute end-to-end tasks.\n\
You operate inside the {host_name} environment, running within an isolated container that persists context across steps.\n\
Current UTC time: {current_time_utc}\nSandbox ID: {sandbox_id}\n\n"
        ));
        prompt.push_str("Agents reach you through standard APIs or MCP; treat each session as part of a coordinated workflow that keeps tool activity grouped while minimizing unnecessary external data transfer.\n");
        prompt.push_str("Use the built-in helpers for filesystem, shell execution, and browser automation to perform work locally, keeping sensitive data inside the sandbox whenever possible.\n");
        prompt.push_str("You pair well with open-source language models; provide precise, tool-centric responses that help them delegate reliably.\n\n");
        prompt.push_str("Follow these rules:\n");
        prompt.push_str("- Always respond with exactly ONE XML element (a tool command). Plain text responses are forbidden.\n");
        prompt.push_str("- Communicate final answers back to the AI Agent exclusively via the `<output>` tool call. Do not use `<output>` for intermediate status updates.\n");
        prompt.push_str("- Keep `commentary` attributes short (gerund form: \"Inspecting…\").\n");
        prompt.push_str("- Use only the commands listed below; do not invent new tool names.\n");
        prompt.push_str("- Continue issuing commands until the task is complete, then send a single `<output>` summarizing the result or question.\n");
        prompt.push_str("- When you receive a `<tool_result>` message, use its information to decide your next command.\n");
        prompt.push_str("- All file paths must stay under /sandbox.\n");
        prompt.push_str("- Never ask the user to run commands; you execute them via the tools.\n");
        prompt.push_str("- Prefer incremental edits: open -> edit -> verify.\n\n");

        prompt.push_str(&self.toolkit.command_catalog_prompt());
        prompt.push_str(
            "\n\nExample final response:\n<output><![CDATA[All tasks complete. Tests pass.]]></output>\n",
        );

        prompt
    }

    fn push_system_note(conversation: &mut Vec<ChatMessage>, text: impl Into<String>) {
        conversation.push(ChatMessage {
            role: "system".to_string(),
            content: text.into(),
            name: None,
            tool_call_id: None,
        });
    }
}

fn extract_first_text(items: &[Value]) -> String {
    for item in items {
        if item
            .get("type")
            .and_then(|t| t.as_str())
            .map(|t| t.eq_ignore_ascii_case("text"))
            .unwrap_or(false)
        {
            if let Some(content) = item.get("content").and_then(|c| c.as_str()) {
                return content.to_string();
            }
        }
    }
    String::new()
}

fn render_task_input(task: &TaskView) -> Option<ChatMessage> {
    let mut parts = Vec::new();
    for item in &task.input_content {
        if item
            .get("type")
            .and_then(|t| t.as_str())
            .map(|t| t.eq_ignore_ascii_case("text"))
            .unwrap_or(false)
        {
            if let Some(content) = item.get("content").and_then(|c| c.as_str()) {
                parts.push(content.trim().to_string());
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(ChatMessage {
            role: "user".to_string(),
            content: parts.join("\n\n"),
            name: None,
            tool_call_id: None,
        })
    }
}

fn render_output_summary(output: &Value, fallback: &str) -> String {
    if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
        if items.is_empty() {
            return fallback.to_string();
        }
        let mut sections = Vec::new();
        for item in items {
            let typ = item
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let title = item
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Result");
            match typ.as_str() {
                "markdown" => {
                    if let Some(content) = item.get("content").and_then(|v| v.as_str()) {
                        sections.push(format!("## {}\n\n{}", title, content.trim()));
                    }
                }
                "text" => {
                    if let Some(content) = item.get("content").and_then(|v| v.as_str()) {
                        sections.push(format!("**{}**\n\n{}", title, content.trim()));
                    }
                }
                "json" => {
                    let pretty = item
                        .get("content")
                        .map(|v| serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()))
                        .unwrap_or_else(|| "null".to_string());
                    sections.push(format!("## {}\n\n```json\n{}\n```", title, pretty));
                }
                "url" => {
                    if let Some(url) = item.get("content").and_then(|v| v.as_str()) {
                        sections.push(format!("- [{}]({})", title, url.trim()));
                    }
                }
                _ => {
                    if let Some(content) = item.get("content").and_then(|v| v.as_str()) {
                        sections.push(format!("{}: {}", title, content.trim()));
                    }
                }
            }
        }
        if sections.is_empty() {
            fallback.to_string()
        } else {
            sections.join("\n\n")
        }
    } else {
        fallback.to_string()
    }
}

fn truncate_output_preview(value: &Value, max_chars: usize, truncated: &mut bool) -> String {
    let mut text = match value {
        Value::String(s) => s.clone(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
    };
    if text.len() > max_chars {
        text.truncate(max_chars);
        text.push_str("...[truncated]");
        *truncated = true;
    }
    text
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn escape_cdata(text: &str) -> String {
    text.replace("]]>", "]]]]><![CDATA[>")
}

fn sanitize_tag(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        let valid = if i == 0 {
            ch.is_ascii_alphabetic() || ch == '_'
        } else {
            ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.'
        };
        if valid {
            result.push(ch);
        } else {
            result.push('_');
        }
    }
    if result.is_empty() {
        "_".to_string()
    } else {
        result
    }
}

fn value_to_xml(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => escape_xml(&b.to_string()),
        Value::Number(n) => escape_xml(&n.to_string()),
        Value::String(s) => escape_xml(s),
        Value::Array(items) => {
            let mut parts = String::new();
            for item in items {
                let body = value_to_xml(item);
                if body.is_empty() {
                    parts.push_str("<item />");
                } else {
                    parts.push_str("<item>");
                    parts.push_str(&body);
                    parts.push_str("</item>");
                }
            }
            parts
        }
        Value::Object(map) => {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            let mut parts = String::new();
            for key in keys {
                let tag = sanitize_tag(&key);
                let body = value_to_xml(map.get(&key).unwrap());
                if body.is_empty() {
                    parts.push_str(&format!("<{tag} />"));
                } else {
                    parts.push_str(&format!("<{tag}>{body}</{tag}>"));
                }
            }
            parts
        }
    }
}
