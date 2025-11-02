use axum::http::StatusCode;
use axum::response::Response;
use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::de::{self, Deserializer, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use sqlx::query;
use sqlx::Row;
use std::sync::Arc;

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::api::rest::rbac_enforcement::{check_api_permission, permissions};
use crate::shared::models::{
    AppState, BranchSessionRequest, CreateSessionRequest, PublishSessionRequest,
    RestoreSessionRequest, Session, UpdateSessionRequest, UpdateSessionStateRequest,
};
use crate::shared::rbac::PermissionContext;
// Use fully-qualified names for response records to avoid name conflict with local SessionResponse
use crate::shared::models::response as resp_model;

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

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub name: String, // Primary key - no more id field
    pub created_by: String,
    pub state: String,
    pub description: Option<String>,
    pub parent_session_name: Option<String>, // Changed from parent_session_id
    pub created_at: String,
    pub last_activity_at: Option<String>,
    pub metadata: serde_json::Value,
    pub tags: Vec<String>,
    pub is_published: bool,
    pub published_at: Option<String>,
    pub published_by: Option<String>,
    pub publish_permissions: serde_json::Value,
    pub idle_timeout_seconds: i32,
    pub busy_timeout_seconds: i32,
    pub idle_from: Option<String>,
    pub busy_from: Option<String>,
    pub context_cutoff_at: Option<String>,
    pub last_context_length: i64,
    // Removed: id, container_id, persistent_volume_id
}

#[derive(Debug, Deserialize, Default)]
pub struct ListSessionsQuery {
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
pub struct PaginatedSessions {
    pub items: Vec<SessionResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    pub page: i64,
    pub pages: i64,
}

impl SessionResponse {
    async fn from_session(session: Session, _pool: &sqlx::MySqlPool) -> Result<Self, ApiError> {
        // Convert tags from JSON value to Vec<String>
        let tags: Vec<String> = match session.tags {
            serde_json::Value::Array(arr) => arr
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            _ => Vec::new(),
        };
        Ok(Self {
            name: session.name,
            created_by: session.created_by,
            state: session.state,
            description: session.description,
            parent_session_name: session.parent_session_name,
            created_at: session.created_at.to_rfc3339(),
            last_activity_at: session.last_activity_at.map(|dt| dt.to_rfc3339()),
            metadata: session.metadata,
            tags,
            is_published: session.is_published,
            published_at: session.published_at.map(|dt| dt.to_rfc3339()),
            published_by: session.published_by,
            publish_permissions: session.publish_permissions,
            idle_timeout_seconds: session.idle_timeout_seconds,
            busy_timeout_seconds: session.busy_timeout_seconds,
            idle_from: session.idle_from.map(|dt| dt.to_rfc3339()),
            busy_from: session.busy_from.map(|dt| dt.to_rfc3339()),
            context_cutoff_at: session.context_cutoff_at.map(|dt| dt.to_rfc3339()),
            last_context_length: session.last_context_length,
        })
    }
}

// Helper function to find session by name
async fn find_session_by_name(
    state: &AppState,
    name: &str,
    created_by: &str,
    is_admin: bool,
) -> Result<Session, ApiError> {
    // Try to find by name directly (names are globally unique)
    if let Some(session) = Session::find_by_name(&state.db, name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch session: {}", e)))?
    {
        // Admins can access any session, regular users only their own or published sessions
        if is_admin || session.created_by == created_by || session.is_published {
            return Ok(session);
        } else {
            return Err(ApiError::Forbidden(
                "Access denied to this session".to_string(),
            ));
        }
    }

    Err(ApiError::NotFound("Session not found".to_string()))
}

// -------- Session Files (read-only) --------

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

fn map_file_task_error(err: &str) -> ApiError {
    let lower = err.to_ascii_lowercase();
    if lower.contains("too large") {
        ApiError::PayloadTooLarge(err.to_string())
    } else if lower.contains("no such file") || lower.contains("not found") {
        ApiError::NotFound("File or directory not found".to_string())
    } else if lower.contains("is a directory") {
        ApiError::BadRequest("Path is a directory".to_string())
    } else if lower.contains("invalid path") {
        ApiError::BadRequest("Invalid path".to_string())
    } else if lower.contains("sleep")
        || lower.contains("not running")
        || lower.contains("container does not exist")
    {
        ApiError::Conflict("Session is sleeping".to_string())
    } else if lower.contains("forbidden") || lower.contains("outside") {
        ApiError::Forbidden(err.to_string())
    } else {
        ApiError::Internal(anyhow::anyhow!(err.to_string()))
    }
}

pub async fn read_session_file(
    State(state): State<Arc<AppState>>,
    Path((name, path)): Path<(String, String)>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Response, ApiError> {
    // Admins require explicit permission; owners can access their own sessions
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SESSION_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to read files".to_string())
            })?;
    }

    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    let _session = find_session_by_name(&state, &name, username, is_admin).await?;

    if !is_safe_relative_path(&path) {
        return Err(ApiError::BadRequest("Invalid path".to_string()));
    }

    // Create file_read task
    let task_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "path": path,
    });
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    sqlx::query(
        r#"INSERT INTO session_tasks (id, session_name, task_type, created_by, payload, status)
            VALUES (?, ?, 'file_read', ?, ?, 'pending')"#,
    )
    .bind(&task_id)
    .bind(&name)
    .bind(username)
    .bind(&payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create file_read task: {}", e)))?;

    // Poll for completion up to 15s
    let start = std::time::Instant::now();
    loop {
        let row = sqlx::query(r#"SELECT status, payload, error FROM session_tasks WHERE id = ?"#)
            .bind(&task_id)
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
                let bytes = base64::decode(content_b64).unwrap_or_default();
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
                return Err(map_file_task_error(&err));
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

pub async fn get_session_file_metadata(
    State(state): State<Arc<AppState>>,
    Path((name, path)): Path<(String, String)>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SESSION_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to read files".to_string())
            })?;
    }
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    let _session = find_session_by_name(&state, &name, username, is_admin).await?;
    if !is_safe_relative_path(&path) {
        return Err(ApiError::BadRequest("Invalid path".to_string()));
    }
    let task_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({ "path": path });
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    sqlx::query(
        r#"INSERT INTO session_tasks (id, session_name, task_type, created_by, payload, status)
            VALUES (?, ?, 'file_metadata', ?, ?, 'pending')"#,
    )
    .bind(&task_id)
    .bind(&name)
    .bind(username)
    .bind(&payload)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        ApiError::Internal(anyhow::anyhow!(
            "Failed to create file_metadata task: {}",
            e
        ))
    })?;

    let start = std::time::Instant::now();
    loop {
        let row = sqlx::query(r#"SELECT status, payload, error FROM session_tasks WHERE id = ?"#)
            .bind(&task_id)
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
                return Err(map_file_task_error(&err));
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

pub async fn list_session_files(
    State(state): State<Arc<AppState>>,
    Path((name, path)): Path<(String, String)>,
    Query(paging): Query<ListFilesQuery>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SESSION_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to list files".to_string())
            })?;
    }
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    let _session = find_session_by_name(&state, &name, username, is_admin).await?;
    if !is_safe_relative_path(&path) && !path.is_empty() {
        return Err(ApiError::BadRequest("Invalid path".to_string()));
    }
    let task_id = uuid::Uuid::new_v4().to_string();
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
        r#"INSERT INTO session_tasks (id, session_name, task_type, created_by, payload, status)
            VALUES (?, ?, 'file_list', ?, ?, 'pending')"#,
    )
    .bind(&task_id)
    .bind(&name)
    .bind(username)
    .bind(&payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create file_list task: {}", e)))?;

    let start = std::time::Instant::now();
    loop {
        let row = sqlx::query(r#"SELECT status, payload, error FROM session_tasks WHERE id = ?"#)
            .bind(&task_id)
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
                return Err(map_file_task_error(&err));
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
pub async fn list_session_files_root(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(paging): Query<ListFilesQuery>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SESSION_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to list files".to_string())
            })?;
    }
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    let _session = find_session_by_name(&state, &name, username, is_admin).await?;
    let task_id = uuid::Uuid::new_v4().to_string();
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
        r#"INSERT INTO session_tasks (id, session_name, task_type, created_by, payload, status)
            VALUES (?, ?, 'file_list', ?, ?, 'pending')"#,
    )
    .bind(&task_id)
    .bind(&name)
    .bind(username)
    .bind(&payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create file_list task: {}", e)))?;

    let start = std::time::Instant::now();
    loop {
        let row = sqlx::query(r#"SELECT status, payload, error FROM session_tasks WHERE id = ?"#)
            .bind(&task_id)
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

pub async fn delete_session_file(
    State(state): State<Arc<AppState>>,
    Path((name, path)): Path<(String, String)>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SESSION_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to delete files".to_string())
            })?;
    }
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    let _session = find_session_by_name(&state, &name, username, is_admin).await?;
    if !is_safe_relative_path(&path) {
        return Err(ApiError::BadRequest("Invalid path".to_string()));
    }
    let task_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({ "path": path });
    sqlx::query(
        r#"INSERT INTO session_tasks (id, session_name, task_type, created_by, payload, status)
            VALUES (?, ?, 'file_delete', ?, ?, 'pending')"#,
    )
    .bind(&task_id)
    .bind(&name)
    .bind(username)
    .bind(&payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create file_delete task: {}", e)))?;

    let start = std::time::Instant::now();
    loop {
        let row = sqlx::query(r#"SELECT status, payload, error FROM session_tasks WHERE id = ?"#)
            .bind(&task_id)
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
                return Err(map_file_task_error(&err));
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

pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListSessionsQuery>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<PaginatedSessions>> {
    // Admins require explicit permission; non-admins can list only their own sessions
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SESSION_LIST)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to list sessions".to_string())
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
            where_sql.push_str(" AND (LOWER(name) LIKE ? OR (description IS NOT NULL AND LOWER(description) LIKE ?)) ");
            let pat = format!("%{}%", q);
            binds.push(serde_json::Value::String(pat.clone()));
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
    let count_sql = format!("SELECT COUNT(*) as cnt FROM sessions {}", where_sql);
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
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to count sessions: {}", e)))?;

    // Fetch page
    let select_sql = format!(
        r#"
        SELECT name, created_by, state, description, parent_session_name,
               created_at, last_activity_at, metadata, tags,
               is_published, published_at, published_by, publish_permissions,
               idle_timeout_seconds, busy_timeout_seconds, idle_from, busy_from, context_cutoff_at,
               last_context_length
        FROM sessions
        {} 
        ORDER BY created_at DESC
        LIMIT ? OFFSET ?
        "#,
        where_sql
    );
    let mut q_items = sqlx::query_as::<_, Session>(&select_sql);
    for b in binds.iter() {
        if let Some(s) = b.as_str() {
            q_items = q_items.bind(s);
        }
    }
    q_items = q_items.bind(limit).bind(offset);

    let sessions: Vec<Session> = q_items
        .fetch_all(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to list sessions: {}", e)))?;

    let mut items: Vec<SessionResponse> = Vec::with_capacity(sessions.len());
    for session in sessions {
        items.push(SessionResponse::from_session(session, &state.db).await?);
    }
    let page = if limit > 0 { (offset / limit) + 1 } else { 1 };
    let pages = if limit > 0 {
        ((total + limit - 1) / limit).max(1)
    } else {
        1
    };

    Ok(Json(PaginatedSessions {
        items,
        total,
        limit,
        offset,
        page,
        pages,
    }))
}

pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SessionResponse>> {
    // Admins require explicit permission; non-admins can access only their own session
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SESSION_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to get session".to_string())
            })?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by name (admin can access any session)
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    Ok(Json(
        SessionResponse::from_session(session, &state.db).await?,
    ))
}

// Cancel the latest in-progress response for an session and set session to idle
pub async fn cancel_active_response(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Admins must have SESSION_UPDATE; owners can cancel their own without RBAC grant
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions".to_string()))?;
    }

    // Resolve principal identity
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Confirm access to the session; enforce ownership for non-admins
    let session = find_session_by_name(&state, &name, username, is_admin).await?;
    if !is_admin && session.created_by != *username {
        return Err(ApiError::Forbidden(
            "You can only cancel your own sessions".to_string(),
        ));
    }

    // Find latest in-progress response (processing or pending)
    let row: Option<(String, serde_json::Value)> = sqlx::query_as(
        r#"SELECT id, output FROM session_responses WHERE session_name = ? AND status IN ('processing','pending') ORDER BY created_at DESC LIMIT 1"#
    )
    .bind(&session.name)
    .fetch_optional(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;

    let mut cancelled = false;
    if let Some((resp_id, output)) = row {
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
            r#"UPDATE session_responses SET status = 'cancelled', output = ?, updated_at = NOW() WHERE id = ?"#
        )
        .bind(&new_output)
        .bind(&resp_id)
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;
        cancelled = true;
    }

    // If no response row, try to cancel a queued create_response task (pre-insert race)
    if !cancelled {
        if let Some((task_id, created_by, payload)) = sqlx::query_as::<_, (String, String, serde_json::Value)>(
            r#"SELECT id, created_by, payload FROM session_tasks WHERE session_name = ? AND task_type = 'create_response' AND status IN ('pending','processing') ORDER BY created_at DESC LIMIT 1"#
        )
        .bind(&session.name)
        .fetch_optional(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))? {
            let resp_id = payload.get("response_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if !resp_id.is_empty() {
                let input = payload.get("input").cloned().unwrap_or_else(|| serde_json::json!({"text":""}));
                let now = chrono::Utc::now();
                let cancelled_item = serde_json::json!({"type":"cancelled","reason":"user_cancel","at": now.to_rfc3339()});
                let output = serde_json::json!({"text":"","items":[cancelled_item]});
                // Insert cancelled response row (idempotent behavior if it already exists)
                let _ = sqlx::query(
                    r#"INSERT INTO session_responses (id, session_name, created_by, status, input, output, created_at, updated_at)
                        VALUES (?, ?, ?, 'cancelled', ?, ?, NOW(), NOW())
                        ON DUPLICATE KEY UPDATE status='cancelled', output=VALUES(output), updated_at=NOW()"#
                )
                .bind(&resp_id)
                .bind(&session.name)
                .bind(&created_by)
                .bind(&input)
                .bind(&output)
                .execute(&*state.db)
                .await;
                // Mark task completed to prevent later insertion
                let _ = sqlx::query(r#"UPDATE session_tasks SET status='completed', updated_at=NOW(), completed_at=NOW(), error='cancelled' WHERE id = ?"#)
                    .bind(&task_id)
                    .execute(&*state.db)
                    .await;
                cancelled = true;
            }
        }
    }

    // Set session to idle
    sqlx::query(r#"UPDATE sessions SET state = 'idle', last_activity_at = NOW(), idle_from = NOW(), busy_from = NULL WHERE name = ?"#)
        .bind(&session.name)
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update session: {}", e)))?;

    Ok(Json(
        serde_json::json!({"status":"ok", "session": session.name, "cancelled": cancelled}),
    ))
}

#[derive(Debug, Serialize)]
pub struct SessionContextUsageResponse {
    pub session: String,
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
    session_name: &str,
    cutoff: Option<DateTime<Utc>>,
) -> Result<(i64, u32), ApiError> {
    // No ordering needed for estimation; avoid sort pressure
    let rows = if let Some(cut) = cutoff {
        sqlx::query(
            r#"SELECT status, input, output, created_at FROM session_responses WHERE session_name = ? AND created_at >= ?"#,
        )
            .bind(session_name)
            .bind(cut)
            .fetch_all(pool)
            .await
    } else {
        sqlx::query(
            r#"SELECT status, input, output, created_at FROM session_responses WHERE session_name = ?"#,
        )
            .bind(session_name)
            .fetch_all(pool)
            .await
    }
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;

    let mut total_chars: i64 = 0;
    let mut msg_count: u32 = 0;
    const TOOL_RESULT_PREVIEW_MAX: usize = 100;

    // Determine the single latest 'processing' response by created_at
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
            // Only include tools for the single most recent processing response
            let include_tools = latest_proc
                .and_then(|lp| {
                    row.try_get::<DateTime<Utc>, _>("created_at")
                        .ok()
                        .map(|c| c == lp)
                })
                .unwrap_or(false);
            if include_tools {
                // For the response currently being worked on, include tool_call and tool_result outputs
                if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
                    for it in items {
                        if it.get("type").and_then(|v| v.as_str()) == Some("tool_call") {
                            let tool = it.get("tool").and_then(|v| v.as_str()).unwrap_or("");
                            let args = it.get("args").cloned().unwrap_or(serde_json::Value::Null);
                            let s = serde_json::json!({"tool_call": {"tool": tool, "args": args}})
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
                        let args = it.get("args").cloned().unwrap_or(serde_json::Value::Null);
                        let s = serde_json::json!({"tool_call": {"tool": tool, "args": args}})
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
            // Completed responses: include only the synthesized assistant message built from output_content
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

pub async fn get_session_context(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SessionContextUsageResponse>> {
    // Reuse GET permission
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SESSION_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to get session context".to_string())
            })?;
    }

    // Get username
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    let session = find_session_by_name(&state, &name, username, is_admin).await?;
    let used = session.last_context_length;
    let limit = soft_limit_tokens();
    let used_percent = if limit > 0 {
        (used as f64 * 100.0) / (limit as f64)
    } else {
        0.0
    };

    let resp = SessionContextUsageResponse {
        session: session.name,
        soft_limit_tokens: limit,
        used_tokens_estimated: used,
        used_percent,
        basis: "ollama_last_context_length".to_string(),
        cutoff_at: session.context_cutoff_at.map(|dt| dt.to_rfc3339()),
        measured_at: Utc::now().to_rfc3339(),
        total_messages_considered: 0,
    };

    Ok(Json(resp))
}

pub async fn clear_session_context(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SessionContextUsageResponse>> {
    // Require update permission
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
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

    // Confirm access to the session
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Set the cutoff now
    crate::shared::models::Session::clear_context_cutoff(&state.db, &session.name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to set context cutoff: {}", e)))?;

    // Record a history marker indicating context was cleared
    if let Ok(created) = resp_model::SessionResponse::create(
        &state.db,
        &session.name,
        username,
        resp_model::CreateResponseRequest {
            input: serde_json::json!({ "content": [] }),
            background: None,
        },
    )
    .await
    {
        let cutoff_now = Utc::now().to_rfc3339();
        let _ = resp_model::SessionResponse::update_by_id(
            &state.db,
            &created.id,
            resp_model::UpdateResponseRequest {
                status: Some("completed".to_string()),
                input: None,
                output: Some(serde_json::json!({
                    "text": "",
                    "items": [ { "type": "context_cleared", "cutoff_at": cutoff_now } ]
                })),
            },
        )
        .await;
    }

    // Return fresh measurement (reset to zero)
    let limit = soft_limit_tokens();
    let now = Utc::now().to_rfc3339();
    let resp = SessionContextUsageResponse {
        session: session.name,
        soft_limit_tokens: limit,
        used_tokens_estimated: 0,
        used_percent: 0.0,
        basis: "ollama_last_context_length".to_string(),
        cutoff_at: Some(now.clone()),
        measured_at: now,
        total_messages_considered: 0,
    };
    Ok(Json(resp))
}

// Compact context: summarize recent conversation and set a new cutoff.
pub async fn compact_session_context(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SessionContextUsageResponse>> {
    // Require update permission
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
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

    // Confirm access to the session
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Load conversation history since the current cutoff (if any)
    let cutoff = session.context_cutoff_at;
    let rows = if let Some(cut) = cutoff {
        sqlx::query(
            r#"SELECT input, output FROM session_responses WHERE session_name = ? AND created_at >= ? ORDER BY created_at ASC"#,
        )
        .bind(&session.name)
        .bind(cut)
        .fetch_all(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?
    } else {
        sqlx::query(
            r#"SELECT input, output FROM session_responses WHERE session_name = ? ORDER BY created_at ASC"#,
        )
        .bind(&session.name)
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
        // Call Ollama to summarize the transcript
        // Prefer the same variable name used by controller/session; default to Docker network hostname
        let base_url =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://ollama:11434".to_string());
        let model = std::env::var("TSBX_DEFAULT_MODEL").unwrap_or_else(|_| "gpt-oss:20b".to_string());
        let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
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
        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Ollama request failed: {}", e)))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(ApiError::Internal(anyhow::anyhow!(
                "Ollama error ({}): {}",
                status,
                text
            )));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Ollama parse error: {}", e)))?;
        v.get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("(summary unavailable)")
            .to_string()
    };

    // Set the cutoff now
    crate::shared::models::Session::clear_context_cutoff(&state.db, &session.name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to set context cutoff: {}", e)))?;

    // Record a history marker indicating context was compacted, include summary as a compact_summary item
    if let Ok(created) = resp_model::SessionResponse::create(
        &state.db,
        &session.name,
        username,
        resp_model::CreateResponseRequest {
            input: serde_json::json!({ "content": [] }),
            background: None,
        },
    )
    .await
    {
        let cutoff_now = Utc::now().to_rfc3339();
        let _ = resp_model::SessionResponse::update_by_id(
            &state.db,
            &created.id,
            resp_model::UpdateResponseRequest {
                status: Some("completed".to_string()),
                input: None,
                output: Some(serde_json::json!({
                    "text": "",
                    "items": [
                        { "type": "context_compacted", "cutoff_at": cutoff_now },
                        { "type": "compact_summary", "content": summary_text }
                    ]
                })),
            },
        )
        .await;
    }

    // Return fresh measurement (post-compaction, reset to zero)
    let limit = soft_limit_tokens();
    let now = Utc::now().to_rfc3339();
    let resp = SessionContextUsageResponse {
        session: session.name,
        soft_limit_tokens: limit,
        used_tokens_estimated: 0,
        used_percent: 0.0,
        basis: "ollama_last_context_length".to_string(),
        cutoff_at: Some(now.clone()),
        measured_at: now,
        total_messages_considered: 0,
    };
    Ok(Json(resp))
}

pub async fn update_session_context_usage(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<UpdateContextUsageRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    let is_admin = is_admin_principal(&auth, &state).await;
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    let tokens = req.tokens.max(0);
    Session::update_last_context_length(&state.db, &session.name, tokens)
        .await
        .map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("Failed to update context length: {}", e))
        })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "last_context_length": tokens
    })))
}

pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    tracing::info!(
        "Creating session with env: {} keys, instructions: {}, setup: {}, prompt: {}",
        req.env.len(),
        req.instructions.is_some(),
        req.setup.is_some(),
        req.prompt.is_some()
    );

    // Admins require explicit permission; non-admins can create their own sessions
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::SESSION_CREATE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to create session".to_string())
            })?;
    }

    // Get the principal name
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    let session = Session::create(&state.db, req.clone(), created_by)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create session: {:?}", e);

            // Check for unique constraint violation on session name
            if let sqlx::Error::Database(db_err) = &e {
                tracing::error!(
                    "Database error - code: {:?}, message: '{}'",
                    db_err.code(),
                    db_err.message()
                );
                if let Some(code) = db_err.code() {
                    // Simplify the condition to catch the specific error
                    if code == "23000" || code == "1062" {
                        let name_display = &req.name;
                        tracing::info!(
                            "Detected database constraint violation for session {}",
                            name_display
                        );
                        if db_err.message().contains("sessions.PRIMARY")
                            || db_err.message().contains("unique_session_name")
                            || db_err.message().contains("Duplicate entry")
                        {
                            tracing::info!("Confirmed duplicate session name constraint violation");
                            return ApiError::Conflict(format!(
                                "Session name '{}' already exists. Choose a different name.",
                                name_display
                            ));
                        }
                    }
                }
            }

            ApiError::Internal(anyhow::anyhow!("Failed to create session: {}", e))
        })?;

    // Add task to queue for session manager to create container with session parameters
    let payload = serde_json::json!({
        "env": req.env,
        "instructions": req.instructions,
        "setup": req.setup,
        "prompt": req.prompt,
        "principal": created_by,
        "principal_type": match &auth.principal {
            crate::shared::rbac::AuthPrincipal::Subject(_) => "User",
            crate::shared::rbac::AuthPrincipal::Operator(_) => "Admin",
        },
        "user_token": auth.token
    });

    sqlx::query(
        r#"
        INSERT INTO session_tasks (session_name, task_type, created_by, payload, status)
        VALUES (?, 'create_session', ?, ?, 'pending')
        "#,
    )
    .bind(&session.name)
    .bind(created_by)
    .bind(payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create session task: {}", e)))?;

    tracing::info!("Created session task for session {}", session.name);

    Ok(Json(
        SessionResponse::from_session(session, &state.db).await?,
    ))
}

pub async fn branch_session(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<BranchSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    // Admins require explicit permission; non-admins can branch according to publish/ownership checks
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::SESSION_CREATE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to branch session".to_string())
            })?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find parent session by ID or name (admin can branch any session, users can branch published sessions)
    let is_admin = is_admin_principal(&auth, &state).await;
    let parent = find_session_by_name(&state, &name, username, true).await?; // Allow finding any session for branch (permission check below)

    // Check branch permissions for non-owners
    if parent.created_by != *username && !is_admin {
        // Non-owner, non-admin can only branch if session is published
        if !parent.is_published {
            return Err(ApiError::Forbidden(
                "You can only branch your own sessions or published sessions".to_string(),
            ));
        }

        // Check published branch permissions
        let publish_perms = parent.publish_permissions.as_object().ok_or_else(|| {
            ApiError::Internal(anyhow::anyhow!("Invalid publish permissions format"))
        })?;

        // Data folder removed in v0.4.0 - no data permission check needed
        if req.code
            && !publish_perms
                .get("code")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        {
            return Err(ApiError::Forbidden(
                "Code branch not permitted for this published session".to_string(),
            ));
        }
        if req.env
            && !publish_perms
                .get("env")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        {
            return Err(ApiError::Forbidden(
                "Environment branch not permitted for this published session".to_string(),
            ));
        }
        // Content is always allowed - no permission check needed
    }

    // Get the principal name for task creation (brancher becomes owner)
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Store the branch options before moving req into Session::branch
    let copy_code = req.code;
    let copy_env = req.env;
    // Content is always copied
    let copy_content = true;
    let initial_prompt = req.prompt.clone();

    let session = Session::branch(&state.db, &parent.name, req.clone(), created_by)
        .await
        .map_err(|e| {
            // Provide a clearer error on duplicate name conflicts
            if let sqlx::Error::Database(db_err) = &e {
                if let Some(code) = db_err.code() {
                    // MySQL duplicate/constraint codes: 23000 (SQLSTATE), 1062 (ER_DUP_ENTRY)
                    if code == "23000" || code == "1062" {
                        if db_err.message().contains("sessions.PRIMARY")
                            || db_err.message().contains("unique_session_name")
                            || db_err.message().contains("Duplicate entry")
                        {
                            return ApiError::Conflict(format!(
                                "Session name '{}' already exists. Choose a different name.",
                                req.name
                            ));
                        }
                    }
                }
            }
            ApiError::Internal(anyhow::anyhow!("Failed to branch session: {}", e))
        })?;

    // Add task to queue for session manager to create container with branch options
    let task_payload = serde_json::json!({
        "branch": true,
        "parent_session_name": parent.name,
        "copy_code": copy_code,
        "copy_env": copy_env,
        "copy_content": copy_content,
        "prompt": initial_prompt,
        "principal": created_by,
        "principal_type": match &auth.principal {
            crate::shared::rbac::AuthPrincipal::Subject(_) => "User",
            crate::shared::rbac::AuthPrincipal::Operator(_) => "Admin",
        }
    });

    sqlx::query(
        r#"
        INSERT INTO session_tasks (session_name, task_type, created_by, payload, status)
        VALUES (?, 'create_session', ?, ?, 'pending')
        "#,
    )
    .bind(&session.name)
    .bind(created_by)
    .bind(task_payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create session task: {}", e)))?;

    tracing::info!("Created session task for branched session {}", session.name);

    Ok(Json(
        SessionResponse::from_session(session, &state.db).await?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct SleepSessionRequest {
    #[serde(default)]
    pub delay_seconds: Option<u64>,
    #[serde(default)]
    pub note: Option<String>,
}

pub async fn sleep_session(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    maybe_req: Option<Json<SleepSessionRequest>>,
) -> ApiResult<Json<SessionResponse>> {
    tracing::info!("Sleep request received for session: {}", name);
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by ID or name (admin can sleep any session)
    let is_admin = is_admin_principal(&auth, &state).await;
    let session = find_session_by_name(&state, &name, created_by, is_admin).await?;

    tracing::info!("Found session in state: {}", session.state);

    // Check permission for updating sessions (admin only). Owners can sleep without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to sleep session".to_string())
            })?;
    }

    // Allow sleeping own sessions or admin can sleep any session
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    let is_admin = is_admin_principal(&auth, &state).await;
    if !is_admin && session.created_by != *username {
        return Err(ApiError::Forbidden(
            "Can only sleep your own sessions".to_string(),
        ));
    }
    tracing::info!("Permission check passed");

    // Check current state - cannot sleep if already sleeping

    if session.state == crate::shared::models::constants::SESSION_STATE_SLEPT {
        return Err(ApiError::BadRequest(
            "Session is already sleeping".to_string(),
        ));
    }

    // Determine delay (min 5 seconds)
    // Try to parse JSON body; if absent or invalid, default to 5
    let mut delay_seconds = maybe_req
        .as_ref()
        .and_then(|r| r.delay_seconds)
        .unwrap_or(5);
    if delay_seconds < 5 {
        delay_seconds = 5;
    }
    // Add task to destroy the container but keep volume after delay
    let note = maybe_req
        .as_ref()
        .and_then(|r| r.note.clone())
        .and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        });
    let payload = if let Some(ref n) = note {
        serde_json::json!({ "delay_seconds": delay_seconds, "note": n })
    } else {
        serde_json::json!({ "delay_seconds": delay_seconds })
    };
    sqlx::query(
        r#"
        INSERT INTO session_tasks (session_name, task_type, created_by, payload, status)
        VALUES (?, 'sleep_session', ?, ?, 'pending')
        "#,
    )
    .bind(&session.name)
    .bind(&created_by)
    .bind(payload)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create suspend task: {:?}", e);
        ApiError::Internal(anyhow::anyhow!("Failed to create suspend task: {}", e))
    })?;

    tracing::info!("Created suspend task for session {}", name);

    // Do not insert a pre-sleep marker; the controller will add a single 'slept' marker when sleep completes

    // Fetch session (state remains as-is until controller executes sleep)
    let updated_session = Session::find_by_name(&state.db, &name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch updated session: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;

    Ok(Json(
        SessionResponse::from_session(updated_session, &state.db).await?,
    ))
}

pub async fn wake_session(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<RestoreSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    // Check permission for updating sessions (admin only). Owners can wake without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to wake session".to_string())
            })?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by ID or name (admin can find any session, but restore has ownership restrictions)
    let is_admin = is_admin_principal(&auth, &state).await;
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Ownership: owners can wake their own sessions; admins (with SESSION_UPDATE) may wake any session
    if session.created_by != *username && !is_admin {
        return Err(ApiError::Forbidden(
            "You can only wake your own sessions.".to_string(),
        ));
    }

    // Check current state - can only wake if sleeping
    if session.state != crate::shared::models::constants::SESSION_STATE_SLEPT {
        return Err(ApiError::BadRequest(format!(
            "Cannot wake session in {} state - only sleeping sessions can be woken",
            session.state
        )));
    }

    // Update session state to INIT and bump activity timestamp.
    // Guard on current state to avoid races between check and update.
    let result = query(
        r#"
        UPDATE sessions 
        SET state = ?, last_activity_at = CURRENT_TIMESTAMP
        WHERE name = ? AND state = ?
        "#,
    )
    .bind(crate::shared::models::constants::SESSION_STATE_INIT)
    .bind(&session.name)
    .bind(crate::shared::models::constants::SESSION_STATE_SLEPT)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to wake session: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("Session not found".to_string()));
    }

    // Get the principal name for task creation
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Add task to restart the container with optional prompt
    // Include the session owner as principal for token generation so the
    // container can authenticate to the API even when an admin triggers wake.
    let restore_payload = serde_json::json!({
        "prompt": req.prompt,
        "reason": "user_wake",
        "principal": session.created_by,
        "principal_type": "User"
    });

    sqlx::query(
        r#"
        INSERT INTO session_tasks (session_name, task_type, created_by, payload, status)
        VALUES (?, 'wake_session', ?, ?, 'pending')
        "#,
    )
    .bind(&session.name)
    .bind(username)
    .bind(&restore_payload)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create resume task: {:?}", e);
        ApiError::Internal(anyhow::anyhow!("Failed to create resume task: {}", e))
    })?;

    tracing::info!("Created resume task for session {}", session.name);

    // Fetch updated session
    let updated_session = Session::find_by_name(&state.db, &session.name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch updated session: {}", e)))?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    Ok(Json(
        SessionResponse::from_session(updated_session, &state.db).await?,
    ))
}

pub async fn update_session(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<UpdateSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    // Admins require explicit permission; owners can update without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to update session".to_string())
            })?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by ID or name (admin can access any session for update/delete)
    let is_admin = is_admin_principal(&auth, &state).await;
    let session = find_session_by_name(&state, &name, username, is_admin).await?;
    // Enforce ownership: only admin or owner may update
    if !is_admin && session.created_by != *username {
        return Err(ApiError::Forbidden(
            "You can only update your own sessions".to_string(),
        ));
    }

    let updated_session = Session::update(&state.db, &session.name, req)
        .await
        .map_err(|e| {
            let error_msg = e.to_string();
            if error_msg.contains("No fields to update") {
                ApiError::BadRequest(error_msg)
            } else if error_msg.contains("unique_session_name")
                || error_msg.contains("Duplicate entry")
            {
                ApiError::BadRequest("A session with this name already exists".to_string())
            } else {
                ApiError::Internal(anyhow::anyhow!("Failed to update session: {}", e))
            }
        })?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    Ok(Json(
        SessionResponse::from_session(updated_session, &state.db).await?,
    ))
}

pub async fn update_session_state(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<UpdateSessionStateRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // Get session and verify ownership (same pattern as other session endpoints)
    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by ID or name (admin can access any session for update/delete)
    let is_admin = is_admin_principal(&auth, &state).await;
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Update the state with ownership verification
    let result = sqlx::query(
        "UPDATE sessions SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE name = ? AND created_by = ?"
    )
    .bind(&req.state)
    .bind(&session.name)
    .bind(username)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update session state: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound(
            "Session not found or access denied".to_string(),
        ));
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "state": req.state
    })))
}

pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<()> {
    // Check permission for deleting sessions (admin only). Owners can delete without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::SESSION_DELETE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to delete session".to_string())
            })?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by ID or name (admin can access any session for update/delete)
    let is_admin = is_admin_principal(&auth, &state).await;
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Hard delete: schedule unpublish and container+volume removal, then remove DB row
    // Queue unpublish to remove any public content
    sqlx::query(
        r#"
        INSERT INTO session_tasks (session_name, task_type, created_by, payload, status)
        VALUES (?, 'unpublish_session', ?, '{}', 'pending')
        "#,
    )
    .bind(&session.name)
    .bind(username)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create unpublish task: {}", e)))?;

    // Add task to queue for session manager to destroy container and cleanup volume
    sqlx::query(
        r#"
        INSERT INTO session_tasks (session_name, task_type, created_by, payload, status)
        VALUES (?, 'destroy_session', ?, '{}', 'pending')
        "#,
    )
    .bind(&session.name)
    .bind(username)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create destroy task: {}", e)))?;

    tracing::info!("Created destroy task for session {}", session.name);

    let deleted = Session::delete(&state.db, &session.name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to delete session: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("Session not found".to_string()));
    }

    Ok(())
}

pub async fn publish_session(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<PublishSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    // Check permission for updating sessions (admin only). Owners can publish without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to publish session".to_string())
            })?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by ID or name (admin can publish any session)
    let is_admin = is_admin_principal(&auth, &state).await;
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Check ownership (only owner or admin can publish)
    if !is_admin && session.created_by != *username {
        return Err(ApiError::Forbidden(
            "You can only publish your own sessions".to_string(),
        ));
    }

    // Publish the session
    let published_session = Session::publish(&state.db, &session.name, username, req.clone())
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to publish session: {}", e)))?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    // Create task to copy content files to public directory
    let payload = serde_json::json!({
        "content": req.content, // Content is always included in v0.4.0
        "code": req.code,
        "env": req.env
    });

    sqlx::query(
        r#"
        INSERT INTO session_tasks (session_name, task_type, created_by, payload, status)
        VALUES (?, 'publish_session', ?, ?, 'pending')
        "#,
    )
    .bind(&session.name)
    .bind(username)
    .bind(payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create publish task: {}", e)))?;

    tracing::info!("Created publish task for session {}", session.name);

    Ok(Json(
        SessionResponse::from_session(published_session, &state.db).await?,
    ))
}

pub async fn unpublish_session(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SessionResponse>> {
    // Check permission for updating sessions (admin only). Owners can unpublish without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to unpublish session".to_string())
            })?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by ID or name (admin can unpublish any session)
    let is_admin = is_admin_principal(&auth, &state).await;
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Check ownership (only owner or admin can unpublish)
    if !is_admin && session.created_by != *username {
        return Err(ApiError::Forbidden(
            "You can only unpublish your own sessions".to_string(),
        ));
    }

    // Unpublish the session
    let unpublished_session = Session::unpublish(&state.db, &session.name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to unpublish session: {}", e)))?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    // Create task to remove content files from public directory
    let payload = serde_json::json!({});

    sqlx::query(
        r#"
        INSERT INTO session_tasks (session_name, task_type, created_by, payload, status)
        VALUES (?, 'unpublish_session', ?, ?, 'pending')
        "#,
    )
    .bind(&session.name)
    .bind(username)
    .bind(payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create unpublish task: {}", e)))?;

    tracing::info!("Created unpublish task for session {}", session.name);

    Ok(Json(
        SessionResponse::from_session(unpublished_session, &state.db).await?,
    ))
}

pub async fn list_published_sessions(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<SessionResponse>>> {
    // No authentication required for listing published sessions (public access)

    let sessions = Session::find_published(&state.db).await.map_err(|e| {
        ApiError::Internal(anyhow::anyhow!("Failed to list published sessions: {}", e))
    })?;

    let mut response = Vec::new();
    for session in sessions {
        response.push(SessionResponse::from_session(session, &state.db).await?);
    }

    Ok(Json(response))
}

pub async fn get_published_session(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> ApiResult<Json<SessionResponse>> {
    // No authentication required for getting published sessions (public access)

    let session = Session::find_by_name(&state.db, &name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch session: {}", e)))?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    // Check if session is published
    if !session.is_published {
        return Err(ApiError::NotFound(
            "Session not found or not published".to_string(),
        ));
    }

    Ok(Json(
        SessionResponse::from_session(session, &state.db).await?,
    ))
}

// GET /sessions/{name}/runtime — total runtime across sessions
pub async fn get_session_runtime(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Permission: owner or admin
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::SESSION_GET)
            .await
            .map_err(|_| {
                ApiError::Forbidden("Insufficient permissions to get session runtime".to_string())
            })?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session (admin can access any session)
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Fetch all responses for this session (created_at + output JSON)
    let rows: Vec<(DateTime<Utc>, serde_json::Value)> = sqlx::query_as(
        r#"SELECT created_at, output FROM session_responses WHERE session_name = ? ORDER BY created_at ASC"#
    )
    .bind(&session.name)
    .fetch_all(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch responses: {}", e)))?;

    // Sum runtime for completed sessions; track last wake for current session inclusion
    let mut total: i64 = 0;
    let mut last_woke: Option<DateTime<Utc>> = None;
    let mut current_session: i64 = 0;
    for (row_created_at, output) in rows.into_iter() {
        if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
            for it in items {
                let t = it.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if t == "woke" {
                    let at = it
                        .get("at")
                        .and_then(|v| v.as_str())
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or(row_created_at);
                    last_woke = Some(at);
                } else if t == "slept" {
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
                        let start_at = last_woke.unwrap_or(session.created_at);
                        let delta = (end_at - start_at).num_seconds();
                        if delta > 0 {
                            total += delta;
                        }
                    }
                }
            }
        }
    }

    // Include current session up to now when session is not sleeping
    if session.state.to_lowercase() != crate::shared::models::constants::SESSION_STATE_SLEPT {
        let start_at = last_woke.unwrap_or(session.created_at);
        let now = Utc::now();
        let delta = (now - start_at).num_seconds();
        if delta > 0 {
            total += delta;
            current_session = delta;
        }
    }

    Ok(Json(serde_json::json!({
        "session_name": session.name,
        "total_runtime_seconds": total,
        "current_session_seconds": current_session
    })))
}

pub async fn update_session_to_busy(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Only the session container should be able to call this
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session (session token should match session ownership)
    let is_admin = is_admin_principal(&auth, &state).await;
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Update session to busy: clear idle_from and set busy_from (strict busy timeout)
    Session::update_session_to_busy(&state.db, &session.name)
        .await
        .map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("Failed to update session to busy: {}", e))
        })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "state": "busy",
        "timeout_status": "paused"
    })))
}

pub async fn update_session_to_idle(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Only the session container should be able to call this
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session (session token should match session ownership)
    let is_admin = is_admin_principal(&auth, &state).await;
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Update session to idle: set idle_from and clear busy_from (idle timeout active)
    Session::update_session_to_idle(&state.db, &session.name)
        .await
        .map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("Failed to update session to idle: {}", e))
        })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "state": "idle",
        "timeout_status": "active"
    })))
}
