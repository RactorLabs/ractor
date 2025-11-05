use axum::http::StatusCode;
use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::shared::models::{AppState, CreateSnapshotRequest, Sandbox, Snapshot};

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotResponse {
    pub id: String,
    pub sandbox_id: String,
    pub trigger_type: String,
    pub created_at: String,
    pub metadata: serde_json::Value,
}

impl From<Snapshot> for SnapshotResponse {
    fn from(snapshot: Snapshot) -> Self {
        Self {
            id: snapshot.id,
            sandbox_id: snapshot.sandbox_id,
            trigger_type: snapshot.trigger_type,
            created_at: snapshot.created_at.to_rfc3339(),
            metadata: snapshot.metadata,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedSnapshots {
    pub items: Vec<SnapshotResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub page: usize,
    pub pages: usize,
}

#[derive(Debug, Deserialize)]
pub struct ListSnapshotsQuery {
    pub sandbox_id: Option<String>,
}

pub async fn list_snapshots(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListSnapshotsQuery>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<PaginatedSnapshots>> {
    let snapshots = if let Some(sandbox_id) = query.sandbox_id {
        // Filter by sandbox_id
        Snapshot::find_by_sandbox(&state.db, &sandbox_id)
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
    } else {
        // Get all snapshots
        Snapshot::find_all(&state.db)
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
    };

    let total = snapshots.len();
    let items: Vec<SnapshotResponse> = snapshots.into_iter().map(SnapshotResponse::from).collect();

    Ok(Json(PaginatedSnapshots {
        items,
        total,
        limit: 100,
        offset: 0,
        page: 1,
        pages: 1,
    }))
}

pub async fn get_snapshot(
    State(state): State<Arc<AppState>>,
    Path(snapshot_id): Path<String>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<SnapshotResponse>> {
    let snapshot = Snapshot::find_by_id(&state.db, &snapshot_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Snapshot not found".to_string()))?;

    Ok(Json(SnapshotResponse::from(snapshot)))
}

pub async fn list_sandbox_snapshots(
    State(state): State<Arc<AppState>>,
    Path(sandbox_id): Path<String>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<PaginatedSnapshots>> {
    let sandbox = Sandbox::find_by_id(&state.db, &sandbox_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Sandbox not found".to_string()))?;

    let snapshots = Snapshot::find_by_sandbox(&state.db, &sandbox.id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?;

    let total = snapshots.len();
    let items: Vec<SnapshotResponse> = snapshots.into_iter().map(SnapshotResponse::from).collect();

    Ok(Json(PaginatedSnapshots {
        items,
        total,
        limit: 100,
        offset: 0,
        page: 1,
        pages: 1,
    }))
}

pub async fn create_snapshot(
    State(state): State<Arc<AppState>>,
    Path(sandbox_id): Path<String>,
    Extension(_auth): Extension<AuthContext>,
    Json(req): Json<CreateSnapshotRequest>,
) -> ApiResult<(StatusCode, Json<SnapshotResponse>)> {
    let sandbox = Sandbox::find_by_id(&state.db, &sandbox_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Sandbox not found".to_string()))?;

    if sandbox.state == "deleted" {
        return Err(ApiError::BadRequest(
            "Cannot create snapshot of deleted sandbox".to_string(),
        ));
    }

    let snapshot = Snapshot::create(&state.db, &sandbox.id, "user", req)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?;

    Ok((StatusCode::CREATED, Json(SnapshotResponse::from(snapshot))))
}

pub async fn delete_snapshot(
    State(state): State<Arc<AppState>>,
    Path(snapshot_id): Path<String>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<StatusCode> {
    let deleted = Snapshot::delete(&state.db, &snapshot_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("Snapshot not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}
