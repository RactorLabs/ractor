use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentResponse {
    pub id: String,
    pub agent_name: String,
    pub created_by: String,
    pub status: String, // pending | processing | completed | failed
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResponseRequest {
    pub input: serde_json::Value, // { text: string }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResponseRequest {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub input: Option<serde_json::Value>,
    #[serde(default)]
    pub output: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseView {
    pub id: String,
    pub agent_name: String,
    pub status: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

impl AgentResponse {
    pub async fn create(
        pool: &sqlx::MySqlPool,
        agent_name: &str,
        created_by: &str,
        req: CreateResponseRequest,
    ) -> Result<AgentResponse, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let status = "pending".to_string();
        // Initialize output with empty structure
        let initial_output = serde_json::json!({
            "text": "",
            "items": []
        });
        // sqlx JSON binding wants Value; already ok

        sqlx::query(
            r#"
            INSERT INTO agent_responses (id, agent_name, created_by, status, input, output, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&id)
        .bind(agent_name)
        .bind(created_by)
        .bind(&status)
        .bind(&req.input)
        .bind(&initial_output)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        Ok(AgentResponse {
            id,
            agent_name: agent_name.to_string(),
            created_by: created_by.to_string(),
            status,
            input: req.input,
            output: initial_output,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn find_by_agent(
        pool: &sqlx::MySqlPool,
        agent_name: &str,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<AgentResponse>, sqlx::Error> {
        let limit = limit.unwrap_or(100).min(1000);
        let offset = offset.unwrap_or(0);
        sqlx::query_as::<_, AgentResponse>(
            r#"
            SELECT id, agent_name, created_by, status, input, output, created_at, updated_at
            FROM agent_responses
            WHERE agent_name = ?
            ORDER BY created_at ASC
            LIMIT ? OFFSET ?
            "#
        )
        .bind(agent_name)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }

    pub async fn count_by_agent(pool: &sqlx::MySqlPool, agent_name: &str) -> Result<i64, sqlx::Error> {
        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM agent_responses WHERE agent_name = ?",
        )
        .bind(agent_name)
        .fetch_one(pool)
        .await?;
        Ok(result)
    }

    pub async fn find_by_id(pool: &sqlx::MySqlPool, id: &str) -> Result<Option<AgentResponse>, sqlx::Error> {
        sqlx::query_as::<_, AgentResponse>(
            r#"SELECT id, agent_name, created_by, status, input, output, created_at, updated_at FROM agent_responses WHERE id = ?"#
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    pub async fn update_by_id(
        pool: &sqlx::MySqlPool,
        id: &str,
        req: UpdateResponseRequest,
    ) -> Result<AgentResponse, sqlx::Error> {
        // Load existing
        let mut resp = Self::find_by_id(pool, id).await?.ok_or_else(|| sqlx::Error::RowNotFound)?;

        if let Some(s) = req.status { resp.status = s; }
        if let Some(i) = req.input { resp.input = i; }
        if let Some(o) = req.output {
            // Merge output with append semantics for items
            use serde_json::{Map, Value};
            let mut merged = match resp.output {
                Value::Object(map) => map,
                _ => Map::new(),
            };

            // Merge text (replace if provided)
            if let Some(t) = o.get("text") {
                merged.insert("text".to_string(), t.clone());
            }

            // Append items if provided
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

            // Carry over any other fields provided in output
            for (k, v) in o.as_object().unwrap_or(&Map::new()) {
                if k != "text" && k != "items" {
                    merged.insert(k.clone(), v.clone());
                }
            }

            resp.output = serde_json::Value::Object(merged);
        }

        let now = Utc::now();
        sqlx::query(
            r#"UPDATE agent_responses SET status=?, input=?, output=?, updated_at=? WHERE id = ?"#
        )
        .bind(&resp.status)
        .bind(&resp.input)
        .bind(&resp.output)
        .bind(&now)
        .bind(&resp.id)
        .execute(pool)
        .await?;
        resp.updated_at = now;
        Ok(resp)
    }
}
