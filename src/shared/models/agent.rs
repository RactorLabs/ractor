use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
// Removed uuid::Uuid - no longer using UUIDs in v0.4.0

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Agent {
    pub name: String, // Primary key - no more UUID id
    pub created_by: String,
    pub state: String,
    pub description: Option<String>,
    pub parent_agent_name: Option<String>, // Changed from parent_agent_id
    pub created_at: DateTime<Utc>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub tags: serde_json::Value,
    pub is_published: bool,
    pub published_at: Option<DateTime<Utc>>,
    pub published_by: Option<String>,
    pub publish_permissions: serde_json::Value,
    pub idle_timeout_seconds: i32,
    pub busy_timeout_seconds: i32,
    pub idle_from: Option<DateTime<Utc>>,
    pub busy_from: Option<DateTime<Utc>>,
    pub context_cutoff_at: Option<DateTime<Utc>>,
    // Removed: id, container_id, persistent_volume_id (derived from name)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentRequest {
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
    #[serde(deserialize_with = "deserialize_required_name")] // Required for now
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_tags_vec")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub secrets: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub setup: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(
        default = "default_idle_timeout",
        deserialize_with = "deserialize_strict_option_i32"
    )]
    pub idle_timeout_seconds: Option<i32>,
    #[serde(
        default = "default_busy_timeout",
        deserialize_with = "deserialize_strict_option_i32"
    )]
    pub busy_timeout_seconds: Option<i32>,
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
    // Removed: container_id, persistent_volume_id (derived from name)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgentRequest {
    // Removed name field - names cannot be changed in v0.4.0
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_tags_vec")]
    pub tags: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_strict_option_i32")]
    pub idle_timeout_seconds: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_strict_option_i32")]
    pub busy_timeout_seconds: Option<i32>,
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

fn default_idle_timeout() -> Option<i32> { Some(300) }
fn default_busy_timeout() -> Option<i32> { Some(900) } // 15 minutes

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

// Validation helpers for tags: alphanumeric only, no spaces or special chars
fn validate_tag_str(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn deserialize_tags_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, SeqAccess, Visitor};

    struct TagsVisitor;

    impl<'de> Visitor<'de> for TagsVisitor {
        type Value = Vec<String>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("an array of alphanumeric tag strings")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut out = Vec::new();
            while let Some(item) = seq.next_element::<String>()? {
                let t = item.trim();
                if !validate_tag_str(t) {
                    return Err(A::Error::custom(
                        "tags must be non-empty alphanumeric strings (A-Za-z0-9)",
                    ));
                }
                out.push(t.to_string());
            }
            Ok(out)
        }
    }

    deserializer.deserialize_any(TagsVisitor)
}

fn deserialize_optional_tags_vec<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, SeqAccess, Visitor};

    struct OptTagsVisitor;

    impl<'de> Visitor<'de> for OptTagsVisitor {
        type Value = Option<Vec<String>>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("null or an array of alphanumeric tag strings")
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

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut out = Vec::new();
            while let Some(item) = seq.next_element::<String>()? {
                let t = item.trim();
                if !validate_tag_str(t) {
                    return Err(A::Error::custom(
                        "tags must be non-empty alphanumeric strings (A-Za-z0-9)",
                    ));
                }
                out.push(t.to_string());
            }
            Ok(Some(out))
        }
    }

    deserializer.deserialize_any(OptTagsVisitor)
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
            formatter.write_str("a required valid name (must start with a letter; letters/numbers/hyphens only; max 64 chars)")
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

            // Allow A-Z or a-z start
            if !value.chars().next().unwrap_or(' ').is_ascii_alphabetic() {
                return Err(E::custom("name must start with a letter"));
            }

            if value.len() > 1 && !value.chars().last().unwrap_or(' ').is_ascii_alphanumeric() {
                return Err(E::custom("name must end with a letter or number"));
            }

            if !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(E::custom(
                    "name must contain only letters, numbers, and hyphens",
                ));
            }

            Ok(value.to_string())
        }
    }

    deserializer.deserialize_str(RequiredNameVisitor)
}

// Custom deserializer for optional validated name
fn deserialize_optional_validated_name<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Visitor};

    struct OptionalValidatedNameVisitor;

    impl<'de> Visitor<'de> for OptionalValidatedNameVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an optional valid name (must start with a letter; letters/numbers/hyphens only; max 64 chars) or null")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if value.is_empty() {
                return Ok(None);
            }

            if value.len() > 64 {
                return Err(E::custom("name too long (max 64 characters)"));
            }

            if !value.chars().next().unwrap_or(' ').is_ascii_alphabetic() {
                return Err(E::custom("name must start with a letter"));
            }

            if value.len() > 1 && !value.chars().last().unwrap_or(' ').is_ascii_alphanumeric() {
                return Err(E::custom("name must end with a letter or number"));
            }

            if !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(E::custom(
                    "name must contain only letters, numbers, and hyphens",
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

    deserializer.deserialize_any(OptionalValidatedNameVisitor)
}

// Database queries
impl Agent {
    pub async fn find_all(pool: &sqlx::MySqlPool) -> Result<Vec<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"
            SELECT name, created_by, state, description, parent_agent_name,
                   created_at, last_activity_at, metadata, tags,
                   is_published, published_at, published_by, publish_permissions,
                   idle_timeout_seconds, busy_timeout_seconds, idle_from, busy_from, context_cutoff_at
            FROM agents
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
            SELECT name, created_by, state, description, parent_agent_name,
                   created_at, last_activity_at, metadata, tags,
                   is_published, published_at, published_by, publish_permissions,
                   idle_timeout_seconds, busy_timeout_seconds, idle_from, busy_from, context_cutoff_at
            FROM agents
            WHERE name = ?
            "#,
        )
        .bind(name)
        .fetch_optional(pool)
        .await
    }

    

    

    pub async fn create(
        pool: &sqlx::MySqlPool,
        req: CreateAgentRequest,
        created_by: &str,
    ) -> Result<Agent, sqlx::Error> {
        // Use the provided name (random generation to be implemented later)
        let agent_name = req.name;

        // Initialize timeouts; idle_from/busy_from will be set on state transitions
        let idle_timeout = req.idle_timeout_seconds.unwrap_or(300);
        let busy_timeout = req.busy_timeout_seconds.unwrap_or(900);
        let idle_from: Option<DateTime<Utc>> = None; // Will be set when agent becomes idle
        let busy_from: Option<DateTime<Utc>> = None; // Will be set when agent becomes busy

        // Insert the agent using name as primary key
        sqlx::query(
            r#"
            INSERT INTO agents (name, created_by, description, metadata, tags, idle_timeout_seconds, busy_timeout_seconds, idle_from, busy_from)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&agent_name)
        .bind(created_by)
        .bind(&req.description)
        .bind(&req.metadata)
        .bind(serde_json::json!(req.tags))
        .bind(idle_timeout)
        .bind(busy_timeout)
        .bind(idle_from)
        .bind(busy_from)
        .execute(pool)
        .await?;

        // Fetch the created agent
        let agent = Self::find_by_name(pool, &agent_name).await?.unwrap();

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

        // Create new agent based on parent (inherit timeouts)
        let idle_from: Option<DateTime<Utc>> = None; // Will be set when agent becomes idle
        let busy_from: Option<DateTime<Utc>> = None; // Will be set when agent becomes busy

        sqlx::query(
            r#"
            INSERT INTO agents (
                name, created_by, description, parent_agent_name,
                metadata, tags, idle_timeout_seconds, busy_timeout_seconds, idle_from, busy_from
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&req.name)
        .bind(created_by) // Use actual remixer as owner
        .bind(&parent.description)
        .bind(parent_name)
        .bind(req.metadata.as_ref().unwrap_or(&parent.metadata))
        .bind(&parent.tags)
        .bind(parent.idle_timeout_seconds) // Inherit idle timeout from parent
        .bind(parent.busy_timeout_seconds) // Inherit busy timeout from parent
        .bind(idle_from)
        .bind(busy_from)
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

        query_builder.push_str(" WHERE name = ?");

        // Build and execute query
        let mut query = sqlx::query(&query_builder)
            .bind(req.state.clone())
            .bind(now);

        // Removed container_id and persistent_volume_id bindings

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
        if req.description.is_some() {
            updates.push(" description = ?".to_string());
        }
        if req.tags.is_some() {
            updates.push(" tags = ?".to_string());
        }

        if req.idle_timeout_seconds.is_some() {
            updates.push(" idle_timeout_seconds = ?".to_string());
        }
        if req.busy_timeout_seconds.is_some() {
            updates.push(" busy_timeout_seconds = ?".to_string());
        }

        if updates.is_empty() {
            return Err(sqlx::Error::Protocol("No fields to update".to_string()));
        }

        query_builder.push_str(&updates.join(","));
        query_builder.push_str(" WHERE name = ?");

        let mut query = sqlx::query(&query_builder);

        if let Some(metadata) = req.metadata {
            query = query.bind(metadata);
        }
        if let Some(description) = req.description {
            query = query.bind(description);
        }
        if let Some(tags) = req.tags {
            query = query.bind(serde_json::json!(tags));
        }

        if let Some(idle_timeout_seconds) = req.idle_timeout_seconds {
            query = query.bind(idle_timeout_seconds);
        }
        if let Some(busy_timeout_seconds) = req.busy_timeout_seconds {
            query = query.bind(busy_timeout_seconds);
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
        // Hard delete agent row; cascades will remove responses; tasks may persist per FK changes
        let result = sqlx::query(r#"DELETE FROM agents WHERE name = ?"#)
            .bind(name)
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
            WHERE name = ?
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
            WHERE name = ?
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
            SELECT name, created_by, state, description, parent_agent_name,
                   created_at, last_activity_at, metadata, tags,
                   is_published, published_at, published_by, publish_permissions,
                   idle_timeout_seconds, busy_timeout_seconds, idle_from, busy_from, context_cutoff_at
            FROM agents
            WHERE is_published = true
            ORDER BY published_at DESC
            "#,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn clear_context_cutoff(pool: &sqlx::MySqlPool, name: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE agents
            SET context_cutoff_at = NOW()
            WHERE name = ?
            "#,
        )
        .bind(name)
        .execute(pool)
        .await?;

        Ok(())
    }

    

    // find_agents_to_auto_close replaced by controller-side logic

    // extend_agent_timeout removed in new timeout model

    pub async fn update_agent_to_idle(
        pool: &sqlx::MySqlPool,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        // Set agent to idle and record idle_from; clear busy_from
        sqlx::query(
            r#"
            UPDATE agents 
            SET state = 'idle',
                last_activity_at = NOW(),
                idle_from = NOW(),
                busy_from = NULL
            WHERE name = ?
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
        // Set agent to busy: clear idle_from, set busy_from
        sqlx::query(
            r#"
            UPDATE agents 
            SET state = 'busy',
                last_activity_at = NOW(),
                idle_from = NULL,
                busy_from = NOW()
            WHERE name = ?
            "#,
        )
        .bind(name)
        .execute(pool)
        .await?;

        Ok(())
    }
}
