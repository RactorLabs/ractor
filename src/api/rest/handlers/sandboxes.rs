use axum::http::StatusCode;
use axum::response::Response;
use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::de::{self, Deserializer, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::api::rest::rbac_enforcement::{check_api_permission, permissions};
use crate::shared::models::{
    AppState, CreateSandboxRequest, Sandbox, UpdateSandboxRequest, UpdateSandboxStateRequest,
};
use crate::shared::rbac::PermissionContext;
// Use fully-qualified names for task records to avoid name conflict with local SandboxResponse
use crate::shared::models::task as task_model;

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

// Helper: check if sandbox is deleted and return error if attempting write operation
fn check_not_deleted(sandbox: &Sandbox) -> ApiResult<()> {
    if sandbox.state == "deleted" {
        return Err(ApiError::BadRequest(
            "Sandbox is deleted. Please create a new one.".to_string(),
        ));
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
    pub context_cutoff_at: Option<String>,
    pub last_context_length: i64,
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
            context_cutoff_at: sandbox.context_cutoff_at.map(|dt| dt.to_rfc3339()),
            last_context_length: sandbox.last_context_length,
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
    } else if lower.contains("deleted")
        || lower.contains("closing")
        || lower.contains("not running")
        || lower.contains("container does not exist")
    {
        ApiError::Conflict("Sandbox is deleted".to_string())
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
    if sandbox.state == "deleted" {
        return Err(ApiError::Conflict("Sandbox is deleted".to_string()));
    }
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
    if sandbox.state == "deleted" {
        return Err(ApiError::Conflict("Sandbox is deleted".to_string()));
    }
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
    check_not_deleted(&sandbox)?;
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
                    .unwrap_or(serde_json::json!({"deleted": true}));
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
               idle_timeout_seconds, idle_from, busy_from, context_cutoff_at,
               last_context_length
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

// Cancel the latest in-progress task for a sandbox and set sandbox to idle
pub async fn cancel_active_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Admins must have SANDBOX_UPDATE; owners can cancel their own without RBAC grant
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_UPDATE)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions".to_string()))?;
    }

    // Resolve principal identity
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Confirm access to the sandbox; enforce ownership for non-admins
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;
    check_not_deleted(&sandbox)?;
    if !is_admin && sandbox.created_by != *username {
        return Err(ApiError::Forbidden(
            "You can only cancel your own sandboxes".to_string(),
        ));
    }

    // Find latest in-progress task (processing or pending)
    let row: Option<(String, serde_json::Value)> = sqlx::query_as(
        r#"SELECT id, output FROM sandbox_tasks WHERE sandbox_id = ? AND status IN ('processing','pending') ORDER BY created_at DESC LIMIT 1"#
    )
    .bind(&sandbox.id)
    .fetch_optional(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;

    let mut cancelled = false;
    if let Some((task_id, output)) = row {
        let mut new_output = output.clone();
        let mut items = new_output
            .get("items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_else(Vec::new);
        items.push(serde_json::json!({
            "type": "cancelled", "reason": "user_cancel", "at": chrono::Utc::now().to_rfc3339()
        }));
        if let serde_json::Value::Object(ref mut map) = new_output {
            map.insert("items".to_string(), serde_json::Value::Array(items));
        } else {
            new_output = serde_json::json!({"text":"","items":[{"type":"cancelled","reason":"user_cancel","at": chrono::Utc::now().to_rfc3339()}]});
        }
        sqlx::query(
            r#"UPDATE sandbox_tasks SET status = 'cancelled', output = ?, updated_at = NOW() WHERE id = ?"#
        )
        .bind(&new_output)
        .bind(&task_id)
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;
        cancelled = true;
    }

    // If no task row, try to cancel a queued create_task request (pre-insert race)
    if !cancelled {
        if let Some((request_id, created_by, payload)) = sqlx::query_as::<_, (String, String, serde_json::Value)>(
            r#"SELECT id, created_by, payload FROM sandbox_requests WHERE sandbox_id = ? AND request_type = 'create_task' AND status IN ('pending','processing') ORDER BY created_at DESC LIMIT 1"#
        )
        .bind(&sandbox.id)
        .fetch_optional(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))? {
            let task_id = payload.get("task_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if !task_id.is_empty() {
                let input = payload.get("input").cloned().unwrap_or_else(|| serde_json::json!({"text":""}));
                let now = chrono::Utc::now();
                let cancelled_item = serde_json::json!({"type":"cancelled","reason":"user_cancel","at": now.to_rfc3339()});
                let output = serde_json::json!({"text":"","items":[cancelled_item]});
                // Insert cancelled task row (idempotent behavior if it already exists)
                let _ = sqlx::query(
                    r#"INSERT INTO sandbox_tasks (id, sandbox_id, created_by, status, input, output, created_at, updated_at)
                        VALUES (?, ?, ?, 'cancelled', ?, ?, NOW(), NOW())
                        ON DUPLICATE KEY UPDATE status='cancelled', output=VALUES(output), updated_at=NOW()"#
                )
                .bind(&task_id)
                .bind(&sandbox.id)
                .bind(&created_by)
                .bind(&input)
                .bind(&output)
                .execute(&*state.db)
                .await;
                // Mark update completed to prevent later insertion
                let _ = sqlx::query(r#"UPDATE sandbox_requests SET status='completed', updated_at=NOW(), completed_at=NOW(), error='cancelled' WHERE id = ?"#)
                    .bind(&request_id)
                    .execute(&*state.db)
                    .await;
                cancelled = true;
            }
        }
    }

    // Set sandbox to idle
    sqlx::query(r#"UPDATE sandboxes SET state = 'idle', last_activity_at = NOW(), idle_from = NOW(), busy_from = NULL WHERE id = ?"#)
        .bind(&sandbox.id)
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update sandbox: {}", e)))?;

    Ok(Json(
        serde_json::json!({"status":"ok", "sandbox": sandbox.id, "cancelled": cancelled}),
    ))
}

#[derive(Debug, Serialize)]
pub struct SandboxContextUsageResponse {
    pub sandbox: String,
    pub soft_limit_tokens: i64,
    pub used_tokens_estimated: i64,
    pub used_percent: f64,
    pub basis: String,
    pub cutoff_at: Option<String>,
    pub measured_at: String,
    pub total_messages_considered: u32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateContextUsageRequest {
    pub tokens: i64,
}

fn soft_limit_tokens() -> i64 {
    std::env::var("CONTEXT_SOFT_LIMIT_TOKENS")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(128_000)
}

#[allow(dead_code)]
fn avg_chars_per_token() -> f64 {
    std::env::var("AVG_CHARS_PER_TOKEN")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(4.0)
}

#[allow(dead_code)]
async fn estimate_history_tokens_since(
    pool: &sqlx::MySqlPool,
    sandbox_id: &str,
    cutoff: Option<DateTime<Utc>>,
) -> Result<(i64, u32), ApiError> {
    // No ordering needed for estimation; avoid sort pressure
    let rows = if let Some(cut) = cutoff {
        sqlx::query(
            r#"SELECT status, input, output, created_at FROM sandbox_tasks WHERE sandbox_id = ? AND created_at >= ?"#,
        )
            .bind(sandbox_id)
            .bind(cut)
            .fetch_all(pool)
            .await
    } else {
        sqlx::query(
            r#"SELECT status, input, output, created_at FROM sandbox_tasks WHERE sandbox_id = ?"#,
        )
            .bind(sandbox_id)
            .fetch_all(pool)
            .await
    }
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;

    let mut total_chars: i64 = 0;
    let mut msg_count: u32 = 0;
    const TOOL_RESULT_PREVIEW_MAX: usize = 100;

    // Determine the single latest 'processing' task by created_at
    let mut latest_proc: Option<DateTime<Utc>> = None;
    for row in rows.iter() {
        let status: String = row
            .try_get::<String, _>("status")
            .unwrap_or_else(|_| "completed".to_string());
        if status.to_lowercase() == "processing" {
            if let Ok(ca) = row.try_get::<DateTime<Utc>, _>("created_at") {
                if latest_proc.map(|x| ca > x).unwrap_or(true) {
                    latest_proc = Some(ca);
                }
            }
        }
    }
    for row in rows {
        let status: String = row
            .try_get::<String, _>("status")
            .unwrap_or_else(|_| "completed".to_string());
        let input: serde_json::Value = row.try_get("input").unwrap_or(serde_json::json!({}));
        let output: serde_json::Value = row.try_get("output").unwrap_or(serde_json::json!({}));

        // Count user messages (input.content text items; include legacy input.text)
        if let Some(user_text) = input.get("text").and_then(|v| v.as_str()) {
            if !user_text.trim().is_empty() {
                total_chars += user_text.len() as i64;
                msg_count += 1;
            }
        }
        if let Some(arr) = input.get("content").and_then(|v| v.as_array()) {
            for it in arr {
                let t = it.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if t.eq_ignore_ascii_case("text") {
                    if let Some(s) = it.get("content").and_then(|v| v.as_str()) {
                        if !s.trim().is_empty() {
                            total_chars += s.len() as i64;
                            msg_count += 1;
                        }
                    }
                }
            }
        }

        let status_lc = status.to_lowercase();
        if status_lc == "processing" {
            // Only include tools for the single most recent processing task
            let include_tools = latest_proc
                .and_then(|lp| {
                    row.try_get::<DateTime<Utc>, _>("created_at")
                        .ok()
                        .map(|c| c == lp)
                })
                .unwrap_or(false);
            if include_tools {
                // For the task currently being worked on, include tool actions and tool_result outputs
                if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
                    for it in items {
                        if it.get("type").and_then(|v| v.as_str()) == Some("tool_call") {
                            let tool = it.get("tool").and_then(|v| v.as_str()).unwrap_or("");
                            let arguments = it
                                .get("arguments")
                                .cloned()
                                .or_else(|| it.get("args").cloned())
                                .unwrap_or_else(|| serde_json::json!({}));
                            let s = serde_json::json!({"action":"tool","tool": tool, "arguments": arguments})
                                .to_string();
                            total_chars += s.len() as i64;
                            msg_count += 1;
                        }
                        if it.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                            if let Some(out) = it.get("output") {
                                let text = out
                                    .as_str()
                                    .map(|x| x.to_string())
                                    .unwrap_or_else(|| out.to_string());
                                if !text.is_empty() {
                                    let len = if text.len() > TOOL_RESULT_PREVIEW_MAX {
                                        (TOOL_RESULT_PREVIEW_MAX + 1) as i64
                                    } else {
                                        text.len() as i64
                                    };
                                    total_chars += len;
                                    msg_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        if status_lc != "processing" {
            if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
                for it in items {
                    if it.get("type").and_then(|v| v.as_str()) == Some("tool_call") {
                        let tool = it.get("tool").and_then(|v| v.as_str()).unwrap_or("");
                        let arguments = it
                            .get("arguments")
                            .cloned()
                            .or_else(|| it.get("args").cloned())
                            .unwrap_or_else(|| serde_json::json!({}));
                        let s = serde_json::json!({"action":"tool","tool": tool, "arguments": arguments})
                            .to_string();
                        total_chars += s.len() as i64;
                        msg_count += 1;
                    } else if it.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                        if let Some(out) = it.get("output") {
                            let text = out
                                .as_str()
                                .map(|x| x.to_string())
                                .unwrap_or_else(|| out.to_string());
                            if !text.is_empty() {
                                let len = if text.len() > TOOL_RESULT_PREVIEW_MAX {
                                    (TOOL_RESULT_PREVIEW_MAX + 1) as i64
                                } else {
                                    text.len() as i64
                                };
                                total_chars += len;
                                msg_count += 1;
                            }
                        }
                    }
                }
            }
        }
        if status_lc == "completed" {
            // Completed tasks: include only the synthesized assistant message built from output_content
            const MAX_TOTAL: usize = 3000;
            const MAX_ITEM: usize = 1200;
            // Extract items from the last 'output' tool_result if present
            let mut out_items: Vec<serde_json::Value> = Vec::new();
            if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
                for it in items.iter().rev() {
                    let is_output = it.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                        && it.get("tool").and_then(|v| v.as_str()) == Some("output");
                    if is_output {
                        if let Some(arr) = it
                            .get("output")
                            .and_then(|v| v.get("items"))
                            .and_then(|v| v.as_array())
                        {
                            out_items = arr.clone();
                        }
                        break;
                    }
                }
            }
            let mut used: usize = 0;
            let mut parts: Vec<String> = Vec::new();
            if !out_items.is_empty() {
                for it in out_items.iter() {
                    if used >= MAX_TOTAL {
                        break;
                    }
                    let typ = it
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let title = it.get("title").and_then(|v| v.as_str());
                    if let Some(t) = title {
                        let h = format!("## {}\n", t);
                        used = used.saturating_add(h.len());
                        parts.push(h);
                    }
                    match typ.as_str() {
                        "markdown" => {
                            if let Some(s) = it.get("content").and_then(|v| v.as_str()) {
                                let mut chunk = s.trim().to_string();
                                if chunk.len() > MAX_ITEM {
                                    chunk.truncate(MAX_ITEM);
                                }
                                used = used.saturating_add(chunk.len());
                                parts.push(chunk);
                            }
                        }
                        "json" => {
                            let val = it
                                .get("content")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            let mut chunk = serde_json::to_string_pretty(&val)
                                .unwrap_or_else(|_| val.to_string());
                            if chunk.len() > MAX_ITEM {
                                chunk.truncate(MAX_ITEM);
                            }
                            used = used.saturating_add(chunk.len());
                            parts.push(format!("```json\n{}\n```", chunk));
                        }
                        "url" => {
                            if let Some(u) = it.get("content").and_then(|v| v.as_str()) {
                                let line = if let Some(tl) = title {
                                    format!("- [{}]({})", tl, u)
                                } else {
                                    u.to_string()
                                };
                                used = used.saturating_add(line.len());
                                parts.push(line);
                            }
                        }
                        _ => {}
                    }
                }
            } else if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
                // Fallback: compact_summary segment content
                for it in items.iter() {
                    let t = it.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if t.eq_ignore_ascii_case("compact_summary") {
                        if let Some(s) = it.get("content").and_then(|v| v.as_str()) {
                            let summary = s.trim().to_string();
                            if !summary.is_empty() {
                                // In fallback compact_summary path, we don't track `used` for truncation here
                                parts.push(summary);
                            }
                        }
                        break;
                    }
                }
            }
            if !parts.is_empty() {
                let content = parts.join("\n\n");
                total_chars += content.len() as i64;
                msg_count += 1;
            }
        }
    }

    let est_tokens = ((total_chars as f64) / avg_chars_per_token()).ceil() as i64;
    Ok((est_tokens, msg_count))
}

pub async fn get_sandbox_context(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SandboxContextUsageResponse>> {
    // Reuse GET permission
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to get sandbox context".to_string())
            })?;
    }

    // Get username
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;
    let used = sandbox.last_context_length;
    let limit = soft_limit_tokens();
    let used_percent = if limit > 0 {
        (used as f64 * 100.0) / (limit as f64)
    } else {
        0.0
    };

    let resp = SandboxContextUsageResponse {
        sandbox: sandbox.id,
        soft_limit_tokens: limit,
        used_tokens_estimated: used,
        used_percent,
        basis: "inference_last_context_length".to_string(),
        cutoff_at: sandbox.context_cutoff_at.map(|dt| dt.to_rfc3339()),
        measured_at: Utc::now().to_rfc3339(),
        total_messages_considered: 0,
    };

    Ok(Json(resp))
}

pub async fn clear_sandbox_context(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SandboxContextUsageResponse>> {
    // Require update permission
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_UPDATE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to clear context".to_string())
            })?;
    }

    // Ownership
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Confirm access to the sandbox
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;
    check_not_deleted(&sandbox)?;

    // Set the cutoff now
    crate::shared::models::Sandbox::clear_context_cutoff(&state.db, &sandbox.id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to set context cutoff: {}", e)))?;

    // Record a history marker indicating context was cleared
    if let Ok(created) = task_model::SandboxTask::create(
        &state.db,
        &sandbox.id,
        username,
        task_model::CreateTaskRequest {
            input: serde_json::json!({ "content": [] }),
            background: None,
            timeout_seconds: None,
        },
    )
    .await
    {
        let cutoff_now = Utc::now().to_rfc3339();
        let _ = task_model::SandboxTask::update_by_id(
            &state.db,
            &created.id,
            task_model::UpdateTaskRequest {
                status: Some("completed".to_string()),
                input: None,
                output: Some(serde_json::json!({
                    "text": "",
                    "items": [ { "type": "context_cleared", "cutoff_at": cutoff_now } ]
                })),
                timeout_seconds: None,
            },
        )
        .await;
    }

    // Return fresh measurement (reset to zero)
    let limit = soft_limit_tokens();
    let now = Utc::now().to_rfc3339();
    let resp = SandboxContextUsageResponse {
        sandbox: sandbox.id,
        soft_limit_tokens: limit,
        used_tokens_estimated: 0,
        used_percent: 0.0,
        basis: "inference_last_context_length".to_string(),
        cutoff_at: Some(now.clone()),
        measured_at: now,
        total_messages_considered: 0,
    };
    Ok(Json(resp))
}

// Compact context: summarize recent conversation and set a new cutoff.
pub async fn compact_sandbox_context(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SandboxContextUsageResponse>> {
    // Require update permission
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SANDBOX_UPDATE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to compact context".to_string())
            })?;
    }

    // Resolve principal name
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Confirm access to the sandbox
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;
    check_not_deleted(&sandbox)?;

    // Load conversation history since the current cutoff (if any)
    let cutoff = sandbox.context_cutoff_at;
    let rows = if let Some(cut) = cutoff {
        sqlx::query(
            r#"SELECT input, output FROM sandbox_tasks WHERE sandbox_id = ? AND created_at >= ? ORDER BY created_at ASC"#,
        )
        .bind(&sandbox.id)
        .bind(cut)
        .fetch_all(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?
    } else {
        sqlx::query(
            r#"SELECT input, output FROM sandbox_tasks WHERE sandbox_id = ? ORDER BY created_at ASC"#,
        )
        .bind(&sandbox.id)
        .fetch_all(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?
    };

    // Build a transcript from input.content text items and assistant output tool results
    let mut transcript = String::new();
    let mut added_chars: usize = 0;
    let max_chars: usize = 50_000; // guard to avoid sending extremely large bodies
    for row in rows {
        let input: serde_json::Value = row.try_get("input").unwrap_or(serde_json::json!({}));
        let output: serde_json::Value = row.try_get("output").unwrap_or(serde_json::json!({}));
        // Legacy: single text field
        if let Some(user_text) = input.get("text").and_then(|v| v.as_str()) {
            if !user_text.trim().is_empty() {
                let line = format!("User: {}\n", user_text.trim());
                if added_chars + line.len() > max_chars {
                    break;
                }
                transcript.push_str(&line);
                added_chars += line.len();
            }
        }
        // Structured: input.content text items
        if let Some(arr) = input.get("content").and_then(|v| v.as_array()) {
            for it in arr {
                let t = it.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if t.eq_ignore_ascii_case("text") {
                    if let Some(s) = it.get("content").and_then(|v| v.as_str()) {
                        if !s.trim().is_empty() {
                            let line = format!("User: {}\n", s.trim());
                            if added_chars + line.len() > max_chars {
                                break;
                            }
                            transcript.push_str(&line);
                            added_chars += line.len();
                        }
                    }
                }
            }
        }
        // Legacy: assistant text
        if let Some(assistant_text) = output.get("text").and_then(|v| v.as_str()) {
            if !assistant_text.trim().is_empty() {
                let line = format!("Assistant: {}\n", assistant_text.trim());
                if added_chars + line.len() > max_chars {
                    break;
                }
                transcript.push_str(&line);
                added_chars += line.len();
            }
        }
        // Structured: find final output tool_result (tool == 'output') and render its items
        if let Some(segs) = output.get("items").and_then(|v| v.as_array()) {
            for seg in segs {
                let seg_type = seg.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if seg_type == "tool_result" {
                    let tool = seg.get("tool").and_then(|v| v.as_str()).unwrap_or("");
                    if tool == "output"
                        || tool == "output_markdown"
                        || tool == "ouput_json"
                        || tool == "output_json"
                    {
                        if let Some(out) = seg.get("output") {
                            if let Some(items) = out.get("items").and_then(|v| v.as_array()) {
                                for item in items {
                                    let typ = item
                                        .get("type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_ascii_lowercase();
                                    let mut content_str = String::new();
                                    match typ.as_str() {
                                        "markdown" => {
                                            if let Some(s) =
                                                item.get("content").and_then(|v| v.as_str())
                                            {
                                                content_str = s.trim().to_string();
                                            }
                                        }
                                        "json" => {
                                            let val = item
                                                .get("content")
                                                .cloned()
                                                .unwrap_or(serde_json::Value::Null);
                                            content_str = val.to_string();
                                        }
                                        "url" => {
                                            if let Some(s) =
                                                item.get("content").and_then(|v| v.as_str())
                                            {
                                                content_str = s.trim().to_string();
                                            }
                                        }
                                        _ => {}
                                    }
                                    if !content_str.is_empty() {
                                        let line = format!("Assistant: {}\n", content_str);
                                        if added_chars + line.len() > max_chars {
                                            break;
                                        }
                                        transcript.push_str(&line);
                                        added_chars += line.len();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if added_chars >= max_chars {
            break;
        }
    }

    // If nothing to summarize, create a minimal marker
    let summary_text = if transcript.trim().is_empty() {
        "(No prior conversation to compact.)".to_string()
    } else {
        // Call the configured inference service to summarize the transcript
        let base_url = std::env::var("TSBX_INFERENCE_URL")
            .unwrap_or_else(|_| "https://api.positron.ai/v1".to_string());
        let model = std::env::var("TSBX_INFERENCE_MODEL")
            .or_else(|_| std::env::var("TSBX_DEFAULT_MODEL"))
            .unwrap_or_else(|_| "llama-3.1-8b-instruct-good-tp2".to_string());
        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
        let system_prompt = "You are a helpful assistant that compresses conversation history into a concise context for future messages.\n- Keep key goals, decisions, constraints, URLs, files, and paths.\n- Remove chit‑chat and redundant steps.\n- Prefer bullet points.\n- Target 150–250 words.";
        let user_content = format!("Please summarize the following conversation so it can be used as compact context for future turns.\n\n{}", transcript);
        let body = serde_json::json!({
            "model": model,
            "stream": false,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_content }
            ]
        });

        let client = reqwest::Client::new();
        let mut req = client.post(&url).json(&body);
        if let Ok(api_key) = std::env::var("TSBX_INFERENCE_API_KEY") {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }
        let resp = req
            .send()
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Inference request failed: {}", e)))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(ApiError::Internal(anyhow::anyhow!(
                "Inference error ({}): {}",
                status,
                text
            )));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Inference parse error: {}", e)))?;
        v.get("choices")
            .and_then(|c| c.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("(summary unavailable)")
            .to_string()
    };

    // Set the cutoff now
    crate::shared::models::Sandbox::clear_context_cutoff(&state.db, &sandbox.id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to set context cutoff: {}", e)))?;

    // Record a history marker indicating context was compacted, include summary as a compact_summary item
    if let Ok(created) = task_model::SandboxTask::create(
        &state.db,
        &sandbox.id,
        username,
        task_model::CreateTaskRequest {
            input: serde_json::json!({ "content": [] }),
            background: None,
            timeout_seconds: None,
        },
    )
    .await
    {
        let cutoff_now = Utc::now().to_rfc3339();
        let _ = task_model::SandboxTask::update_by_id(
            &state.db,
            &created.id,
            task_model::UpdateTaskRequest {
                status: Some("completed".to_string()),
                input: None,
                output: Some(serde_json::json!({
                    "text": "",
                    "items": [
                        { "type": "context_compacted", "cutoff_at": cutoff_now },
                        { "type": "compact_summary", "content": summary_text }
                    ]
                })),
                timeout_seconds: None,
            },
        )
        .await;
    }

    // Return fresh measurement (post-compaction, reset to zero)
    let limit = soft_limit_tokens();
    let now = Utc::now().to_rfc3339();
    let resp = SandboxContextUsageResponse {
        sandbox: sandbox.id,
        soft_limit_tokens: limit,
        used_tokens_estimated: 0,
        used_percent: 0.0,
        basis: "inference_last_context_length".to_string(),
        cutoff_at: Some(now.clone()),
        measured_at: now,
        total_messages_considered: 0,
    };
    Ok(Json(resp))
}

pub async fn update_sandbox_context_usage(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<UpdateContextUsageRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    let is_admin = is_admin_principal(&auth, &state).await;
    let sandbox = find_sandbox_by_id(&state, &id, username, is_admin).await?;
    check_not_deleted(&sandbox)?;

    let tokens = req.tokens.max(0);
    Sandbox::update_last_context_length(&state.db, &sandbox.id, tokens)
        .await
        .map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("Failed to update context length: {}", e))
        })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "last_context_length": tokens
    })))
}

pub async fn create_sandbox(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateSandboxRequest>,
) -> ApiResult<Json<SandboxResponse>> {
    tracing::info!(
        "Creating sandbox with env: {} keys, instructions: {}, setup: {}, prompt: {}",
        req.env.len(),
        req.instructions.is_some(),
        req.setup.is_some(),
        req.prompt.is_some()
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
        "prompt": req.prompt,
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

    // Check if sandbox is deleted
    check_not_deleted(&sandbox)?;

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
    check_not_deleted(&sandbox)?;

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

pub async fn delete_sandbox(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<()> {
    // Check permission for deleting sandboxes (admin only). Owners can delete without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::SANDBOX_DELETE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to delete sandbox".to_string())
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

    // Delete sandbox: schedule container stop and mark as deleted
    // Add request to queue for sandbox manager to stop container and set state to deleted
    sqlx::query(
        r#"
        INSERT INTO sandbox_requests (sandbox_id, request_type, created_by, payload, status)
        VALUES (?, 'delete_sandbox', ?, '{}', 'pending')
        "#,
    )
    .bind(&sandbox.id)
    .bind(username)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create delete request: {}", e)))?;

    tracing::info!("Created delete request for sandbox {}", sandbox.id);

    // The controller will stop the container and set state to 'deleted'
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
        r#"SELECT created_at, output FROM sandbox_tasks WHERE sandbox_id = ? ORDER BY created_at ASC"#
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
        if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
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
                } else if t == "deleted" {
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

    // Include current sandbox up to now when sandbox is not deleted
    if sandbox.state.to_lowercase() != crate::shared::models::constants::SANDBOX_STATE_DELETED {
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
    check_not_deleted(&sandbox)?;

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
    check_not_deleted(&sandbox)?;

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
