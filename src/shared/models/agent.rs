use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
// Removed uuid::Uuid - no longer using UUIDs in v0.4.0
use super::constants::AGENT_STATE_DELETED;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Agent {
    pub name: String, // Primary key - no more UUID id
    pub created_by: String,
    pub state: String,
    pub parent_agent_name: Option<String>, // Changed from parent_agent_id
    pub created_at: DateTime<Utc>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub is_published: bool,
    pub published_at: Option<DateTime<Utc>>,
    pub published_by: Option<String>,
    pub publish_permissions: serde_json::Value,
    pub timeout_seconds: i32,
    pub auto_close_at: Option<DateTime<Utc>>,
    pub content_port: Option<i32>,
    // Removed: id, container_id, persistent_volume_id (derived from name)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentRequest {
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
    #[serde(deserialize_with = "deserialize_required_name")] // Name is now required
    pub name: String,
    #[serde(default)]
    pub secrets: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub setup: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(
        default = "default_timeout",
        deserialize_with = "deserialize_strict_option_i32"
    )]
    pub timeout_seconds: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemixAgentRequest {
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(deserialize_with = "deserialize_required_name")] // Name is now required
    pub name: String,
    // Removed data field - data folder no longer exists
    #[serde(
        default = "default_true",
        deserialize_with = "deserialize_strict_bool_default_true"
    )]
    pub code: bool,
    #[serde(
        default = "default_true",
        deserialize_with = "deserialize_strict_bool_default_true"
    )]
    pub secrets: bool,
    #[serde(
        default = "default_true",
        deserialize_with = "deserialize_strict_bool_default_true"
    )]
    pub content: bool,
    #[serde(default)]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishAgentRequest {
    // Removed data field - data folder no longer exists
    #[serde(
        default = "default_true",
        deserialize_with = "deserialize_strict_bool_default_true"
    )]
    pub code: bool,
    #[serde(
        default = "default_true",
        deserialize_with = "deserialize_strict_bool_default_true"
    )]
    pub secrets: bool,
    #[serde(
        default = "default_true",
        deserialize_with = "deserialize_strict_bool_default_true"
    )]
    pub content: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgentStateRequest {
    pub state: String,
    #[serde(default)]
    pub content_port: Option<i32>,
    // Removed: container_id, persistent_volume_id (derived from name)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgentRequest {
    // Removed name field - names cannot be changed in v0.4.0
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default, deserialize_with = "deserialize_strict_option_i32")]
    pub timeout_seconds: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreAgentRequest {
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

    deserializer.deserialize_any(StrictOptionI32Visitor)
}

// Custom deserializer for name validation (alphanumeric and hyphens only)
fn deserialize_valid_name<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Visitor};

    struct ValidNameVisitor;

    impl<'de> Visitor<'de> for ValidNameVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a valid name (alphanumeric and hyphens only) or null")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if value.is_empty() {
                return Ok(None);
            }

            if value.len() > 100 {
                return Err(E::custom("name too long (max 100 characters)"));
            }

            if !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(E::custom(
                    "name must contain only alphanumeric characters and hyphens",
                ));
            }

            Ok(Some(value.to_string()))
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
    }

    deserializer.deserialize_any(ValidNameVisitor)
}

// Custom deserializer for required name validation (v0.4.0: names are mandatory)
fn deserialize_required_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Visitor};

    struct RequiredNameVisitor;

    impl<'de> Visitor<'de> for RequiredNameVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a required valid name (must start with letter, contain only lowercase letters/numbers/hyphens, max 64 chars)")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if value.is_empty() {
                return Err(E::custom("name is required and cannot be empty"));
            }

            if value.len() > 64 {
                return Err(E::custom("name too long (max 64 characters)"));
            }

            // v0.4.0 strict validation: ^[a-z][a-z0-9-]{0,61}[a-z0-9]$
            if !value.chars().next().unwrap_or(' ').is_ascii_lowercase() {
                return Err(E::custom("name must start with a lowercase letter"));
            }

            if value.len() > 1 && !value.chars().last().unwrap_or(' ').is_ascii_alphanumeric() {
                return Err(E::custom("name must end with a letter or number"));
            }

            if !value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err(E::custom(
                    "name must contain only lowercase letters, numbers, and hyphens",
                ));
            }

            Ok(value.to_string())
        }
    }

    deserializer.deserialize_str(RequiredNameVisitor)
}

// Database queries
impl Agent {
    pub async fn find_all(pool: &sqlx::MySqlPool) -> Result<Vec<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"
            SELECT name, created_by, state, parent_agent_name,
                   created_at, last_activity_at, metadata,
                   is_published, published_at, published_by, publish_permissions,
                   timeout_seconds, auto_close_at, content_port
            FROM agents
            WHERE state != 'deleted'
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_name(
        pool: &sqlx::MySqlPool,
        name: &str,
    ) -> Result<Option<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"
            SELECT name, created_by, state, parent_agent_name,
                   created_at, last_activity_at, metadata,
                   is_published, published_at, published_by, publish_permissions,
                   timeout_seconds, auto_close_at, content_port
            FROM agents
            WHERE name = ? AND state != 'deleted'
            "#,
        )
        .bind(name)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_name_and_creator(
        pool: &sqlx::MySqlPool,
        name: &str,
        created_by: &str,
    ) -> Result<Option<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"
            SELECT name, created_by, state, parent_agent_name,
                   created_at, last_activity_at, metadata,
                   is_published, published_at, published_by, publish_permissions,
                   timeout_seconds, auto_close_at, content_port
            FROM agents
            WHERE name = ? AND created_by = ? AND state != 'deleted'
            "#,
        )
        .bind(name)
        .bind(created_by)
        .fetch_optional(pool)
        .await
    }

    // Helper function to find an available port for Content HTTP server
    async fn find_available_port() -> Result<u16, std::io::Error> {
        use std::net::TcpListener;
        let listener = TcpListener::bind("0.0.0.0:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        Ok(port)
    }

    pub async fn create(
        pool: &sqlx::MySqlPool,
        req: CreateAgentRequest,
        created_by: &str,
    ) -> Result<Agent, sqlx::Error> {
        // Calculate timeout - auto_close_at will be set when agent becomes idle
        let timeout = req.timeout_seconds.unwrap_or(300); // Default 5 minutes (300 seconds)
        let auto_close_at: Option<DateTime<Utc>> = None; // Will be calculated when agent becomes idle

        // Allocate Content port
        let content_port = Self::find_available_port()
            .await
            .map_err(|e| sqlx::Error::Io(e))?;

        // Insert the agent using name as primary key
        sqlx::query(
            r#"
            INSERT INTO agents (name, created_by, metadata, timeout_seconds, auto_close_at, content_port)
            VALUES (?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&req.name)
        .bind(created_by)
        .bind(&req.metadata)
        .bind(timeout)
        .bind(auto_close_at)
.bind(content_port as i32)
        .execute(pool)
        .await?;

        // Fetch the created agent
        let agent = Self::find_by_name(pool, &req.name).await?.unwrap();

        Ok(agent)
    }

    pub async fn remix(
        pool: &sqlx::MySqlPool,
        parent_name: &str,
        req: RemixAgentRequest,
        created_by: &str,
    ) -> Result<Agent, sqlx::Error> {
        // Get parent agent
        let parent = Self::find_by_name(pool, parent_name)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        // Create new agent based on parent (inherit timeout)
        let auto_close_at: Option<DateTime<Utc>> = None; // Will be calculated when agent becomes idle

        // Allocate Content port for remix agent
        let content_port = Self::find_available_port()
            .await
            .map_err(|e| sqlx::Error::Io(e))?;

        sqlx::query(
            r#"
            INSERT INTO agents (
                name, created_by, parent_agent_name,
                metadata, timeout_seconds, auto_close_at, content_port
            )
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&req.name)
        .bind(created_by) // Use actual remixer as owner
        .bind(parent_name)
        .bind(req.metadata.as_ref().unwrap_or(&parent.metadata))
        .bind(parent.timeout_seconds) // Inherit timeout from parent
        .bind(auto_close_at)
        .bind(content_port as i32)
        .execute(pool)
        .await?;

        // Fetch the created agent
        let agent = Self::find_by_name(pool, &req.name).await?.unwrap();

        Ok(agent)
    }

    #[allow(dead_code)]
    pub async fn update_state(
        pool: &sqlx::MySqlPool,
        name: &str,
        req: UpdateAgentStateRequest,
    ) -> Result<Option<Agent>, sqlx::Error> {
        // Check current state and validate transition
        let current = Self::find_by_name(pool, name).await?;
        if let Some(agent) = current {
            if !super::state_helpers::can_transition_to(&agent.state, &req.state) {
                return Err(sqlx::Error::Protocol(format!(
                    "Invalid state transition from {:?} to {:?}",
                    agent.state, req.state
                )));
            }
        } else {
            return Ok(None);
        }

        let now = Utc::now();
        let mut query_builder = String::from("UPDATE agents SET state = ?, last_activity_at = ?");

        // Removed container_id and persistent_volume_id - derived from name in v0.4.0

        if req.content_port.is_some() {
            query_builder.push_str(", content_port = ?");
        }

        query_builder.push_str(" WHERE name = ?");

        // Build and execute query
        let mut query = sqlx::query(&query_builder)
            .bind(req.state.clone())
            .bind(now);

        // Removed container_id and persistent_volume_id bindings

        if let Some(content_port) = req.content_port {
            query = query.bind(content_port);
        }

        query = query.bind(name);

        let result = query.execute(pool).await?;

        if result.rows_affected() > 0 {
            Self::find_by_name(pool, name).await
        } else {
            Ok(None)
        }
    }

    pub async fn update(
        pool: &sqlx::MySqlPool,
        name: &str,
        req: UpdateAgentRequest,
    ) -> Result<Option<Agent>, sqlx::Error> {
        let mut query_builder = String::from("UPDATE agents SET");
        let mut updates = Vec::new();

        if req.metadata.is_some() {
            updates.push(" metadata = ?".to_string());
        }

        if req.timeout_seconds.is_some() {
            updates.push(" timeout_seconds = ?".to_string());
            updates.push(
                " auto_close_at = DATE_ADD(COALESCE(last_activity_at, NOW()), INTERVAL ? SECOND)"
                    .to_string(),
            );
        }

        if updates.is_empty() {
            return Err(sqlx::Error::Protocol("No fields to update".to_string()));
        }

        query_builder.push_str(&updates.join(","));
        query_builder.push_str(" WHERE name = ? AND state != 'deleted'");

        let mut query = sqlx::query(&query_builder);

        if let Some(metadata) = req.metadata {
            query = query.bind(metadata);
        }

        if let Some(timeout_seconds) = req.timeout_seconds {
            query = query.bind(timeout_seconds);
            query = query.bind(timeout_seconds); // For the DATE_ADD calculation
        }

        query = query.bind(name);

        let result = query.execute(pool).await?;

        if result.rows_affected() > 0 {
            Self::find_by_name(pool, name).await
        } else {
            Ok(None)
        }
    }

    pub async fn delete(pool: &sqlx::MySqlPool, name: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(r#"UPDATE agents SET state = ? WHERE name = ? AND state != ?"#)
            .bind(AGENT_STATE_DELETED)
            .bind(name)
            .bind(AGENT_STATE_DELETED)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn publish(
        pool: &sqlx::MySqlPool,
        name: &str,
        published_by: &str,
        req: PublishAgentRequest,
    ) -> Result<Option<Agent>, sqlx::Error> {
        let publish_permissions = serde_json::json!({
            "code": req.code,
            "secrets": req.secrets,
            "content": true // Content is always allowed
        });

        let result = sqlx::query(
            r#"
            UPDATE agents 
            SET is_published = true, 
                published_at = NOW(), 
                published_by = ?,
                publish_permissions = ?
            WHERE name = ? AND state != 'deleted'
            "#,
        )
        .bind(published_by)
        .bind(&publish_permissions)
        .bind(name)
        .execute(pool)
        .await?;

        if result.rows_affected() > 0 {
            Self::find_by_name(pool, name).await
        } else {
            Ok(None)
        }
    }

    pub async fn unpublish(
        pool: &sqlx::MySqlPool,
        name: &str,
    ) -> Result<Option<Agent>, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE agents 
            SET is_published = false, 
                published_at = NULL, 
                published_by = NULL,
                publish_permissions = JSON_OBJECT('code', true, 'secrets', true)
            WHERE name = ? AND state != 'deleted'
            "#,
        )
        .bind(name)
        .execute(pool)
        .await?;

        if result.rows_affected() > 0 {
            Self::find_by_name(pool, name).await
        } else {
            Ok(None)
        }
    }

    pub async fn find_published(pool: &sqlx::MySqlPool) -> Result<Vec<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"
            SELECT name, created_by, state, parent_agent_name,
                   created_at, last_activity_at, metadata,
                   is_published, published_at, published_by, publish_permissions,
                   timeout_seconds, auto_close_at, content_port
            FROM agents
            WHERE is_published = true AND state != 'deleted'
            ORDER BY published_at DESC
            "#,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_published_by_name(
        pool: &sqlx::MySqlPool,
        name: &str,
    ) -> Result<Option<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"
            SELECT name, created_by, state, parent_agent_name,
                   created_at, last_activity_at, metadata,
                   is_published, published_at, published_by, publish_permissions,
                   timeout_seconds, auto_close_at, content_port
            FROM agents
            WHERE name = ? AND is_published = true AND state != 'deleted'
            ORDER BY published_at DESC
            LIMIT 1
            "#,
        )
        .bind(name)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_agents_to_auto_close(
        pool: &sqlx::MySqlPool,
    ) -> Result<Vec<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"
            SELECT name, created_by, state, parent_agent_name,
                   created_at, last_activity_at, metadata,
                   is_published, published_at, published_by, publish_permissions,
                   timeout_seconds, auto_close_at, content_port
            FROM agents
            WHERE auto_close_at <= NOW() 
              AND state IN ('init', 'idle', 'busy')
              AND state != 'deleted'
            ORDER BY auto_close_at ASC
            LIMIT 50
            "#,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn extend_agent_timeout(
        pool: &sqlx::MySqlPool,
        name: &str,
    ) -> Result<Option<Agent>, sqlx::Error> {
        // Extend timeout based on last activity or current time
        let result = sqlx::query(
            r#"
            UPDATE agents 
            SET auto_close_at = DATE_ADD(COALESCE(last_activity_at, NOW()), INTERVAL timeout_seconds SECOND),
                last_activity_at = NOW()
            WHERE name = ? AND state IN ('init', 'idle', 'busy') AND state != 'deleted'
            "#
        )
        .bind(name)
        .execute(pool)
        .await?;

        if result.rows_affected() > 0 {
            Self::find_by_name(pool, name).await
        } else {
            Ok(None)
        }
    }

    pub async fn update_agent_to_idle(
        pool: &sqlx::MySqlPool,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        // Set agent to idle and calculate auto_close_at from now
        sqlx::query(
            r#"
            UPDATE agents 
            SET state = 'idle',
                last_activity_at = NOW(),
                auto_close_at = DATE_ADD(NOW(), INTERVAL timeout_seconds SECOND)
            WHERE name = ? AND state != 'deleted'
            "#,
        )
        .bind(name)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update_agent_to_busy(
        pool: &sqlx::MySqlPool,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        // Set agent to busy and clear auto_close_at (no timeout while active)
        sqlx::query(
            r#"
            UPDATE agents 
            SET state = 'busy',
                last_activity_at = NOW(),
                auto_close_at = NULL
            WHERE name = ? AND state != 'deleted'
            "#,
        )
        .bind(name)
        .execute(pool)
        .await?;

        Ok(())
    }
}
