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
    pub is_published: bool,
    pub published_at: Option<DateTime<Utc>>,
    pub published_by: Option<String>,
    pub publish_permissions: serde_json::Value,
    pub timeout_seconds: i32,
    pub auto_close_at: Option<DateTime<Utc>>,
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
    #[serde(default = "default_timeout", deserialize_with = "deserialize_strict_option_i32")]
    pub timeout_seconds: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemixSessionRequest {
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "default_true", deserialize_with = "deserialize_strict_bool_default_true")]
    pub data: bool,
    #[serde(default = "default_true", deserialize_with = "deserialize_strict_bool_default_true")]
    pub code: bool,
    #[serde(default = "default_true", deserialize_with = "deserialize_strict_bool_default_true")]
    pub secrets: bool,
    #[serde(default)]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishSessionRequest {
    #[serde(default = "default_true", deserialize_with = "deserialize_strict_bool_default_true")]
    pub data: bool,
    #[serde(default = "default_true", deserialize_with = "deserialize_strict_bool_default_true")]
    pub code: bool,
    #[serde(default = "default_true", deserialize_with = "deserialize_strict_bool_default_true")]
    pub secrets: bool,
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
    #[serde(default, deserialize_with = "deserialize_strict_option_i32")]
    pub timeout_seconds: Option<i32>,
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

// Custom deserializer for strict boolean validation
fn deserialize_strict_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct StrictBoolVisitor;

    impl<'de> serde::de::Visitor<'de> for StrictBoolVisitor {
        type Value = bool;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a boolean value (true or false)")
        }

        fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(value)
        }

        // Reject all other types
        fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Err(E::custom("expected boolean, found string"))
        }

        fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Err(E::custom("expected boolean, found integer"))
        }

        fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Err(E::custom("expected boolean, found integer"))
        }

        fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Err(E::custom("expected boolean, found number"))
        }
    }

    deserializer.deserialize_bool(StrictBoolVisitor)
}

// Custom deserializer for strict boolean validation with default true
fn deserialize_strict_bool_default_true<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_strict_bool(deserializer)
}

fn default_timeout() -> Option<i32> {
    Some(60) // Default 60 seconds (1 minute) timeout
}

// Custom deserializer for strict optional i32 validation
fn deserialize_strict_option_i32<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Visitor};
    
    struct StrictOptionI32Visitor;

    impl<'de> Visitor<'de> for StrictOptionI32Visitor {
        type Value = Option<i32>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an integer value or null")
        }

        fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Some(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if value >= i32::MIN as i64 && value <= i32::MAX as i64 {
                Ok(Some(value as i32))
            } else {
                Err(E::custom("integer value out of range for i32"))
            }
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if value <= i32::MAX as u64 {
                Ok(Some(value as i32))
            } else {
                Err(E::custom("integer value too large for i32"))
            }
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(None)
        }

        // Reject all other types
        fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Err(E::custom("expected integer or null, found string"))
        }

        fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Err(E::custom("expected integer or null, found boolean"))
        }

        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            // Accept whole numbers that fit in i32 range
            if value.fract() == 0.0 && value >= i32::MIN as f64 && value <= i32::MAX as f64 {
                Ok(Some(value as i32))
            } else {
                Err(E::custom("expected integer value within i32 range"))
            }
        }
    }

    deserializer.deserialize_option(StrictOptionI32Visitor)
}



// Database queries
impl Session {
    pub async fn find_all(pool: &sqlx::MySqlPool) -> Result<Vec<Session>, sqlx::Error> {
        sqlx::query_as::<_, Session>(
            r#"
            SELECT id, created_by, name, state,
                   container_id, persistent_volume_id, parent_session_id,
                   created_at, last_activity_at, metadata,
                   is_published, published_at, published_by, publish_permissions,
                   timeout_seconds, auto_close_at
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
                   created_at, last_activity_at, metadata,
                   is_published, published_at, published_by, publish_permissions,
                   timeout_seconds, auto_close_at
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
                   created_at, last_activity_at, metadata,
                   is_published, published_at, published_by, publish_permissions,
                   timeout_seconds, auto_close_at
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
        
        // Calculate timeout - auto_close_at will be set when session becomes idle
        let timeout = req.timeout_seconds.unwrap_or(60); // Default 60 seconds
        let auto_close_at: Option<DateTime<Utc>> = None; // Will be calculated when session becomes idle

        // Insert the session
        sqlx::query(
            r#"
            INSERT INTO sessions (id, created_by, name, metadata, timeout_seconds, auto_close_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(session_id.to_string())
        .bind(created_by)
        .bind(&req.name)
        .bind(&req.metadata)
        .bind(timeout)
        .bind(auto_close_at)
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
        created_by: &str,
    ) -> Result<Session, sqlx::Error> {
        // Get parent session
        let parent = Self::find_by_id(pool, parent_id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        // Create new session based on parent (inherit timeout)
        let session_id = Uuid::new_v4();
        let auto_close_at: Option<DateTime<Utc>> = None; // Will be calculated when session becomes idle
        
        sqlx::query(
            r#"
            INSERT INTO sessions (
                id, created_by, name,
                parent_session_id, metadata, timeout_seconds, auto_close_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(session_id.to_string())
        .bind(created_by) // Use actual remixer as owner
        .bind(&req.name)
        .bind(parent_id)
        .bind(req.metadata.as_ref().unwrap_or(&parent.metadata))
        .bind(parent.timeout_seconds) // Inherit timeout from parent
        .bind(auto_close_at)
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

        if req.timeout_seconds.is_some() {
            updates.push(" timeout_seconds = ?".to_string());
            updates.push(" auto_close_at = DATE_ADD(COALESCE(last_activity_at, NOW()), INTERVAL ? SECOND)".to_string());
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

        if let Some(timeout_seconds) = req.timeout_seconds {
            query = query.bind(timeout_seconds);
            query = query.bind(timeout_seconds); // For the DATE_ADD calculation
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

    pub async fn publish(
        pool: &sqlx::MySqlPool,
        id: &str,
        published_by: &str,
        req: PublishSessionRequest,
    ) -> Result<Option<Session>, sqlx::Error> {
        let publish_permissions = serde_json::json!({
            "data": req.data,
            "code": req.code,
            "secrets": req.secrets
        });

        let result = sqlx::query(
            r#"
            UPDATE sessions 
            SET is_published = true, 
                published_at = NOW(), 
                published_by = ?,
                publish_permissions = ?
            WHERE id = ? AND state != 'deleted'
            "#
        )
        .bind(published_by)
        .bind(&publish_permissions)
        .bind(id)
        .execute(pool)
        .await?;

        if result.rows_affected() > 0 {
            Self::find_by_id(pool, id).await
        } else {
            Ok(None)
        }
    }

    pub async fn unpublish(pool: &sqlx::MySqlPool, id: &str) -> Result<Option<Session>, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE sessions 
            SET is_published = false, 
                published_at = NULL, 
                published_by = NULL,
                publish_permissions = JSON_OBJECT('data', true, 'code', true, 'secrets', true)
            WHERE id = ? AND state != 'deleted'
            "#
        )
        .bind(id)
        .execute(pool)
        .await?;

        if result.rows_affected() > 0 {
            Self::find_by_id(pool, id).await
        } else {
            Ok(None)
        }
    }

    pub async fn find_published(pool: &sqlx::MySqlPool) -> Result<Vec<Session>, sqlx::Error> {
        sqlx::query_as::<_, Session>(
            r#"
            SELECT id, created_by, name, state,
                   container_id, persistent_volume_id, parent_session_id,
                   created_at, last_activity_at, metadata,
                   is_published, published_at, published_by, publish_permissions,
                   timeout_seconds, auto_close_at
            FROM sessions
            WHERE is_published = true AND state != 'deleted'
            ORDER BY published_at DESC
            "#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_published_by_name(pool: &sqlx::MySqlPool, name: &str) -> Result<Option<Session>, sqlx::Error> {
        sqlx::query_as::<_, Session>(
            r#"
            SELECT id, created_by, name, state,
                   container_id, persistent_volume_id, parent_session_id,
                   created_at, last_activity_at, metadata,
                   is_published, published_at, published_by, publish_permissions,
                   timeout_seconds, auto_close_at
            FROM sessions
            WHERE name = ? AND is_published = true AND state != 'deleted'
            ORDER BY published_at DESC
            LIMIT 1
            "#
        )
        .bind(name)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_sessions_to_auto_close(pool: &sqlx::MySqlPool) -> Result<Vec<Session>, sqlx::Error> {
        sqlx::query_as::<_, Session>(
            r#"
            SELECT id, created_by, name, state,
                   container_id, persistent_volume_id, parent_session_id,
                   created_at, last_activity_at, metadata,
                   is_published, published_at, published_by, publish_permissions,
                   timeout_seconds, auto_close_at
            FROM sessions
            WHERE auto_close_at <= NOW() 
              AND state IN ('init', 'idle', 'busy')
              AND state != 'deleted'
            ORDER BY auto_close_at ASC
            LIMIT 50
            "#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn extend_session_timeout(pool: &sqlx::MySqlPool, id: &str) -> Result<Option<Session>, sqlx::Error> {
        // Extend timeout based on last activity or current time
        let result = sqlx::query(
            r#"
            UPDATE sessions 
            SET auto_close_at = DATE_ADD(COALESCE(last_activity_at, NOW()), INTERVAL timeout_seconds SECOND),
                last_activity_at = NOW()
            WHERE id = ? AND state IN ('init', 'idle', 'busy') AND state != 'deleted'
            "#
        )
        .bind(id)
        .execute(pool)
        .await?;

        if result.rows_affected() > 0 {
            Self::find_by_id(pool, id).await
        } else {
            Ok(None)
        }
    }

    pub async fn update_session_to_idle(pool: &sqlx::MySqlPool, id: &str) -> Result<(), sqlx::Error> {
        // Set session to idle and calculate auto_close_at from now
        sqlx::query(
            r#"
            UPDATE sessions 
            SET state = 'idle',
                last_activity_at = NOW(),
                auto_close_at = DATE_ADD(NOW(), INTERVAL timeout_seconds SECOND)
            WHERE id = ? AND state != 'deleted'
            "#
        )
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update_session_to_busy(pool: &sqlx::MySqlPool, id: &str) -> Result<(), sqlx::Error> {
        // Set session to busy and clear auto_close_at (no timeout while active)
        sqlx::query(
            r#"
            UPDATE sessions 
            SET state = 'busy',
                last_activity_at = NOW(),
                auto_close_at = NULL
            WHERE id = ? AND state != 'deleted'
            "#
        )
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }

}