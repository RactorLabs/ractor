use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SandboxTask {
    pub id: String,
    pub sandbox_id: String,
    pub created_by: String,
    pub status: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskRequest {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub input: Option<serde_json::Value>,
    #[serde(default)]
    pub output: Option<serde_json::Value>,
    #[serde(default)]
    pub timeout_seconds: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskView {
    pub id: String,
    pub sandbox_id: String,
    pub status: String,
    #[serde(default)]
    pub input_content: Vec<Value>,
    #[serde(default)]
    pub output_content: Vec<Value>,
    #[serde(default)]
    pub segments: Vec<Value>,
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

        let status = "pending".to_string();
        let timeout_seconds = req.timeout_seconds.filter(|v| *v > 0);
        let timeout_at = timeout_seconds.map(|secs| now + chrono::Duration::seconds(secs as i64));
        let initial_output = serde_json::json!({
            "text": "",
            "items": [],
            "content": []
        });

        sqlx::query(
            r#"
            INSERT INTO sandbox_tasks (id, sandbox_id, created_by, status, input, output, timeout_seconds, timeout_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&id)
        .bind(sandbox_id)
        .bind(created_by)
        .bind(&status)
        .bind(&req.input)
        .bind(&initial_output)
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
            input: req.input,
            output: initial_output,
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
            SELECT id, sandbox_id, created_by, status, input, output, timeout_seconds, timeout_at, created_at, updated_at
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

    pub async fn find_by_id(
        pool: &sqlx::MySqlPool,
        id: &str,
    ) -> Result<Option<SandboxTask>, sqlx::Error> {
        sqlx::query_as::<_, SandboxTask>(
            r#"SELECT id, sandbox_id, created_by, status, input, output, timeout_seconds, timeout_at, created_at, updated_at FROM sandbox_tasks WHERE id = ?"#
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
        if let Some(o) = req.output {
            use serde_json::{Map, Value};
            let mut merged = match task.output {
                Value::Object(map) => map,
                _ => Map::new(),
            };

            if let Some(t) = o.get("text") {
                merged.insert("text".to_string(), t.clone());
            }

            if let Some(new_items_val) = o.get("items") {
                let mut items: Vec<Value> = merged
                    .get("items")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_else(Vec::new);
                if let Some(to_append) = new_items_val.as_array() {
                    items.extend(to_append.iter().cloned());
                }
                merged.insert("items".to_string(), Value::Array(items));
            }

            for (k, v) in o.as_object().unwrap_or(&Map::new()) {
                if k != "text" && k != "items" {
                    merged.insert(k.clone(), v.clone());
                }
            }

            let mut merged_value = serde_json::Value::Object(merged);
            let content_items = compute_output_content(&merged_value);
            if let serde_json::Value::Object(ref mut obj) = merged_value {
                if content_items.is_empty() {
                    obj.remove("content");
                } else {
                    obj.insert(
                        "content".to_string(),
                        serde_json::Value::Array(content_items),
                    );
                }
            }

            task.output = merged_value;
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
            r#"UPDATE sandbox_tasks SET status=?, input=?, output=?, timeout_seconds=?, timeout_at=?, updated_at=? WHERE id = ?"#,
        )
        .bind(&task.status)
        .bind(&task.input)
        .bind(&task.output)
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

pub fn compute_output_content(output: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
        for it in items.iter().rev() {
            if it.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                && it.get("tool").and_then(|v| v.as_str()) == Some("output")
            {
                if let Some(arr) = it
                    .get("output")
                    .and_then(|v| v.get("items"))
                    .and_then(|v| v.as_array())
                {
                    return arr.clone();
                }
            }
        }
        for it in items.iter().rev() {
            if it.get("type").and_then(|v| v.as_str()) == Some("tool_call")
                && it.get("tool").and_then(|v| v.as_str()) == Some("output")
            {
                if let Some(arr) = it
                    .get("arguments")
                    .or_else(|| it.get("args"))
                    .and_then(|v| v.get("content"))
                    .and_then(|v| v.as_array())
                {
                    return arr.clone();
                }
            }
        }
    }
    Vec::new()
}
