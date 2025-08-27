use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SessionMessage {
    pub id: String,
    pub session_id: String,
    pub created_by: String,
    pub role: String,
    pub content: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageRequest {
    pub role: String,
    pub content: String,
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ListMessagesQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    #[allow(dead_code)]
    pub role: Option<String>,
    #[allow(dead_code)]
    pub since: Option<DateTime<Utc>>,
}

fn default_metadata() -> serde_json::Value {
    serde_json::json!({})
}

impl SessionMessage {
    pub async fn create(
        pool: &sqlx::MySqlPool,
        session_id: &str,
        created_by: &str,
        req: CreateMessageRequest,
    ) -> Result<SessionMessage, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        
        sqlx::query(
            r#"
            INSERT INTO session_messages (id, session_id, created_by, role, content, metadata, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&id)
        .bind(session_id)
        .bind(created_by)
        .bind(&req.role)
        .bind(&req.content)
        .bind(&req.metadata)
        .bind(&now)
        .execute(pool)
        .await?;
        
        Ok(SessionMessage {
            id,
            session_id: session_id.to_string(),
            created_by: created_by.to_string(),
            role: req.role,
            content: req.content,
            metadata: req.metadata,
            created_at: now,
        })
    }

    #[allow(dead_code)]
    pub async fn find_by_session(
        pool: &sqlx::MySqlPool,
        session_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<SessionMessage>, sqlx::Error> {
        let limit = limit.unwrap_or(100).min(1000);  // Max 1000 messages
        let offset = offset.unwrap_or(0);
        
        sqlx::query_as::<_, SessionMessage>(
            r#"
            SELECT id, session_id, created_by, role, content,
                   metadata, created_at
            FROM session_messages
            WHERE session_id = ?
            ORDER BY created_at ASC
            LIMIT ? OFFSET ?
            "#
        )
        .bind(session_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }

    #[allow(dead_code)]
    pub async fn find_by_session_with_filter(
        pool: &sqlx::MySqlPool,
        session_id: &str,
        query: ListMessagesQuery,
    ) -> Result<Vec<SessionMessage>, sqlx::Error> {
        let limit = query.limit.unwrap_or(100).min(1000);
        let offset = query.offset.unwrap_or(0);
        
        let mut sql = String::from(
            r#"
            SELECT id, session_id, created_by, role, content,
                   metadata, created_at
            FROM session_messages
            WHERE session_id = ?
            "#
        );
        
        let mut param_count = 1;
        
        if query.role.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND role = ${param_count}"));
        }
        
        if query.since.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND created_at > ${param_count}"));
        }
        
        sql.push_str(" ORDER BY created_at ASC");
        param_count += 1;
        sql.push_str(&format!(" LIMIT ${param_count}"));
        param_count += 1;
        sql.push_str(&format!(" OFFSET ${param_count}"));
        
        let mut query_builder = sqlx::query_as::<_, SessionMessage>(&sql)
            .bind(session_id);
        
        if let Some(role) = query.role {
            query_builder = query_builder.bind(role);
        }
        
        if let Some(since) = query.since {
            query_builder = query_builder.bind(since);
        }
        
        query_builder
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
    }

    pub async fn count_by_session(
        pool: &sqlx::MySqlPool,
        session_id: &str,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM session_messages WHERE session_id = ?"
        )
        .bind(session_id)
        .fetch_one(pool)
        .await?;
        
        Ok(result)
    }

    pub async fn delete_by_session(
        pool: &sqlx::MySqlPool,
        session_id: &str,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM session_messages WHERE session_id = ?"#)
        .bind(session_id)
        .execute(pool)
        .await?;
        
        Ok(result.rows_affected())
    }
}