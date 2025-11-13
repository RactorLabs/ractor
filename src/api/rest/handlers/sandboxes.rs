use axum::http::StatusCode;
use axum::response::Response;
use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::de::{self, Deserializer, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;
use tracing::warn;

use bollard::container::{InspectContainerOptions, MemoryStatsStats, Stats, StatsOptions};
use bollard::errors::Error as BollardError;
use bollard::Docker;

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::api::rest::rbac_enforcement::{check_api_permission, permissions};
use crate::shared::models::task::{compute_output_content, extract_steps};
use crate::shared::models::{
    AppState, CreateSandboxRequest, CreateTaskRequest, Sandbox, SandboxTask, TaskSummary, TaskView,
    UpdateSandboxRequest, UpdateSandboxStateRequest, UpdateTaskRequest,
};
use crate::shared::rbac::PermissionContext;

// Helper: determine if principal has admin-like privileges via RBAC (wildcard rule)
async fn is_admin_principal(auth: &AuthContext, state: &AppState) -> bool {
    let ctx = PermissionContext {
        api_group: "api".into(),
        resource: "*".into(),
        verb: "*".into(),
    };
    match crate::api::auth::check_permission(&auth.principal, state, &ctx).await {
        Ok(true) => true,
        _ => false,
    }
}

// Helper: ensure sandbox is not terminated before allowing write operations
fn check_not_terminated(sandbox: &Sandbox) -> ApiResult<()> {
    if sandbox.state.eq_ignore_ascii_case("terminated")
        || sandbox.state.eq_ignore_ascii_case("terminating")
        || sandbox.state.eq_ignore_ascii_case("initializing")
        || sandbox.state.eq_ignore_ascii_case("deleted")
    {
        return Err(ApiError::BadRequest("Sandbox not available.".to_string()));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct SandboxResponse {
    pub id: String, // Primary key - UUID
    pub created_by: String,
    pub state: String,
    pub description: Option<String>,
    pub snapshot_id: Option<String>,
    pub created_at: String,
    pub last_activity_at: Option<String>,
    pub metadata: serde_json::Value,
    pub tags: Vec<String>,
    pub idle_timeout_seconds: i32,
    pub idle_from: Option<String>,
    pub busy_from: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListSandboxesQuery {
    pub state: Option<String>,
    pub q: Option<String>,
    // Accept both tags=alpha (single), tags=alpha&tags=beta (repeat), or tags[]=alpha
    #[serde(default, deserialize_with = "deserialize_opt_string_or_seq")]
    pub tags: Option<Vec<String>>,
    pub limit: Option<i64>,
    pub page: Option<i64>,   // 1-based
    pub offset: Option<i64>, // takes precedence over page when provided
}

#[derive(Debug, Deserialize)]
pub struct ListTasksQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// Custom deserializer for query param that can be string or array
fn deserialize_opt_string_or_seq<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrSeq;
    impl<'de> Visitor<'de> for StringOrSeq {
        type Value = Option<Vec<String>>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a string, a sequence of strings, or null")
        }
        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(vec![v.to_string()]))
        }
        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(vec![v]))
        }
        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut out: Vec<String> = Vec::new();
            while let Some(item) = seq.next_element::<String>()? {
                out.push(item);
            }
            Ok(Some(out))
        }
    }
    deserializer.deserialize_any(StringOrSeq)
}

#[derive(Debug, Serialize)]
pub struct PaginatedSandboxes {
    pub items: Vec<SandboxResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    pub page: i64,
    pub pages: i64,
}

#[derive(Debug, Serialize)]
pub struct SandboxTopResponse {
    pub sandbox_id: String,
    pub container_state: String,
    pub tasks_completed: i64,
    pub cpu_usage_percent: f64,
    pub cpu_limit_cores: f64,
    pub memory_usage_bytes: u64,
    pub memory_limit_bytes: u64,
    pub captured_at: String,
}

struct ContainerMetrics {
    state: String,
    cpu_percent: f64,
    cpu_limit_cores: f64,
    memory_usage: u64,
    memory_limit: u64,
}

impl SandboxResponse {
    async fn from_sandbox(sandbox: Sandbox, _pool: &sqlx::MySqlPool) -> Result<Self, ApiError> {
        // Convert tags from JSON value to Vec<String>
        let tags: Vec<String> = match sandbox.tags {
            serde_json::Value::Array(arr) => arr
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            _ => Vec::new(),
        };
        Ok(Self {
            id: sandbox.id,
            created_by: sandbox.created_by,
            state: sandbox.state,
            description: sandbox.description,
            snapshot_id: sandbox.snapshot_id,
            created_at: sandbox.created_at.to_rfc3339(),
            last_activity_at: sandbox.last_activity_at.map(|dt| dt.to_rfc3339()),
            metadata: sandbox.metadata,
            tags,
            idle_timeout_seconds: sandbox.idle_timeout_seconds,
            idle_from: sandbox.idle_from.map(|dt| dt.to_rfc3339()),
            busy_from: sandbox.busy_from.map(|dt| dt.to_rfc3339()),
        })
    }
}

// Helper function to find sandbox by ID
async fn find_sandbox_by_id(
    state: &AppState,
    id: &str,
    created_by: &str,
    is_admin: bool,
) -> Result<Sandbox, ApiError> {
    // Try to find by id
    if let Some(sandbox) = Sandbox::find_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch sandbox: {}", e)))?
    {
        // Admins can access any sandbox, regular users only their own
        if is_admin || sandbox.created_by == created_by {
            return Ok(sandbox);
        } else {
            return Err(ApiError::Forbidden(
                "Access denied to this sandbox".to_string(),
            ));
        }
    }

    Err(ApiError::NotFound("Sandbox not found".to_string()))
}

// -------- Sandbox Files (read-only) --------

#[derive(Debug, Deserialize, Default)]
pub struct ListFilesQuery {
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

fn is_safe_relative_path(p: &str) -> bool {
    if p.is_empty() {
        return true;
    }
    if p.starts_with('/') || p.contains('\0') {
        return false;
    }
    !p.split('/').any(|seg| seg == ".." || seg.is_empty())
}

fn map_file_request_error(err: &str) -> ApiError {
    let lower = err.to_ascii_lowercase();
    if lower.contains("too large") {
        ApiError::PayloadTooLarge(err.to_string())
    } else if lower.contains("no such file") || lower.contains("not found") {
        ApiError::NotFound("File or directory not found".to_string())
    } else if lower.contains("is a directory") {
        ApiError::BadRequest("Path is a directory".to_string())
    } else if lower.contains("invalid path") {
        ApiError::BadRequest("Invalid path".to_string())
    } else if lower.contains("terminated")
        || lower.contains("terminating")
        || lower.contains("initializing")
        || lower.contains("deleted")
        || lower.contains("closing")
        || lower.contains("not running")
        || lower.contains("container does not exist")
    {
        ApiError::Conflict("Sandbox not available.".to_string())
    } else if lower.contains("forbidden") || lower.contains("outside") {
        ApiError::Forbidden(err.to_string())
    } else {
        ApiError::Internal(anyhow::anyhow!(err.to_string()))
    }
}

pub async fn read_sandbox_file(
    State(state): State<Arc<AppState>>,
    Path((id, path)): Path<(String, String)>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Response, ApiError> {
    // Admins require explicit permission; owners can access their own sandboxes
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to read files".to_string())
            })?;
    }

    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;

    if !is_safe_relative_path(&path) {
        return Err(ApiError::BadRequest("Invalid path".to_string()));
    }

    // Create file_read request
    let request_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "path": path,
    });
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    sqlx::query(
        r#"INSERT INTO sandbox_requests (id, sandbox_id, request_type, created_by, payload, status)
            VALUES (?, ?, 'file_read', ?, ?, 'pending')"#,
    )
    .bind(&request_id)
    .bind(&sandbox.id)
    .bind(username)
    .bind(&payload)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        ApiError::Internal(anyhow::anyhow!("Failed to create file_read request: {}", e))
    })?;

    // Poll for completion up to 15s
    let start = std::time::Instant::now();
    loop {
        let row =
            sqlx::query(r#"SELECT status, payload, error FROM sandbox_requests WHERE id = ?"#)
                .bind(&request_id)
                .fetch_optional(&*state.db)
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;
        if let Some(row) = row {
            let status: String = row.try_get("status").unwrap_or_default();
            if status == "completed" {
                let payload_val: serde_json::Value =
                    row.try_get("payload").unwrap_or(serde_json::json!({}));
                let res = payload_val
                    .get("result")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                let content_b64 = res
                    .get("content_base64")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let ct = res
                    .get("content_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("application/octet-stream");
                let bytes = BASE64_STANDARD.decode(content_b64).unwrap_or_default();
                let mut builder = Response::builder().status(StatusCode::OK);
                builder = builder.header("content-type", ct);
                builder = builder.header("cache-control", "no-store");
                builder = builder.header(
                    "x-tsbx-file-size",
                    res.get("size")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(bytes.len() as u64)
                        .to_string(),
                );
                let resp = builder
                    .body(axum::body::Body::from(bytes))
                    .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.to_string())))?;
                return Ok(resp);
            } else if status == "failed" {
                let err: String = row
                    .try_get("error")
                    .unwrap_or_else(|_| "file read failed".to_string());
                return Err(map_file_request_error(&err));
            }
        }
        if start.elapsed().as_secs() >= 15 {
            return Err(ApiError::Timeout(
                "Timed out waiting for file read".to_string(),
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

pub async fn get_sandbox_file_metadata(
    State(state): State<Arc<AppState>>,
    Path((id, path)): Path<(String, String)>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to read files".to_string())
            })?;
    }
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;
    if !is_safe_relative_path(&path) {
        return Err(ApiError::BadRequest("Invalid path".to_string()));
    }
    let request_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({ "path": path });
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    sqlx::query(
        r#"INSERT INTO sandbox_requests (id, sandbox_id, request_type, created_by, payload, status)
            VALUES (?, ?, 'file_metadata', ?, ?, 'pending')"#,
    )
    .bind(&request_id)
    .bind(&sandbox.id)
    .bind(username)
    .bind(&payload)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        ApiError::Internal(anyhow::anyhow!(
            "Failed to create file_metadata request: {}",
            e
        ))
    })?;

    let start = std::time::Instant::now();
    loop {
        let row =
            sqlx::query(r#"SELECT status, payload, error FROM sandbox_requests WHERE id = ?"#)
                .bind(&request_id)
                .fetch_optional(&*state.db)
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;
        if let Some(row) = row {
            let status: String = row.try_get("status").unwrap_or_default();
            if status == "completed" {
                let payload_val: serde_json::Value =
                    row.try_get("payload").unwrap_or(serde_json::json!({}));
                let res = payload_val
                    .get("result")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                return Ok(Json(res));
            } else if status == "failed" {
                let err: String = row
                    .try_get("error")
                    .unwrap_or_else(|_| "metadata failed".to_string());
                return Err(map_file_request_error(&err));
            }
        }
        if start.elapsed().as_secs() >= 15 {
            return Err(ApiError::Timeout(
                "Timed out waiting for file metadata".to_string(),
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

pub async fn list_sandbox_files(
    State(state): State<Arc<AppState>>,
    Path((id, path)): Path<(String, String)>,
    Query(paging): Query<ListFilesQuery>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to list files".to_string())
            })?;
    }
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;
    check_not_terminated(&sandbox)?;
    if !is_safe_relative_path(&path) && !path.is_empty() {
        return Err(ApiError::BadRequest("Invalid path".to_string()));
    }
    let request_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "path": path,
        "offset": paging.offset.unwrap_or(0),
        "limit": paging.limit.unwrap_or(100),
    });
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    sqlx::query(
        r#"INSERT INTO sandbox_requests (id, sandbox_id, request_type, created_by, payload, status)
            VALUES (?, ?, 'file_list', ?, ?, 'pending')"#,
    )
    .bind(&request_id)
    .bind(&sandbox.id)
    .bind(username)
    .bind(&payload)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        ApiError::Internal(anyhow::anyhow!("Failed to create file_list request: {}", e))
    })?;

    let start = std::time::Instant::now();
    loop {
        let row =
            sqlx::query(r#"SELECT status, payload, error FROM sandbox_requests WHERE id = ?"#)
                .bind(&request_id)
                .fetch_optional(&*state.db)
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;
        if let Some(row) = row {
            let status: String = row.try_get("status").unwrap_or_default();
            if status == "completed" {
                let payload_val: serde_json::Value =
                    row.try_get("payload").unwrap_or(serde_json::json!({}));
                let res = payload_val
                    .get("result")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                return Ok(Json(res));
            } else if status == "failed" {
                let err: String = row
                    .try_get("error")
                    .unwrap_or_else(|_| "list failed".to_string());
                return Err(map_file_request_error(&err));
            }
        }
        if start.elapsed().as_secs() >= 15 {
            return Err(ApiError::Timeout(
                "Timed out waiting for file list".to_string(),
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

// List at root when no path segment provided
pub async fn list_sandbox_files_root(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(paging): Query<ListFilesQuery>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to list files".to_string())
            })?;
    }
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;
    check_not_terminated(&sandbox)?;
    let request_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "path": "",
        "offset": paging.offset.unwrap_or(0),
        "limit": paging.limit.unwrap_or(100),
    });
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    sqlx::query(
        r#"INSERT INTO sandbox_requests (id, sandbox_id, request_type, created_by, payload, status)
            VALUES (?, ?, 'file_list', ?, ?, 'pending')"#,
    )
    .bind(&request_id)
    .bind(&sandbox.id)
    .bind(username)
    .bind(&payload)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        ApiError::Internal(anyhow::anyhow!("Failed to create file_list request: {}", e))
    })?;

    let start = std::time::Instant::now();
    loop {
        let row =
            sqlx::query(r#"SELECT status, payload, error FROM sandbox_requests WHERE id = ?"#)
                .bind(&request_id)
                .fetch_optional(&*state.db)
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;
        if let Some(row) = row {
            let status: String = row.try_get("status").unwrap_or_default();
            if status == "completed" {
                let payload_val: serde_json::Value =
                    row.try_get("payload").unwrap_or(serde_json::json!({}));
                let res = payload_val
                    .get("result")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                return Ok(Json(res));
            } else if status == "failed" {
                let err: String = row
                    .try_get("error")
                    .unwrap_or_else(|_| "list failed".to_string());
                return Err(ApiError::Internal(anyhow::anyhow!(err)));
            }
        }
        if start.elapsed().as_secs() >= 15 {
            return Err(ApiError::Timeout(
                "Timed out waiting for file list".to_string(),
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

pub async fn delete_sandbox_file(
    State(state): State<Arc<AppState>>,
    Path((id, path)): Path<(String, String)>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to delete files".to_string())
            })?;
    }
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;
    check_not_terminated(&sandbox)?;
    if !is_safe_relative_path(&path) {
        return Err(ApiError::BadRequest("Invalid path".to_string()));
    }
    let request_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({ "path": path });
    sqlx::query(
        r#"INSERT INTO sandbox_requests (id, sandbox_id, request_type, created_by, payload, status)
            VALUES (?, ?, 'file_delete', ?, ?, 'pending')"#,
    )
    .bind(&request_id)
    .bind(&sandbox.id)
    .bind(username)
    .bind(&payload)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        ApiError::Internal(anyhow::anyhow!(
            "Failed to create file_delete request: {}",
            e
        ))
    })?;

    let start = std::time::Instant::now();
    loop {
        let row =
            sqlx::query(r#"SELECT status, payload, error FROM sandbox_requests WHERE id = ?"#)
                .bind(&request_id)
                .fetch_optional(&*state.db)
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;
        if let Some(row) = row {
            let status: String = row.try_get("status").unwrap_or_default();
            if status == "completed" {
                let payload_val: serde_json::Value =
                    row.try_get("payload").unwrap_or(serde_json::json!({}));
                let res = payload_val
                    .get("result")
                    .cloned()
                    .unwrap_or(serde_json::json!({"terminated": true}));
                return Ok(Json(res));
            } else if status == "failed" {
                let err: String = row
                    .try_get("error")
                    .unwrap_or_else(|_| "delete failed".to_string());
                return Err(map_file_request_error(&err));
            }
        }
        if start.elapsed().as_secs() >= 15 {
            return Err(ApiError::Timeout(
                "Timed out waiting for file delete".to_string(),
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

pub async fn list_sandboxes(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListSandboxesQuery>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<PaginatedSandboxes>> {
    // Admins require explicit permission; non-admins can list only their own sandboxes
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_LIST)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to list sandboxes".to_string())
            })?;
    }

    // Resolve principal for ownership constraint
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Normalize pagination
    let limit = query.limit.unwrap_or(30).clamp(1, 100);
    let offset = match query.offset {
        Some(o) if o >= 0 => o,
        _ => {
            let page = query.page.unwrap_or(1).max(1);
            (page - 1) * limit
        }
    };

    // Build dynamic WHERE clauses
    let mut where_sql = String::from(" WHERE 1=1 ");
    let mut binds: Vec<serde_json::Value> = Vec::new();

    if !is_admin {
        where_sql.push_str(" AND created_by = ? ");
        binds.push(serde_json::Value::String(username.to_string()));
    }

    if let Some(state_filter) = query.state.as_ref().map(|s| s.trim().to_string()) {
        if !state_filter.is_empty() {
            where_sql.push_str(" AND state = ? ");
            binds.push(serde_json::Value::String(state_filter));
        }
    }

    if let Some(q) = query.q.as_ref().map(|s| s.trim().to_lowercase()) {
        if !q.is_empty() {
            where_sql.push_str(" AND (description IS NOT NULL AND LOWER(description) LIKE ?) ");
            let pat = format!("%{}%", q);
            binds.push(serde_json::Value::String(pat));
        }
    }

    if let Some(tags) = query.tags.clone() {
        let list: Vec<String> = tags
            .into_iter()
            .flat_map(|s| {
                s.split(',')
                    .map(|t| t.trim().to_lowercase())
                    .collect::<Vec<_>>()
            })
            .filter(|t| !t.is_empty())
            .collect();
        for _t in list {
            where_sql.push_str(" AND JSON_CONTAINS(tags, JSON_QUOTE(?), '$') ");
            binds.push(serde_json::Value::String(_t));
        }
    }

    // Count total
    let count_sql = format!("SELECT COUNT(*) as cnt FROM sandboxes {}", where_sql);
    let mut q_count = sqlx::query_scalar::<_, i64>(&count_sql);
    for b in binds.iter() {
        if let Some(s) = b.as_str() {
            q_count = q_count.bind(s);
        } else {
            // Should not happen for our binds
        }
    }
    let total: i64 = q_count
        .fetch_one(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to count sandboxes: {}", e)))?;

    // Fetch page
    let select_sql = format!(
        r#"
        SELECT id, created_by, state, description, snapshot_id,
               created_at, last_activity_at, metadata, tags,
               idle_timeout_seconds, idle_from, busy_from
        FROM sandboxes
        {}
        ORDER BY created_at DESC
        LIMIT ? OFFSET ?
        "#,
        where_sql
    );
    let mut q_items = sqlx::query_as::<_, Sandbox>(&select_sql);
    for b in binds.iter() {
        if let Some(s) = b.as_str() {
            q_items = q_items.bind(s);
        }
    }
    q_items = q_items.bind(limit).bind(offset);

    let sandboxes: Vec<Sandbox> = q_items
        .fetch_all(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to list sandboxes: {}", e)))?;

    let mut items: Vec<SandboxResponse> = Vec::with_capacity(sandboxes.len());
    for sandbox in sandboxes {
        items.push(SandboxResponse::from_sandbox(sandbox, &state.db).await?);
    }
    let page = if limit > 0 { (offset / limit) + 1 } else { 1 };
    let pages = if limit > 0 {
        ((total + limit - 1) / limit).max(1)
    } else {
        1
    };

    Ok(Json(PaginatedSandboxes {
        items,
        total,
        limit,
        offset,
        page,
        pages,
    }))
}

pub async fn get_sandbox(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SandboxResponse>> {
    // Admins require explicit permission; non-admins can access only their own sandbox
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to get sandbox".to_string())
            })?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find sandbox by id (admin can access any sandbox)
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;

    Ok(Json(
        SandboxResponse::from_sandbox(sandbox, &state.db).await?,
    ))
}

// Cancel a specific task for a sandbox; sets sandbox idle when no active work remains
pub async fn cancel_task(
    State(state): State<Arc<AppState>>,
    Path((id, task_id)): Path<(String, String)>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_UPDATE)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions".to_string()))?;
    }

    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;
    check_not_terminated(&sandbox)?;
    if !is_admin && sandbox.created_by != *username {
        return Err(ApiError::Forbidden(
            "You can only cancel tasks for your own sandboxes".to_string(),
        ));
    }

    let cancelled_item = serde_json::json!({
        "type": "cancelled",
        "reason": "user_cancel",
        "at": chrono::Utc::now().to_rfc3339(),
    });

    let mut cancelled = false;
    if let Some(task) = SandboxTask::find_by_id(&state.db, &task_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?
    {
        if task.sandbox_id != sandbox.id {
            return Err(ApiError::NotFound("Task not found".to_string()));
        }
        let status_lower = task.status.to_lowercase();
        if status_lower == "processing" || status_lower == "pending" {
            let req = UpdateTaskRequest {
                status: Some("cancelled".to_string()),
                input: None,
                output: Some(serde_json::json!({
                    "text": "Task cancelled by user"
                })),
                steps: Some(vec![cancelled_item.clone()]),
                timeout_seconds: None,
                context_length: None,
            };
            SandboxTask::update_by_id(&state.db, &task_id, req)
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to cancel task: {}", e)))?;
            cancelled = true;
        } else {
            return Err(ApiError::Conflict(
                "Task is not in a cancellable state".to_string(),
            ));
        }
    }

    if !cancelled {
        let pending_requests = sqlx::query_as::<_, (String, serde_json::Value)>(
            r#"SELECT id, payload
               FROM sandbox_requests
               WHERE sandbox_id = ?
                 AND request_type = 'create_task'
                 AND status IN ('pending','processing')
               ORDER BY created_at DESC"#,
        )
        .bind(&sandbox.id)
        .fetch_all(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;

        for (request_id, payload) in pending_requests {
            if payload
                .get("task_id")
                .and_then(|v| v.as_str())
                .map(|v| v == task_id)
                .unwrap_or(false)
            {
                let _ = sqlx::query(
                    r#"UPDATE sandbox_requests
                       SET status='completed', updated_at=NOW(), completed_at=NOW(), error='cancelled'
                       WHERE id = ?"#,
                )
                .bind(&request_id)
                .execute(&*state.db)
                .await;

                cancelled = true;
                break;
            }
        }
    }

    if !cancelled {
        return Err(ApiError::NotFound(
            "Task not found or already completed".to_string(),
        ));
    }

    let (remaining_tasks,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM sandbox_tasks WHERE sandbox_id = ? AND status IN ('pending','processing')"#
    )
    .bind(&sandbox.id)
    .fetch_one(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;

    let (remaining_requests,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM sandbox_requests WHERE sandbox_id = ? AND request_type = 'create_task' AND status IN ('pending','processing')"#
    )
    .bind(&sandbox.id)
    .fetch_one(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;

    if remaining_tasks == 0 && remaining_requests == 0 {
        sqlx::query(
            r#"UPDATE sandboxes
               SET state = 'idle', last_activity_at = NOW(), idle_from = NOW(), busy_from = NULL
               WHERE id = ? AND state NOT IN ('terminated','terminating','deleted')"#,
        )
        .bind(&sandbox.id)
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update sandbox: {}", e)))?;
    }

    Ok(Json(
        serde_json::json!({"status":"ok", "sandbox": sandbox.id, "task": task_id, "cancelled": true}),
    ))
}

fn soft_limit_tokens() -> i64 {
    std::env::var("CONTEXT_SOFT_LIMIT_TOKENS")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(128_000)
}

pub async fn create_sandbox(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateSandboxRequest>,
) -> ApiResult<Json<SandboxResponse>> {
    tracing::info!(
        "Creating sandbox with env: {} keys, instructions: {}, setup: {}, startup_task: {}",
        req.env.len(),
        req.instructions.is_some(),
        req.setup.is_some(),
        req.startup_task.is_some()
    );

    // Admins require explicit permission; non-admins can create their own sandboxes
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::SANDBOX_CREATE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to create sandbox".to_string())
            })?;
    }

    // Get the principal name
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    let sandbox = Sandbox::create(&state.db, req.clone(), created_by)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create sandbox: {:?}", e);
            ApiError::Internal(anyhow::anyhow!("Failed to create sandbox: {}", e))
        })?;

    // Add request to queue for sandbox manager to create container with sandbox parameters
    let payload = serde_json::json!({
        "env": req.env,
        "instructions": req.instructions,
        "setup": req.setup,
        "startup_task": req.startup_task,
        "snapshot_id": req.snapshot_id,
        "principal": created_by,
        "principal_type": match &auth.principal {
            crate::shared::rbac::AuthPrincipal::Subject(_) => "User",
            crate::shared::rbac::AuthPrincipal::Operator(_) => "Admin",
        },
        "user_token": auth.token
    });

    sqlx::query(
        r#"
        INSERT INTO sandbox_requests (sandbox_id, request_type, created_by, payload, status)
        VALUES (?, 'create_sandbox', ?, ?, 'pending')
        "#,
    )
    .bind(&sandbox.id)
    .bind(created_by)
    .bind(payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create sandbox request: {}", e)))?;

    tracing::info!("Created sandbox request for sandbox {}", sandbox.id);

    Ok(Json(
        SandboxResponse::from_sandbox(sandbox, &state.db).await?,
    ))
}

pub async fn update_sandbox(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<UpdateSandboxRequest>,
) -> ApiResult<Json<SandboxResponse>> {
    // Admins require explicit permission; owners can update without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::SANDBOX_UPDATE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to update sandbox".to_string())
            })?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find sandbox by ID or name (admin can access any sandbox for update/delete)
    let is_admin = is_admin_principal(&auth, &state).await;
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;

    // Check if sandbox is terminated
    check_not_terminated(&sandbox)?;

    // Enforce ownership: only admin or owner may update
    if !is_admin && sandbox.created_by != *username {
        return Err(ApiError::Forbidden(
            "You can only update your own sandboxes".to_string(),
        ));
    }

    let updated_sandbox = Sandbox::update(&state.db, &sandbox.id, req)
        .await
        .map_err(|e| {
            let error_msg = e.to_string();
            if error_msg.contains("No fields to update") {
                ApiError::BadRequest(error_msg)
            } else {
                ApiError::Internal(anyhow::anyhow!("Failed to update sandbox: {}", e))
            }
        })?
        .ok_or(ApiError::NotFound("Sandbox not found".to_string()))?;

    Ok(Json(
        SandboxResponse::from_sandbox(updated_sandbox, &state.db).await?,
    ))
}

pub async fn update_sandbox_state(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<UpdateSandboxStateRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // Get sandbox and verify ownership (same pattern as other sandbox endpoints)
    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find sandbox by ID or name (admin can access any sandbox for update/delete)
    let is_admin = is_admin_principal(&auth, &state).await;
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;
    if !sandbox.state.eq_ignore_ascii_case("initializing") {
        check_not_terminated(&sandbox)?;
    }

    // Update the state with ownership verification
    let result = sqlx::query(
        "UPDATE sandboxes SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE id = ? AND created_by = ?"
    )
    .bind(&req.state)
    .bind(&sandbox.id)
    .bind(username)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update sandbox state: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound(
            "Sandbox not found or access denied".to_string(),
        ));
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "state": req.state
    })))
}

pub async fn terminate_sandbox(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<()> {
    // Check permission for terminating sandboxes (admin only). Owners can terminate without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::SANDBOX_DELETE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to terminate sandbox".to_string())
            })?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find sandbox by ID or name (admin can access any sandbox for update/delete)
    let is_admin = is_admin_principal(&auth, &state).await;
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;

    let cancelled_item = serde_json::json!({
        "type": "cancelled",
        "reason": "termination",
        "at": chrono::Utc::now().to_rfc3339(),
    });

    let active_tasks = sqlx::query_as::<_, (String,)>(
        r#"SELECT id FROM sandbox_tasks WHERE sandbox_id = ? AND status IN ('pending','processing')"#
    )
    .bind(&sandbox.id)
    .fetch_all(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to load tasks: {}", e)))?;

    for (task_id,) in active_tasks {
        let req = UpdateTaskRequest {
            status: Some("cancelled".to_string()),
            input: None,
            output: Some(serde_json::json!({
                "text": "Task cancelled due to sandbox termination"
            })),
            steps: Some(vec![cancelled_item.clone()]),
            timeout_seconds: None,
            context_length: None,
        };
        SandboxTask::update_by_id(&state.db, &task_id, req)
            .await
            .map_err(|e| {
                ApiError::Internal(anyhow::anyhow!("Failed to cancel task {}: {}", task_id, e))
            })?;
    }

    let pending_requests = sqlx::query_as::<_, (String, serde_json::Value)>(
        r#"SELECT id, payload
           FROM sandbox_requests
           WHERE sandbox_id = ?
             AND request_type = 'create_task'
             AND status IN ('pending','processing')"#,
    )
    .bind(&sandbox.id)
    .fetch_all(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to load pending requests: {}", e)))?;

    for (request_id, payload) in pending_requests {
        let cancelled_payload = payload.get("input").cloned();
        let _ = sqlx::query(
            r#"UPDATE sandbox_requests
               SET status='completed', updated_at=NOW(), completed_at=NOW(), error='cancelled', payload = ?
               WHERE id = ?"#,
        )
        .bind(cancelled_payload.unwrap_or_else(|| serde_json::json!({ "cancelled": true })))
        .bind(&request_id)
        .execute(&*state.db)
        .await;
    }

    // Mark sandbox as terminating so further requests treat it as unavailable
    sqlx::query(
        r#"UPDATE sandboxes
           SET state = 'terminating', last_activity_at = NOW()
           WHERE id = ? AND state <> 'terminated'"#,
    )
    .bind(&sandbox.id)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        ApiError::Internal(anyhow::anyhow!("Failed to mark sandbox terminating: {}", e))
    })?;

    // Terminate sandbox: schedule container stop and mark as terminated
    // Add request to queue for sandbox manager to stop container and set state to terminated
    sqlx::query(
        r#"
        INSERT INTO sandbox_requests (sandbox_id, request_type, created_by, payload, status)
        VALUES (?, 'terminate_sandbox', ?, '{}', 'pending')
        "#,
    )
    .bind(&sandbox.id)
    .bind(username)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create delete request: {}", e)))?;

    tracing::info!("Created delete request for sandbox {}", sandbox.id);

    // The controller will stop the container and set state to 'terminated'
    // The sandbox row remains in DB for history/audit purposes

    Ok(())
}

// GET /sandboxes/{id}/runtime — total runtime across sandboxes
pub async fn get_sandbox_runtime(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Permission: owner or admin
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to get sandbox runtime".to_string())
            })?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find sandbox (admin can access any sandbox)
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;

    // Fetch all tasks for this sandbox (created_at + output JSON)
    let rows: Vec<(DateTime<Utc>, serde_json::Value)> = sqlx::query_as(
        r#"SELECT created_at, steps FROM sandbox_tasks WHERE sandbox_id = ? ORDER BY created_at ASC"#
    )
    .bind(&sandbox.id)
    .fetch_all(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch tasks: {}", e)))?;

    // Sum runtime for completed sandboxes; track last restart marker for current sandbox inclusion
    let mut total: i64 = 0;
    let mut last_restarted: Option<DateTime<Utc>> = None;
    let mut current_sandbox: i64 = 0;
    for (row_created_at, output) in rows.into_iter() {
        if let Some(items) = output.as_array() {
            for it in items {
                let t = it.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if t == "restarted" {
                    let at = it
                        .get("at")
                        .and_then(|v| v.as_str())
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or(row_created_at);
                    last_restarted = Some(at);
                } else if t == "terminated" || t == "deleted" {
                    // Prefer embedded runtime_seconds, else compute delta
                    if let Some(rs) = it.get("runtime_seconds").and_then(|v| v.as_i64()) {
                        if rs > 0 {
                            total += rs;
                        }
                    } else {
                        let end_at = it
                            .get("at")
                            .and_then(|v| v.as_str())
                            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or(row_created_at);
                        let start_at = last_restarted.unwrap_or(sandbox.created_at);
                        let delta = (end_at - start_at).num_seconds();
                        if delta > 0 {
                            total += delta;
                        }
                    }
                }
            }
        }
    }

    // Include current sandbox up to now when sandbox is not terminated
    if sandbox.state.to_lowercase() != crate::shared::models::constants::SANDBOX_STATE_TERMINATED {
        let start_at = last_restarted.unwrap_or(sandbox.created_at);
        let now = Utc::now();
        let delta = (now - start_at).num_seconds();
        if delta > 0 {
            total += delta;
            current_sandbox = delta;
        }
    }

    Ok(Json(serde_json::json!({
        "sandbox_id": sandbox.id,
        "total_runtime_seconds": total,
        "current_sandbox_seconds": current_sandbox
    })))
}

// GET /sandboxes/{id}/top — realtime CPU/memory snapshot and task counts
pub async fn get_sandbox_top(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SandboxTopResponse>> {
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to inspect sandbox".to_string())
            })?;
    }

    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;

    let tasks_completed: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM sandbox_tasks WHERE sandbox_id = ? AND LOWER(status) = 'completed'"#,
    )
    .bind(&sandbox.id)
    .fetch_one(&*state.db)
    .await
    .map_err(|e| {
        ApiError::Internal(anyhow::anyhow!(
            "Failed to count completed tasks for sandbox {}: {}",
            sandbox.id,
            e
        ))
    })?;

    let container_name = format!("tsbx_sandbox_{}", sandbox.id);
    let mut container_state = sandbox.state.clone();
    let mut cpu_usage_percent = 0.0_f64;
    let mut cpu_limit_cores = 0.0_f64;
    let mut memory_usage_bytes = 0_u64;
    let mut memory_limit_bytes = 0_u64;

    match fetch_container_metrics(&container_name).await {
        Ok(Some(metrics)) => {
            container_state = metrics.state;
            cpu_usage_percent = metrics.cpu_percent;
            cpu_limit_cores = metrics.cpu_limit_cores;
            memory_usage_bytes = metrics.memory_usage;
            memory_limit_bytes = metrics.memory_limit;
        }
        Ok(None) => {
            container_state = "not_found".to_string();
        }
        Err(e) => {
            warn!(
                "Failed to read container metrics for {}: {}",
                container_name, e
            );
            container_state = format!("{} (metrics unavailable)", container_state);
        }
    }

    let response = SandboxTopResponse {
        sandbox_id: sandbox.id,
        container_state,
        tasks_completed,
        cpu_usage_percent,
        cpu_limit_cores,
        memory_usage_bytes,
        memory_limit_bytes,
        captured_at: Utc::now().to_rfc3339(),
    };

    Ok(Json(response))
}

async fn fetch_container_metrics(container_name: &str) -> anyhow::Result<Option<ContainerMetrics>> {
    let docker = Docker::connect_with_socket_defaults()
        .map_err(|e| anyhow::anyhow!("Docker connection failed: {}", e))?;

    let inspect = match docker
        .inspect_container(container_name, None::<InspectContainerOptions>)
        .await
    {
        Ok(resp) => resp,
        Err(BollardError::DockerResponseServerError { status_code, .. }) if status_code == 404 => {
            return Ok(None);
        }
        Err(err) => {
            return Err(anyhow::anyhow!(
                "Inspect failed for {}: {}",
                container_name,
                err
            ));
        }
    };

    let container_state = inspect
        .state
        .as_ref()
        .and_then(|s| s.status.as_ref().map(|status| status.to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    let host_config = inspect.host_config.unwrap_or_default();
    let mut memory_limit_bytes = host_config
        .memory
        .map(|v| if v < 0 { 0 } else { v as u64 })
        .unwrap_or(0);

    let mut cpu_limit_cores = if let Some(nano_cpus) = host_config.nano_cpus {
        if nano_cpus > 0 {
            nano_cpus as f64 / 1_000_000_000.0
        } else {
            0.0
        }
    } else {
        0.0
    };
    if cpu_limit_cores <= 0.0 {
        if let (Some(quota), Some(period)) = (host_config.cpu_quota, host_config.cpu_period) {
            if quota > 0 && period > 0 {
                cpu_limit_cores = quota as f64 / period as f64;
            }
        }
    }

    let mut stats_stream = docker.stats(
        container_name,
        Some(StatsOptions {
            stream: false,
            one_shot: true,
        }),
    );

    let mut cpu_percent = 0.0;
    let mut memory_usage_bytes = 0_u64;

    if let Some(stats_result) = stats_stream.next().await {
        match stats_result {
            Ok(stats) => {
                cpu_percent = compute_cpu_percent(&stats);
                let (usage, limit_from_stats) = compute_memory_usage(&stats);
                memory_usage_bytes = usage;
                if memory_limit_bytes == 0 && limit_from_stats > 0 {
                    memory_limit_bytes = limit_from_stats;
                } else if limit_from_stats > 0 {
                    memory_limit_bytes = memory_limit_bytes.max(limit_from_stats);
                }
            }
            Err(BollardError::DockerResponseServerError { status_code, .. })
                if status_code == 404 =>
            {
                return Ok(None);
            }
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "Failed to collect stats for {}: {}",
                    container_name,
                    err
                ));
            }
        }
    }

    Ok(Some(ContainerMetrics {
        state: container_state,
        cpu_percent,
        cpu_limit_cores,
        memory_usage: memory_usage_bytes,
        memory_limit: memory_limit_bytes,
    }))
}

fn compute_cpu_percent(stats: &Stats) -> f64 {
    let cpu_delta = stats
        .cpu_stats
        .cpu_usage
        .total_usage
        .saturating_sub(stats.precpu_stats.cpu_usage.total_usage);
    let system_delta = stats
        .cpu_stats
        .system_cpu_usage
        .unwrap_or(0)
        .saturating_sub(stats.precpu_stats.system_cpu_usage.unwrap_or(0));
    let online_cpus = stats.cpu_stats.online_cpus.unwrap_or(1) as f64;

    if cpu_delta > 0 && system_delta > 0 && online_cpus > 0.0 {
        (cpu_delta as f64 / system_delta as f64) * online_cpus * 100.0
    } else {
        0.0
    }
}

fn compute_memory_usage(stats: &Stats) -> (u64, u64) {
    let mut usage = stats.memory_stats.usage.unwrap_or(0);
    if let Some(MemoryStatsStats::V1(v1)) = stats.memory_stats.stats {
        usage = usage.saturating_sub(v1.cache);
    }
    let limit = stats.memory_stats.limit.unwrap_or(0);
    (usage, limit)
}

pub async fn update_sandbox_to_busy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Only the sandbox container should be able to call this
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find sandbox (sandbox token should match sandbox ownership)
    let is_admin = is_admin_principal(&auth, &state).await;
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;
    check_not_terminated(&sandbox)?;

    // Update sandbox to busy: clear idle_from and set busy_from (pauses stop timeout)
    Sandbox::update_sandbox_to_busy(&state.db, &sandbox.id)
        .await
        .map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("Failed to update sandbox to busy: {}", e))
        })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "state": "busy",
        "timeout_status": "paused"
    })))
}

pub async fn update_sandbox_to_idle(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Only the sandbox container should be able to call this
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find sandbox (sandbox token should match sandbox ownership)
    let is_admin = is_admin_principal(&auth, &state).await;
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;
    if !sandbox.state.eq_ignore_ascii_case("initializing") {
        check_not_terminated(&sandbox)?;
    }

    // Update sandbox to idle: set idle_from and clear busy_from (idle timeout active)
    Sandbox::update_sandbox_to_idle(&state.db, &sandbox.id)
        .await
        .map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("Failed to update sandbox to idle: {}", e))
        })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "state": "idle",
        "timeout_status": "active"
    })))
}

pub async fn list_tasks(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<ListTasksQuery>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<Vec<TaskSummary>>> {
    let sandbox = crate::shared::models::Sandbox::find_by_id(&state.db, &id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Sandbox not found".to_string()))?;

    let list = SandboxTask::find_by_sandbox(&state.db, &sandbox.id, query.limit, query.offset)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch tasks: {}", e)))?;

    let tasks = list
        .into_iter()
        .map(|task| TaskSummary {
            id: task.id,
            sandbox_id: task.sandbox_id,
            status: task.status,
            input_content: extract_input_content(&task.input),
            context_length: task.context_length,
            timeout_seconds: task.timeout_seconds,
            timeout_at: task.timeout_at.map(|dt| dt.to_rfc3339()),
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(tasks))
}

pub async fn create_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateTaskRequest>,
) -> ApiResult<Json<TaskView>> {
    use tokio::time::{sleep, Duration, Instant};

    let sandbox = crate::shared::models::Sandbox::find_by_id(&state.db, &id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Sandbox not found".to_string()))?;

    check_not_terminated(&sandbox)?;

    let limit_tokens = soft_limit_tokens();
    let used_tokens = SandboxTask::latest_context_length(&state.db, &sandbox.id)
        .await
        .map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("Failed to fetch context length: {}", e))
        })?;
    if used_tokens >= limit_tokens {
        return Err(ApiError::Conflict(format!(
            "Context is full ({} / {} tokens). Terminate and relaunch sandbox {} to continue.",
            used_tokens, limit_tokens, sandbox.id
        )));
    }

    if sandbox.state == crate::shared::models::constants::SANDBOX_STATE_BUSY {
        return Err(ApiError::Conflict("Sandbox is busy".to_string()));
    }

    if let Some(timeout) = req.timeout_seconds {
        if timeout < 0 {
            return Err(ApiError::BadRequest(
                "timeout_seconds must be a non-negative integer".to_string(),
            ));
        }
    }

    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    if sandbox.state == crate::shared::models::constants::SANDBOX_STATE_IDLE {
        sqlx::query(
            r#"UPDATE sandboxes SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE id = ? AND state = ?"#,
        )
        .bind(crate::shared::models::constants::SANDBOX_STATE_BUSY)
        .bind(&sandbox.id)
        .bind(&sandbox.state)
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update sandbox state: {}", e)))?;
    }

    let task_id = uuid::Uuid::new_v4().to_string();
    let timeout_seconds = req.timeout_seconds.or(Some(300));

    let payload = serde_json::json!({
        "task_id": task_id,
        "input": req.input,
        "background": req.background.unwrap_or(true),
        "timeout_seconds": timeout_seconds
    });
    sqlx::query(
        r#"
        INSERT INTO sandbox_requests (sandbox_id, request_type, created_by, payload, status)
        VALUES (?, 'create_task', ?, ?, 'pending')
        "#,
    )
    .bind(&sandbox.id)
    .bind(created_by)
    .bind(payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create task request: {}", e)))?;

    let background = req.background.unwrap_or(true);
    if !background {
        let start = Instant::now();
        let timeout = Duration::from_secs(15 * 60);
        let poll_interval = Duration::from_millis(500);

        loop {
            if start.elapsed() >= timeout {
                return Err(ApiError::Timeout(
                    "Timed out waiting for task to complete".to_string(),
                ));
            }

            match SandboxTask::find_by_id(&state.db, &task_id).await {
                Ok(Some(cur)) => {
                    let status_lc = cur.status.to_lowercase();
                    if matches!(status_lc.as_str(), "completed" | "failed" | "cancelled") {
                        return Ok(Json(TaskView {
                            id: cur.id,
                            sandbox_id: cur.sandbox_id,
                            status: cur.status,
                            input_content: extract_input_content(&cur.input),
                            output_content: extract_output_content(&cur.output, &cur.steps),
                            segments: extract_segments(&cur.steps),
                            steps: extract_steps(&cur.steps),
                            output: cur.output.clone(),
                            context_length: cur.context_length,
                            timeout_seconds: cur.timeout_seconds,
                            timeout_at: cur.timeout_at.map(|dt| dt.to_rfc3339()),
                            created_at: cur.created_at.to_rfc3339(),
                            updated_at: cur.updated_at.to_rfc3339(),
                        }));
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    return Err(ApiError::Internal(anyhow::anyhow!(
                        "Failed to fetch task: {}",
                        e
                    )));
                }
            }

            sleep(poll_interval).await;
        }
    }

    let now = Utc::now();
    let timeout_seconds_value = req.timeout_seconds.or(Some(300)).filter(|v| *v > 0);
    let timeout_at = timeout_seconds_value.and_then(|secs| {
        now.checked_add_signed(chrono::Duration::seconds(secs as i64))
            .map(|dt| dt.to_rfc3339())
    });

    Ok(Json(TaskView {
        id: task_id,
        sandbox_id: sandbox.id.clone(),
        status: "pending".to_string(),
        input_content: extract_input_content(&req.input),
        output_content: vec![],
        segments: vec![],
        steps: vec![],
        output: serde_json::json!({
            "text": "",
            "content": []
        }),
        context_length: 0,
        timeout_seconds: timeout_seconds_value,
        timeout_at,
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
    }))
}

pub async fn get_task_by_id(
    State(state): State<Arc<AppState>>,
    Path((sandbox_id, task_id)): Path<(String, String)>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<TaskView>> {
    let _sandbox = crate::shared::models::Sandbox::find_by_id(&state.db, &sandbox_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Sandbox not found".to_string()))?;

    let cur = SandboxTask::find_by_id(&state.db, &task_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch task: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    Ok(Json(TaskView {
        id: cur.id,
        sandbox_id: cur.sandbox_id,
        status: cur.status,
        input_content: extract_input_content(&cur.input),
        output_content: extract_output_content(&cur.output, &cur.steps),
        segments: extract_segments(&cur.steps),
        steps: extract_steps(&cur.steps),
        output: cur.output.clone(),
        context_length: cur.context_length,
        timeout_seconds: cur.timeout_seconds,
        timeout_at: cur.timeout_at.map(|dt| dt.to_rfc3339()),
        created_at: cur.created_at.to_rfc3339(),
        updated_at: cur.updated_at.to_rfc3339(),
    }))
}

pub async fn update_task(
    State(state): State<Arc<AppState>>,
    Path((sandbox_id, task_id)): Path<(String, String)>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<UpdateTaskRequest>,
) -> ApiResult<Json<TaskView>> {
    let sandbox = crate::shared::models::Sandbox::find_by_id(&state.db, &sandbox_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Sandbox not found".to_string()))?;

    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    let is_admin = is_admin_principal(&auth, &state).await;
    if !is_admin && sandbox.created_by != *username {
        return Err(ApiError::Forbidden(
            "You can only update tasks for your own sandboxes".to_string(),
        ));
    }

    check_not_terminated(&sandbox)?;

    if let Some(existing) = SandboxTask::find_by_id(&state.db, &task_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?
    {
        if existing.sandbox_id != sandbox_id {
            return Err(ApiError::NotFound("Task not found".to_string()));
        }
    } else {
        return Err(ApiError::NotFound("Task not found".to_string()));
    }

    if let Some(timeout) = req.timeout_seconds {
        if timeout < 0 {
            return Err(ApiError::BadRequest(
                "timeout_seconds must be a non-negative integer".to_string(),
            ));
        }
    }

    let updated = SandboxTask::update_by_id(&state.db, &task_id, req)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update task: {}", e)))?;

    Ok(Json(TaskView {
        id: updated.id,
        sandbox_id: updated.sandbox_id,
        status: updated.status,
        input_content: extract_input_content(&updated.input),
        output_content: extract_output_content(&updated.output, &updated.steps),
        segments: extract_segments(&updated.steps),
        steps: extract_steps(&updated.steps),
        output: updated.output.clone(),
        context_length: updated.context_length,
        timeout_seconds: updated.timeout_seconds,
        timeout_at: updated.timeout_at.map(|dt| dt.to_rfc3339()),
        created_at: updated.created_at.to_rfc3339(),
        updated_at: updated.updated_at.to_rfc3339(),
    }))
}

pub async fn get_task_count(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    let sandbox = crate::shared::models::Sandbox::find_by_id(&state.db, &id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Sandbox not found".to_string()))?;

    let count = SandboxTask::count_by_sandbox(&state.db, &sandbox.id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to count tasks: {}", e)))?;

    Ok(Json(
        serde_json::json!({ "count": count, "sandbox_id": sandbox.id }),
    ))
}

fn extract_input_content(input: &serde_json::Value) -> Vec<serde_json::Value> {
    input
        .get("content")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

fn extract_output_content(
    output: &serde_json::Value,
    steps: &serde_json::Value,
) -> Vec<serde_json::Value> {
    compute_output_content(output, steps)
}

fn extract_segments(steps: &serde_json::Value) -> Vec<serde_json::Value> {
    extract_steps(steps)
}
