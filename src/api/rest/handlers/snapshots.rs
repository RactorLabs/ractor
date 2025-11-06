use axum::http::{Response, StatusCode};
use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncReadExt;

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::shared::models::{AppState, CreateSandboxRequest, CreateSnapshotRequest, Sandbox, Snapshot};

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

pub async fn create_from_snapshot(
    State(state): State<Arc<AppState>>,
    Path(snapshot_id): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(mut req): Json<CreateSandboxRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // Verify snapshot exists
    let _snapshot = Snapshot::find_by_id(&state.db, &snapshot_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Snapshot not found".to_string()))?;

    // Set the snapshot_id in the request
    req.snapshot_id = Some(snapshot_id.clone());

    // Delegate to the regular create_sandbox handler
    let sandbox_response = crate::api::rest::handlers::sandboxes::create_sandbox(
        State(state),
        Extension(auth),
        Json(req),
    )
    .await?;

    // Convert SandboxResponse to JSON Value
    Ok(Json(serde_json::to_value(sandbox_response.0).unwrap()))
}

#[derive(Debug, Deserialize)]
pub struct ListFilesQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

fn is_safe_relative_path(path: &str) -> bool {
    !path.contains("..") && !path.starts_with('/')
}

fn get_content_type(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    match ext.to_lowercase().as_str() {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "application/javascript",
        "json" => "application/json",
        "txt" | "md" => "text/plain",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "xml" => "application/xml",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

pub async fn list_snapshot_files(
    State(state): State<Arc<AppState>>,
    Path((snapshot_id, path)): Path<(String, String)>,
    Query(paging): Query<ListFilesQuery>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Verify snapshot exists
    let _snapshot = Snapshot::find_by_id(&state.db, &snapshot_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Snapshot not found".to_string()))?;

    if !is_safe_relative_path(&path) && !path.is_empty() {
        return Err(ApiError::BadRequest("Invalid path".to_string()));
    }

    let snapshot_base = format!("/data/snapshots/{}/sandbox", snapshot_id);
    let full_path = if path.is_empty() {
        snapshot_base.clone()
    } else {
        format!("{}/{}", snapshot_base, path)
    };

    // Check if path exists
    let metadata = fs::metadata(&full_path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ApiError::NotFound("Path not found".to_string())
        } else {
            ApiError::Internal(anyhow::anyhow!("Failed to read path: {}", e))
        }
    })?;

    if !metadata.is_dir() {
        return Err(ApiError::BadRequest("Path is not a directory".to_string()));
    }

    let mut entries = vec![];
    let mut read_dir = fs::read_dir(&full_path)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to read directory: {}", e)))?;

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to read entry: {}", e)))?
    {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let metadata = entry
            .metadata()
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to read metadata: {}", e)))?;

        entries.push(serde_json::json!({
            "name": file_name,
            "is_dir": metadata.is_dir(),
            "size": metadata.len(),
            "modified": metadata.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs()),
        }));
    }

    // Sort: directories first, then by name
    entries.sort_by(|a, b| {
        let a_is_dir = a.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
        let b_is_dir = b.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
        let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");

        if a_is_dir != b_is_dir {
            b_is_dir.cmp(&a_is_dir)
        } else {
            a_name.cmp(b_name)
        }
    });

    let offset = paging.offset.unwrap_or(0);
    let limit = paging.limit.unwrap_or(100);
    let total = entries.len();
    let paginated_entries: Vec<_> = entries.into_iter().skip(offset).take(limit).collect();

    Ok(Json(serde_json::json!({
        "items": paginated_entries,
        "total": total,
        "offset": offset,
        "limit": limit,
    })))
}

pub async fn list_snapshot_files_root(
    State(state): State<Arc<AppState>>,
    Path(snapshot_id): Path<String>,
    Query(paging): Query<ListFilesQuery>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    list_snapshot_files(State(state), Path((snapshot_id, String::new())), Query(paging), Extension(auth)).await
}

pub async fn read_snapshot_file(
    State(state): State<Arc<AppState>>,
    Path((snapshot_id, path)): Path<(String, String)>,
    Extension(_auth): Extension<AuthContext>,
) -> Result<Response<axum::body::Body>, ApiError> {
    // Verify snapshot exists
    let _snapshot = Snapshot::find_by_id(&state.db, &snapshot_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Snapshot not found".to_string()))?;

    if !is_safe_relative_path(&path) {
        return Err(ApiError::BadRequest("Invalid path".to_string()));
    }

    let full_path = format!("/data/snapshots/{}/sandbox/{}", snapshot_id, path);

    // Check if file exists
    let metadata = fs::metadata(&full_path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ApiError::NotFound("File not found".to_string())
        } else {
            ApiError::Internal(anyhow::anyhow!("Failed to read file: {}", e))
        }
    })?;

    if !metadata.is_file() {
        return Err(ApiError::BadRequest("Path is not a file".to_string()));
    }

    // Read file contents
    let mut file = fs::File::open(&full_path)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to open file: {}", e)))?;

    let mut contents = Vec::new();
    file.read_to_end(&mut contents)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to read file: {}", e)))?;

    let content_type = get_content_type(&path);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", content_type)
        .header("cache-control", "no-store")
        .header("x-tsbx-file-size", metadata.len().to_string())
        .body(axum::body::Body::from(contents))
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.to_string())))?;

    Ok(response)
}

pub async fn get_snapshot_file_metadata(
    State(state): State<Arc<AppState>>,
    Path((snapshot_id, path)): Path<(String, String)>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Verify snapshot exists
    let _snapshot = Snapshot::find_by_id(&state.db, &snapshot_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Snapshot not found".to_string()))?;

    if !is_safe_relative_path(&path) {
        return Err(ApiError::BadRequest("Invalid path".to_string()));
    }

    let full_path = format!("/data/snapshots/{}/sandbox/{}", snapshot_id, path);

    // Check if path exists
    let metadata = fs::metadata(&full_path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ApiError::NotFound("Path not found".to_string())
        } else {
            ApiError::Internal(anyhow::anyhow!("Failed to read metadata: {}", e))
        }
    })?;

    Ok(Json(serde_json::json!({
        "is_dir": metadata.is_dir(),
        "is_file": metadata.is_file(),
        "size": metadata.len(),
        "modified": metadata.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs()),
    })))
}
