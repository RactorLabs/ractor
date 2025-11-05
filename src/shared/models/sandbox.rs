use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Sandbox {
    pub id: String,
    pub created_by: String,
    pub state: String,
    pub description: Option<String>,
    pub snapshot_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub tags: serde_json::Value,
    pub idle_timeout_seconds: i32,
    pub idle_from: Option<DateTime<Utc>>,
    pub busy_from: Option<DateTime<Utc>>,
    pub context_cutoff_at: Option<DateTime<Utc>>,
    pub last_context_length: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSandboxRequest {
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
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
        default = "default_idle_timeout",
        deserialize_with = "deserialize_strict_option_i32"
    )]
    pub idle_timeout_seconds: Option<i32>,
    #[serde(default)]
    pub snapshot_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSandboxStateRequest {
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSandboxRequest {
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_tags_vec")]
    pub tags: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_strict_option_i32")]
    pub idle_timeout_seconds: Option<i32>,
}

fn default_metadata() -> serde_json::Value {
    serde_json::json!({})
}

fn default_idle_timeout() -> Option<i32> {
    Some(900)
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
            if value.fract() == 0.0 && value >= i32::MIN as f64 && value <= i32::MAX as f64 {
                Ok(Some(value as i32))
            } else {
                Err(E::custom("expected integer value within i32 range"))
            }
        }
    }

    deserializer.deserialize_any(StrictOptionI32Visitor)
}

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

impl Sandbox {
    pub async fn find_all(pool: &sqlx::MySqlPool) -> Result<Vec<Sandbox>, sqlx::Error> {
        sqlx::query_as::<_, Sandbox>(
            r#"
            SELECT id, created_by, state, description,
                   created_at, last_activity_at, metadata, tags,
                   idle_timeout_seconds, idle_from, busy_from, context_cutoff_at,
                   last_context_length
            FROM sandboxes
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_id(
        pool: &sqlx::MySqlPool,
        id: &str,
    ) -> Result<Option<Sandbox>, sqlx::Error> {
        sqlx::query_as::<_, Sandbox>(
            r#"
            SELECT id, created_by, state, description, snapshot_id,
                   created_at, last_activity_at, metadata, tags,
                   idle_timeout_seconds, idle_from, busy_from, context_cutoff_at,
                   last_context_length
            FROM sandboxes
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    pub async fn create(
        pool: &sqlx::MySqlPool,
        req: CreateSandboxRequest,
        created_by: &str,
    ) -> Result<Sandbox, sqlx::Error> {
        let sandbox_id = uuid::Uuid::new_v4().to_string();
        let idle_timeout = req.idle_timeout_seconds.unwrap_or(900);
        let idle_from: Option<DateTime<Utc>> = None;
        let busy_from: Option<DateTime<Utc>> = None;

        sqlx::query(
            r#"
            INSERT INTO sandboxes (id, created_by, description, snapshot_id, metadata, tags, idle_timeout_seconds, idle_from, busy_from)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&sandbox_id)
        .bind(created_by)
        .bind(&req.description)
        .bind(&req.snapshot_id)
        .bind(&req.metadata)
        .bind(serde_json::json!(req.tags.into_iter().map(|t| t.to_lowercase()).collect::<Vec<_>>()))
        .bind(idle_timeout)
        .bind(idle_from)
        .bind(busy_from)
        .execute(pool)
        .await?;

        let sandbox = Self::find_by_id(pool, &sandbox_id).await?.unwrap();

        Ok(sandbox)
    }

    #[allow(dead_code)]
    pub async fn update_state(
        pool: &sqlx::MySqlPool,
        id: &str,
        req: UpdateSandboxStateRequest,
    ) -> Result<Option<Sandbox>, sqlx::Error> {
        let current = Self::find_by_id(pool, id).await?;
        if let Some(sandbox) = current {
            if !super::state_helpers::can_transition_to(&sandbox.state, &req.state) {
                return Err(sqlx::Error::Protocol(format!(
                    "Invalid state transition from {:?} to {:?}",
                    sandbox.state, req.state
                )));
            }
        } else {
            return Ok(None);
        }

        let now = Utc::now();
        let query_builder = String::from("UPDATE sandboxes SET state = ?, last_activity_at = ? WHERE id = ?");

        let query = sqlx::query(&query_builder)
            .bind(req.state.clone())
            .bind(now)
            .bind(id);

        let _result = query.execute(pool).await?;

        Self::find_by_id(pool, id).await
    }

    pub async fn update(
        pool: &sqlx::MySqlPool,
        id: &str,
        req: UpdateSandboxRequest,
    ) -> Result<Option<Sandbox>, sqlx::Error> {
        let mut query_builder = String::from("UPDATE sandboxes SET");
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

        if updates.is_empty() {
            return Err(sqlx::Error::Protocol("No fields to update".to_string()));
        }

        query_builder.push_str(&updates.join(","));
        query_builder.push_str(" WHERE id = ?");

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

        if let Some(idle_timeout_seconds) = req.idle_timeout_seconds {
            query = query.bind(idle_timeout_seconds);
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
        let result = sqlx::query(r#"DELETE FROM sandboxes WHERE id = ?"#)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn clear_context_cutoff(
        pool: &sqlx::MySqlPool,
        id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE sandboxes
            SET context_cutoff_at = NOW(),
                last_context_length = 0
            WHERE id = ?
            "#,
        )
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update_last_context_length(
        pool: &sqlx::MySqlPool,
        id: &str,
        tokens: i64,
    ) -> Result<(), sqlx::Error> {
        let clamped = if tokens < 0 { 0 } else { tokens };
        sqlx::query(
            r#"
            UPDATE sandboxes
            SET last_context_length = ?
            WHERE id = ?
            "#,
        )
        .bind(clamped)
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update_sandbox_to_idle(
        pool: &sqlx::MySqlPool,
        id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE sandboxes
            SET state = 'idle',
                last_activity_at = NOW(),
                idle_from = NOW(),
                busy_from = NULL
            WHERE id = ?
            "#,
        )
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update_sandbox_to_busy(
        pool: &sqlx::MySqlPool,
        id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE sandboxes
            SET state = 'busy',
                last_activity_at = NOW(),
                idle_from = NULL,
                busy_from = NOW()
            WHERE id = ?
            "#,
        )
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }
}
