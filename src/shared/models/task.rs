use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskOutput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commentary: Option<String>,
    #[serde(default)]
    pub items: Vec<Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TaskType {
    NL,
    SH,
    PY,
    JS,
}

impl TaskType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            TaskType::NL => "NL",
            TaskType::SH => "SH",
            TaskType::PY => "PY",
            TaskType::JS => "JS",
        }
    }

    pub fn from_db_value(value: &str) -> Self {
        TaskType::from_str(value).unwrap_or(TaskType::NL)
    }
}

impl Default for TaskType {
    fn default() -> Self {
        TaskType::NL
    }
}

impl fmt::Display for TaskType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TaskType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "NL" => Ok(TaskType::NL),
            "SH" => Ok(TaskType::SH),
            "PY" => Ok(TaskType::PY),
            "JS" => Ok(TaskType::JS),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SandboxTask {
    pub id: String,
    pub sandbox_id: String,
    pub created_by: String,
    pub status: String,
    pub task_type: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub steps: serde_json::Value,
    pub context_length: i64,
    pub timeout_seconds: Option<i32>,
    pub timeout_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub input: serde_json::Value,
    #[serde(default)]
    pub background: Option<bool>,
    #[serde(default)]
    pub timeout_seconds: Option<i32>,
    #[serde(default)]
    pub task_type: Option<TaskType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub id: String,
    pub sandbox_id: String,
    pub status: String,
    pub task_type: TaskType,
    #[serde(default)]
    pub input: Vec<Value>,
    #[serde(default)]
    pub output: TaskOutput,
    pub context_length: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskRequest {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub input: Option<serde_json::Value>,
    #[serde(default)]
    pub output: Option<Vec<Value>>,
    #[serde(default)]
    pub steps: Option<Vec<Value>>,
    #[serde(default)]
    pub timeout_seconds: Option<i32>,
    #[serde(default)]
    pub context_length: Option<i64>,
    #[serde(default)]
    pub prompt_tokens_delta: Option<i64>,
    #[serde(default)]
    pub completion_tokens_delta: Option<i64>,
    #[serde(default)]
    pub tool_used: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskView {
    pub id: String,
    pub sandbox_id: String,
    pub status: String,
    pub task_type: TaskType,
    #[serde(default)]
    pub input: Vec<Value>,
    #[serde(default)]
    pub steps: Vec<Value>,
    #[serde(default)]
    pub output: TaskOutput,
    pub context_length: i64,
    pub timeout_seconds: Option<i32>,
    pub timeout_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl SandboxTask {
    pub async fn create(
        pool: &sqlx::MySqlPool,
        sandbox_id: &str,
        created_by: &str,
        req: CreateTaskRequest,
    ) -> Result<SandboxTask, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let status = "queued".to_string();
        let task_type = req.task_type.unwrap_or_default();
        let timeout_seconds = req.timeout_seconds.filter(|v| *v > 0);
        let timeout_at = timeout_seconds.map(|secs| now + chrono::Duration::seconds(secs as i64));
        let initial_output = serialize_task_output(&TaskOutput::default());
        let initial_steps = serde_json::json!([]);

        sqlx::query(
            r#"
            INSERT INTO sandbox_tasks (id, sandbox_id, created_by, status, task_type, input, output, steps, context_length, timeout_seconds, timeout_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&id)
        .bind(sandbox_id)
        .bind(created_by)
        .bind(&status)
        .bind(task_type.as_str())
        .bind(&req.input)
        .bind(&initial_output)
        .bind(&initial_steps)
        .bind(0_i64)
        .bind(timeout_seconds)
        .bind(timeout_at)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        Ok(SandboxTask {
            id,
            sandbox_id: sandbox_id.to_string(),
            created_by: created_by.to_string(),
            status,
            task_type: task_type.as_str().to_string(),
            input: req.input,
            output: initial_output,
            steps: initial_steps,
            context_length: 0,
            timeout_seconds,
            timeout_at,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn find_by_sandbox(
        pool: &sqlx::MySqlPool,
        sandbox_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<SandboxTask>, sqlx::Error> {
        let limit = limit.unwrap_or(100).min(1000);
        let offset = offset.unwrap_or(0);
        sqlx::query_as::<_, SandboxTask>(
            r#"
            SELECT id, sandbox_id, created_by, status, task_type, input, output, steps, context_length, timeout_seconds, timeout_at, created_at, updated_at
            FROM sandbox_tasks
            WHERE sandbox_id = ?
            ORDER BY created_at ASC, id ASC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(sandbox_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }

    pub async fn count_by_sandbox(
        pool: &sqlx::MySqlPool,
        sandbox_id: &str,
    ) -> Result<i64, sqlx::Error> {
        let result =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sandbox_tasks WHERE sandbox_id = ?")
                .bind(sandbox_id)
                .fetch_one(pool)
                .await?;
        Ok(result)
    }

    pub async fn latest_context_length(
        pool: &sqlx::MySqlPool,
        sandbox_id: &str,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT context_length
            FROM sandbox_tasks
            WHERE sandbox_id = ?
            ORDER BY updated_at DESC, created_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(sandbox_id)
        .fetch_optional(pool)
        .await?;

        Ok(result.unwrap_or(0))
    }

    pub async fn find_by_id(
        pool: &sqlx::MySqlPool,
        id: &str,
    ) -> Result<Option<SandboxTask>, sqlx::Error> {
        sqlx::query_as::<_, SandboxTask>(
            r#"SELECT id, sandbox_id, created_by, status, task_type, input, output, steps, context_length, timeout_seconds, timeout_at, created_at, updated_at FROM sandbox_tasks WHERE id = ?"#
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    pub async fn update_by_id(
        pool: &sqlx::MySqlPool,
        id: &str,
        req: UpdateTaskRequest,
    ) -> Result<SandboxTask, sqlx::Error> {
        let mut task = Self::find_by_id(pool, id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        if let Some(s) = req.status {
            task.status = s;
        }
        if let Some(i) = req.input {
            task.input = i;
        }
        if let Some(output_items) = req.output {
            let normalized = normalize_output_items(output_items);
            task.output = Value::Array(normalized);
        }

        if let Some(new_steps) = req.steps {
            let mut existing = task.steps.as_array().cloned().unwrap_or_else(Vec::new);
            existing.extend(new_steps);
            task.steps = serde_json::Value::Array(existing);
        }
        if let Some(context_length) = req.context_length {
            task.context_length = if context_length < 0 {
                0
            } else {
                context_length
            };
        }

        let mut timeout_updated = false;
        if let Some(timeout) = req.timeout_seconds {
            timeout_updated = true;
            if timeout > 0 {
                task.timeout_seconds = Some(timeout);
            } else {
                task.timeout_seconds = None;
            }
        }

        let now = Utc::now();
        if timeout_updated {
            if let Some(timeout) = task.timeout_seconds {
                task.timeout_at = Some(now + chrono::Duration::seconds(timeout as i64));
            } else {
                task.timeout_at = None;
            }
        }
        sqlx::query(
            r#"UPDATE sandbox_tasks SET status=?, input=?, output=?, steps=?, context_length=?, timeout_seconds=?, timeout_at=?, updated_at=? WHERE id = ?"#,
        )
        .bind(&task.status)
        .bind(&task.input)
        .bind(&task.output)
        .bind(&task.steps)
        .bind(task.context_length)
        .bind(task.timeout_seconds)
        .bind(task.timeout_at)
        .bind(&now)
        .bind(&task.id)
        .execute(pool)
        .await?;
        task.updated_at = now;
        Ok(task)
    }
}

pub fn extract_steps(value: &serde_json::Value) -> Vec<serde_json::Value> {
    value.as_array().cloned().unwrap_or_default()
}

pub fn extract_output_items(value: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(arr) = value.as_array() {
        return normalize_output_items(arr.clone());
    }
    if let Some(obj) = value.as_object() {
        let mut items = Vec::new();
        // Extract commentary field first
        if let Some(commentary) = obj.get("commentary") {
            if let Some(commentary_str) = commentary.as_str() {
                if !commentary_str.trim().is_empty() {
                    items.push(json!({ "type": "commentary", "content": commentary_str }));
                }
            }
        }
        if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
            items.push(json!({ "type": "md", "content": text }));
        }
        if let Some(content_arr) = obj.get("content").and_then(|v| v.as_array()) {
            items.extend(content_arr.clone());
        }
        return normalize_output_items(items);
    }
    Vec::new()
}

pub fn normalize_output_items(items: Vec<Value>) -> Vec<Value> {
    items
        .into_iter()
        .filter_map(|item| normalize_output_item(item))
        .collect()
}

fn normalize_output_item(item: Value) -> Option<Value> {
    match item {
        Value::Object(mut map) => {
            let raw_type = map
                .remove("type")
                .and_then(|v| v.as_str().map(|s| s.trim().to_lowercase()))
                .unwrap_or_else(|| "text".to_string());
            let normalized_type = canonical_output_type(&raw_type);
            let content_value = map.remove("content");
            match normalized_type {
                "json" => {
                    let content = content_value.unwrap_or(Value::Null);
                    let mut normalized = serde_json::Map::new();
                    normalized.insert("type".into(), Value::String(normalized_type.to_string()));
                    normalized.insert("content".into(), content);
                    if let Some(title) = map.remove("title") {
                        normalized.insert("title".into(), title);
                    }
                    for (k, v) in map {
                        normalized.insert(k, v);
                    }
                    Some(Value::Object(normalized))
                }
                _ => {
                    let text = match content_value {
                        Some(Value::String(s)) => s,
                        Some(Value::Object(obj)) => {
                            // Try to extract string content from nested object
                            if let Some(nested_content) = obj.get("content") {
                                if let Some(s) = nested_content.as_str() {
                                    s.to_string()
                                } else {
                                    // Serialize nested content as proper JSON
                                    serde_json::to_string(nested_content).unwrap_or_default()
                                }
                            } else if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                                text.to_string()
                            } else {
                                // Serialize entire object as proper JSON
                                serde_json::to_string(&Value::Object(obj)).unwrap_or_default()
                            }
                        }
                        Some(Value::Array(arr)) => {
                            // Serialize array as proper JSON
                            serde_json::to_string(&Value::Array(arr)).unwrap_or_default()
                        }
                        Some(other) => {
                            // For primitives, convert to string
                            serde_json::to_string(&other).unwrap_or_else(|_| other.to_string())
                        }
                        None => String::new(),
                    };
                    let mut normalized = serde_json::Map::new();
                    normalized.insert("type".into(), Value::String(normalized_type.to_string()));
                    normalized.insert("content".into(), Value::String(text));
                    if let Some(title) = map.remove("title") {
                        normalized.insert("title".into(), title);
                    }
                    for (k, v) in map {
                        normalized.insert(k, v);
                    }
                    Some(Value::Object(normalized))
                }
            }
        }
        Value::String(s) => Some(json!({ "type": "text", "content": s })),
        Value::Number(n) => Some(json!({ "type": "text", "content": n.to_string() })),
        Value::Bool(b) => Some(json!({ "type": "text", "content": b.to_string() })),
        _ => None,
    }
}

fn canonical_output_type(raw: &str) -> &'static str {
    match raw {
        "markdown" | "md" => "md",
        "json" => "json",
        "stdout" => "stdout",
        "stderr" => "stderr",
        "exit_code" => "exit_code",
        "commentary" => "commentary",
        _ => "text",
    }
}

fn serialize_task_output(output: &TaskOutput) -> Value {
    if output.items.is_empty() {
        Value::Array(vec![])
    } else {
        Value::Array(output.items.clone())
    }
}
