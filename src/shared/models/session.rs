use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
// Removed uuid::Uuid - no longer using UUIDs in v0.4.0

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub name: String, // Primary key - no more UUID id
    pub created_by: String,
    pub state: String,
    pub description: Option<String>,
    pub parent_session_name: Option<String>, // Changed from parent_session_id
    pub created_at: DateTime<Utc>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub tags: serde_json::Value,
    pub stop_timeout_seconds: i32,
    pub archive_timeout_seconds: i32,
    pub idle_from: Option<DateTime<Utc>>,
    pub busy_from: Option<DateTime<Utc>>,
    pub context_cutoff_at: Option<DateTime<Utc>>,
    pub last_context_length: i64,
    // Removed: id, container_id, persistent_volume_id (derived from name)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartSessionRequest {
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
    #[serde(deserialize_with = "deserialize_required_name")] // Required for now
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_tags_vec")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub setup: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(
        default = "default_stop_timeout",
        deserialize_with = "deserialize_strict_option_i32"
    )]
    pub stop_timeout_seconds: Option<i32>,
    #[serde(
        default = "default_archive_timeout",
        deserialize_with = "deserialize_strict_option_i32"
    )]
    pub archive_timeout_seconds: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneSessionRequest {
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
    pub env: bool,
    #[serde(
        default = "default_true",
        deserialize_with = "deserialize_strict_bool_default_true"
    )]
    pub content: bool,
    #[serde(default)]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSessionStateRequest {
    pub state: String,
    // Removed: container_id, persistent_volume_id (derived from name)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSessionRequest {
    // Removed name field - names cannot be changed in v0.4.0
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_tags_vec")]
    pub tags: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_strict_option_i32")]
    pub stop_timeout_seconds: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_strict_option_i32")]
    pub archive_timeout_seconds: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartSessionRequest {
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

fn default_stop_timeout() -> Option<i32> {
    Some(300)
}
fn default_archive_timeout() -> Option<i32> {
    Some(86400)
} // 24 hours

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

// Validation helpers for tags: allow alphanumeric and '/', '-', '_', '.'; no spaces
fn validate_tag_str(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-' || c == '_' || c == '.')
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
            f.write_str("an array of tag strings (letters, digits, '/', '-', '_', '.'; no spaces)")
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
                        "tags must be non-empty and contain only letters, digits, '/', '-', '_', '.'",
                    ));
                }
                out.push(t.to_lowercase());
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
            f.write_str(
                "null or an array of tag strings (letters, digits, '/', '-', '_', '.'; no spaces)",
            )
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
                        "tags must be non-empty and contain only letters, digits, '/', '-', '_', '.'",
                    ));
                }
                out.push(t.to_lowercase());
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
impl Session {
    pub async fn find_all(pool: &sqlx::MySqlPool) -> Result<Vec<Session>, sqlx::Error> {
        sqlx::query_as::<_, Session>(
            r#"
            SELECT name, created_by, state, description, parent_session_name,
                   created_at, last_activity_at, metadata, tags,
                   stop_timeout_seconds, archive_timeout_seconds, idle_from, busy_from, context_cutoff_at,
                   last_context_length
            FROM sessions
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_name(
        pool: &sqlx::MySqlPool,
        name: &str,
    ) -> Result<Option<Session>, sqlx::Error> {
        sqlx::query_as::<_, Session>(
            r#"
            SELECT name, created_by, state, description, parent_session_name,
                   created_at, last_activity_at, metadata, tags,
                   stop_timeout_seconds, archive_timeout_seconds, idle_from, busy_from, context_cutoff_at,
                   last_context_length
            FROM sessions
            WHERE name = ?
            "#,
        )
        .bind(name)
        .fetch_optional(pool)
        .await
    }

    pub async fn create(
        pool: &sqlx::MySqlPool,
        req: StartSessionRequest,
        created_by: &str,
    ) -> Result<Session, sqlx::Error> {
        // Use the provided name (random generation to be implemented later)
        let session_name = req.name;

        // Initialize stop/archive timeouts; idle_from/busy_from will be set on state transitions
        let stop_timeout = req.stop_timeout_seconds.unwrap_or(300);
        let archive_timeout = req.archive_timeout_seconds.unwrap_or(86400);
        let idle_from: Option<DateTime<Utc>> = None; // Will be set when session becomes idle
        let busy_from: Option<DateTime<Utc>> = None; // Will be set when session becomes busy

        // Insert the session using name as primary key
        sqlx::query(
            r#"
            INSERT INTO sessions (name, created_by, description, metadata, tags, stop_timeout_seconds, archive_timeout_seconds, idle_from, busy_from)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&session_name)
        .bind(created_by)
        .bind(&req.description)
        .bind(&req.metadata)
        .bind(serde_json::json!(req.tags.into_iter().map(|t| t.to_lowercase()).collect::<Vec<_>>()))
        .bind(stop_timeout)
        .bind(archive_timeout)
        .bind(idle_from)
        .bind(busy_from)
        .execute(pool)
        .await?;

        // Fetch the created session
        let session = Self::find_by_name(pool, &session_name).await?.unwrap();

        Ok(session)
    }

    pub async fn clone_from(
        pool: &sqlx::MySqlPool,
        parent_name: &str,
        req: CloneSessionRequest,
        created_by: &str,
    ) -> Result<Session, sqlx::Error> {
        // Get parent session
        let parent = Self::find_by_name(pool, parent_name)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        // Create new session based on parent (inherit stop/archive timeouts)
        let idle_from: Option<DateTime<Utc>> = None; // Will be set when session becomes idle
        let busy_from: Option<DateTime<Utc>> = None; // Will be set when session becomes busy

        sqlx::query(
            r#"
            INSERT INTO sessions (
                name, created_by, description, parent_session_name,
                metadata, tags, stop_timeout_seconds, archive_timeout_seconds, idle_from, busy_from
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&req.name)
        .bind(created_by) // Use actual cloner as owner
        .bind(&parent.description)
        .bind(parent_name)
        .bind(req.metadata.as_ref().unwrap_or(&parent.metadata))
        .bind(&match &parent.tags {
            serde_json::Value::Array(arr) => serde_json::Value::Array(
                arr.iter()
                    .map(|v| {
                        v.as_str()
                            .map(|s| serde_json::Value::String(s.to_lowercase()))
                            .unwrap_or_else(|| v.clone())
                    })
                    .collect(),
            ),
            v => v.clone(),
        })
        .bind(parent.stop_timeout_seconds) // Inherit stop timeout from parent
        .bind(parent.archive_timeout_seconds) // Inherit archive timeout from parent
        .bind(idle_from)
        .bind(busy_from)
        .execute(pool)
        .await?;

        // Fetch the created session
        let session = Self::find_by_name(pool, &req.name).await?.unwrap();

        Ok(session)
    }

    #[allow(dead_code)]
    pub async fn update_state(
        pool: &sqlx::MySqlPool,
        name: &str,
        req: UpdateSessionStateRequest,
    ) -> Result<Option<Session>, sqlx::Error> {
        // Check current state and validate transition
        let current = Self::find_by_name(pool, name).await?;
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

        // Removed container_id and persistent_volume_id - derived from name in v0.4.0

        query_builder.push_str(" WHERE name = ?");

        // Build and execute query
        let mut query = sqlx::query(&query_builder)
            .bind(req.state.clone())
            .bind(now);

        // Removed container_id and persistent_volume_id bindings

        query = query.bind(name);

        let _result = query.execute(pool).await?;

        // Always fetch and return the current record. rows_affected() can be 0 when
        // updating with the same values; treat that as a successful no-op update.
        Self::find_by_name(pool, name).await
    }

    pub async fn update(
        pool: &sqlx::MySqlPool,
        name: &str,
        req: UpdateSessionRequest,
    ) -> Result<Option<Session>, sqlx::Error> {
        let mut query_builder = String::from("UPDATE sessions SET");
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

        if req.stop_timeout_seconds.is_some() {
            updates.push(" stop_timeout_seconds = ?".to_string());
        }
        if req.archive_timeout_seconds.is_some() {
            updates.push(" archive_timeout_seconds = ?".to_string());
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
            let lowered: Vec<String> = tags.into_iter().map(|t| t.to_lowercase()).collect();
            query = query.bind(serde_json::json!(lowered));
        }

        if let Some(stop_timeout_seconds) = req.stop_timeout_seconds {
            query = query.bind(stop_timeout_seconds);
        }
        if let Some(archive_timeout_seconds) = req.archive_timeout_seconds {
            query = query.bind(archive_timeout_seconds);
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
        // Hard delete session row; cascades will remove tasks; requests may persist per FK changes
        let result = sqlx::query(r#"DELETE FROM sessions WHERE name = ?"#)
            .bind(name)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn clear_context_cutoff(
        pool: &sqlx::MySqlPool,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE sessions
            SET context_cutoff_at = NOW(),
                last_context_length = 0
            WHERE name = ?
            "#,
        )
        .bind(name)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update_last_context_length(
        pool: &sqlx::MySqlPool,
        name: &str,
        tokens: i64,
    ) -> Result<(), sqlx::Error> {
        let clamped = if tokens < 0 { 0 } else { tokens };
        sqlx::query(
            r#"
            UPDATE sessions
            SET last_context_length = ?
            WHERE name = ?
            "#,
        )
        .bind(clamped)
        .bind(name)
        .execute(pool)
        .await?;

        Ok(())
    }

    // find_sessions_to_auto_close replaced by controller-side logic

    // extend_session_timeout removed in new timeout model

    pub async fn update_session_to_idle(
        pool: &sqlx::MySqlPool,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        // Set session to idle and record idle_from; clear busy_from
        sqlx::query(
            r#"
            UPDATE sessions 
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

    pub async fn update_session_to_busy(
        pool: &sqlx::MySqlPool,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        // Set session to busy: clear idle_from, set busy_from
        sqlx::query(
            r#"
            UPDATE sessions 
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
