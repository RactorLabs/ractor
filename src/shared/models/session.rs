use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use super::constants::SESSION_STATE_DELETED;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: String,
    pub created_by: String,
    pub name: Option<String>,
    pub state: String,
    pub container_id: Option<String>,
    pub persistent_volume_id: Option<String>,
    pub parent_session_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub secrets: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub setup: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemixSessionRequest {
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "default_true")]
    pub data: bool,
    #[serde(default = "default_true")]
    pub code: bool,
    #[serde(default = "default_true")]
    pub secrets: bool,
    #[serde(default)]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSessionStateRequest {
    pub state: String,
    #[serde(default)]
    pub container_id: Option<String>,
    #[serde(default)]
    pub persistent_volume_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSessionRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreSessionRequest {
    #[serde(default)]
    pub prompt: Option<String>,
}

fn default_metadata() -> serde_json::Value {
    serde_json::json!({})
}

fn default_true() -> bool {
    true
}



// Database queries
impl Session {
    pub async fn find_all(pool: &sqlx::MySqlPool) -> Result<Vec<Session>, sqlx::Error> {
        sqlx::query_as::<_, Session>(
            r#"
            SELECT id, created_by, name, state,
                   container_id, persistent_volume_id, parent_session_id,
                   created_at,  last_activity_at,
                   metadata
            FROM sessions
            WHERE state != 'deleted'
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_id(pool: &sqlx::MySqlPool, id: &str) -> Result<Option<Session>, sqlx::Error> {
        sqlx::query_as::<_, Session>(
            r#"
            SELECT id, created_by, name, state,
                   container_id, persistent_volume_id, parent_session_id,
                   created_at,  last_activity_at,
                   metadata
            FROM sessions
            WHERE id = ? AND state != 'deleted'
            "#
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_name(pool: &sqlx::MySqlPool, name: &str, created_by: &str) -> Result<Option<Session>, sqlx::Error> {
        sqlx::query_as::<_, Session>(
            r#"
            SELECT id, created_by, name, state,
                   container_id, persistent_volume_id, parent_session_id,
                   created_at,  last_activity_at,
                   metadata
            FROM sessions
            WHERE name = ? AND created_by = ? AND state != 'deleted'
            ORDER BY created_at DESC
            LIMIT 1
            "#
        )
        .bind(name)
        .bind(created_by)
        .fetch_optional(pool)
        .await
    }

    pub async fn create(
        pool: &sqlx::MySqlPool,
        req: CreateSessionRequest,
        created_by: &str,
    ) -> Result<Session, sqlx::Error> {
        // Generate UUID for the session
        let session_id = Uuid::new_v4();
        
        // Insert the session
        sqlx::query(
            r#"
            INSERT INTO sessions (id, created_by, name, metadata)
            VALUES (?, ?, ?, ?)
            "#
        )
        .bind(session_id.to_string())
        .bind(created_by)
        .bind(&req.name)
        .bind(&req.metadata)
        .execute(pool)
        .await?;
        
        // Fetch the created session
        let session = Self::find_by_id(pool, &session_id.to_string()).await?.unwrap();

        Ok(session)
    }

    pub async fn remix(
        pool: &sqlx::MySqlPool,
        parent_id: &str,
        req: RemixSessionRequest,
    ) -> Result<Session, sqlx::Error> {
        // Get parent session
        let parent = Self::find_by_id(pool, parent_id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        // Create new session based on parent
        let session_id = Uuid::new_v4();
        
        sqlx::query(
            r#"
            INSERT INTO sessions (
                id, created_by, name,
                parent_session_id, metadata
            )
            VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(session_id.to_string())
        .bind(&parent.created_by) // Inherit created_by from parent
        .bind(&req.name)
        .bind(parent_id)
        .bind(req.metadata.as_ref().unwrap_or(&parent.metadata))
        .execute(pool)
        .await?;
        
        // Fetch the created session
        let session = Self::find_by_id(pool, &session_id.to_string()).await?.unwrap();

        Ok(session)
    }

    #[allow(dead_code)]
    pub async fn update_state(
        pool: &sqlx::MySqlPool,
        id: &str,
        req: UpdateSessionStateRequest,
    ) -> Result<Option<Session>, sqlx::Error> {
        // Check current state and validate transition
        let current = Self::find_by_id(pool, id).await?;
        if let Some(session) = current {
            if !super::state_helpers::can_transition_to(&session.state, &req.state) {
                return Err(sqlx::Error::Protocol(format!(
                    "Invalid state transition from {:?} to {:?}",
                    session.state, req.state
                )));
            }
        } else {
            return Ok(None);
        }

        let now = Utc::now();
        let mut query_builder = String::from("UPDATE sessions SET state = ?, last_activity_at = ?");



        if req.container_id.is_some() {
            query_builder.push_str(", container_id = ?");
        }

        if req.persistent_volume_id.is_some() {
            query_builder.push_str(", persistent_volume_id = ?");
        }

        query_builder.push_str(" WHERE id = ?");

        // Build and execute query
        let mut query = sqlx::query(&query_builder)
            .bind(req.state.clone())
            .bind(now);



        if let Some(container_id) = req.container_id {
            query = query.bind(container_id);
        }

        if let Some(pv_id) = req.persistent_volume_id {
            query = query.bind(pv_id);
        }

        query = query.bind(id);

        let result = query.execute(pool).await?;
        
        if result.rows_affected() > 0 {
            Self::find_by_id(pool, id).await
        } else {
            Ok(None)
        }
    }

    pub async fn update(
        pool: &sqlx::MySqlPool,
        id: &str,
        req: UpdateSessionRequest,
    ) -> Result<Option<Session>, sqlx::Error> {
        let mut query_builder = String::from("UPDATE sessions SET");
        let mut updates = Vec::new();

        if req.name.is_some() {
            updates.push(" name = ?".to_string());
        }


        if req.metadata.is_some() {
            updates.push(" metadata = ?".to_string());
        }

        if updates.is_empty() {
            return Err(sqlx::Error::Protocol("No fields to update".to_string()));
        }

        query_builder.push_str(&updates.join(","));
        query_builder.push_str(" WHERE id = ? AND state != 'deleted'");

        let mut query = sqlx::query(&query_builder);

        if let Some(name) = req.name {
            query = query.bind(name);
        }


        if let Some(metadata) = req.metadata {
            query = query.bind(metadata);
        }

        query = query.bind(id);

        let result = query.execute(pool).await?;
        
        if result.rows_affected() > 0 {
            Self::find_by_id(pool, id).await
        } else {
            Ok(None)
        }
    }

    pub async fn delete(pool: &sqlx::MySqlPool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(r#"UPDATE sessions SET state = ? WHERE id = ? AND state != ?"#
        )
        .bind(SESSION_STATE_DELETED)
        .bind(id)
        .bind(SESSION_STATE_DELETED)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

}