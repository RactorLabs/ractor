use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Snapshot {
    pub id: String,
    pub sandbox_id: String,
    pub trigger_type: String,
    pub created_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSnapshotRequest {
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
}

fn default_metadata() -> serde_json::Value {
    serde_json::json!({})
}

impl Snapshot {
    pub async fn find_all(pool: &sqlx::MySqlPool) -> Result<Vec<Snapshot>, sqlx::Error> {
        sqlx::query_as::<_, Snapshot>(
            r#"
            SELECT id, sandbox_id, trigger_type, created_at, metadata
            FROM snapshots
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_id(
        pool: &sqlx::MySqlPool,
        id: &str,
    ) -> Result<Option<Snapshot>, sqlx::Error> {
        sqlx::query_as::<_, Snapshot>(
            r#"
            SELECT id, sandbox_id, trigger_type, created_at, metadata
            FROM snapshots
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_sandbox(
        pool: &sqlx::MySqlPool,
        sandbox_id: &str,
    ) -> Result<Vec<Snapshot>, sqlx::Error> {
        sqlx::query_as::<_, Snapshot>(
            r#"
            SELECT id, sandbox_id, trigger_type, created_at, metadata
            FROM snapshots
            WHERE sandbox_id = ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(sandbox_id)
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &sqlx::MySqlPool,
        sandbox_id: &str,
        trigger_type: &str,
        req: CreateSnapshotRequest,
    ) -> Result<Snapshot, sqlx::Error> {
        Self::create_with_id(pool, sandbox_id, trigger_type, req, None).await
    }

    pub async fn create_with_id(
        pool: &sqlx::MySqlPool,
        sandbox_id: &str,
        trigger_type: &str,
        req: CreateSnapshotRequest,
        snapshot_id: Option<String>,
    ) -> Result<Snapshot, sqlx::Error> {
        let snapshot_id = snapshot_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        sqlx::query(
            r#"
            INSERT INTO snapshots (id, sandbox_id, trigger_type, metadata)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&snapshot_id)
        .bind(sandbox_id)
        .bind(trigger_type)
        .bind(&req.metadata)
        .execute(pool)
        .await?;

        let snapshot = Self::find_by_id(pool, &snapshot_id).await?.unwrap();

        Ok(snapshot)
    }

    pub async fn delete(pool: &sqlx::MySqlPool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM snapshots WHERE id = ?"#)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
