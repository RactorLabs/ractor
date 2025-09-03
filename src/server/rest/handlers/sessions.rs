use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::query;
use std::sync::Arc;

use crate::server::rest::error::{ApiError, ApiResult};
use crate::server::rest::middleware::AuthContext;
use crate::server::rest::rbac_enforcement::{check_api_permission, permissions};
use crate::shared::models::{
    AppState, CreateSessionRequest, PublishSessionRequest, RemixSessionRequest,
    RestoreSessionRequest, Session, UpdateSessionRequest, UpdateSessionStateRequest,
};

// Helper function to check if authenticated user is admin
fn is_admin_user(auth: &AuthContext) -> bool {
    match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Operator(op) => op.user == "admin",
        _ => false,
    }
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub name: String, // Primary key - no more id field
    pub created_by: String,
    pub state: String,
    pub parent_session_name: Option<String>, // Changed from parent_session_id
    pub created_at: String,
    pub last_activity_at: Option<String>,
    pub metadata: serde_json::Value,
    pub is_published: bool,
    pub published_at: Option<String>,
    pub published_by: Option<String>,
    pub publish_permissions: serde_json::Value,
    pub timeout_seconds: i32,
    pub auto_close_at: Option<String>,
    pub content_port: Option<i32>,
    // Removed: id, container_id, persistent_volume_id
}

#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    pub state: Option<String>,
}

impl SessionResponse {
    async fn from_session(session: Session, _pool: &sqlx::MySqlPool) -> Result<Self, ApiError> {
        Ok(Self {
            name: session.name,
            created_by: session.created_by,
            state: session.state,
            parent_session_name: session.parent_session_name,
            created_at: session.created_at.to_rfc3339(),
            last_activity_at: session.last_activity_at.map(|dt| dt.to_rfc3339()),
            metadata: session.metadata,
            is_published: session.is_published,
            published_at: session.published_at.map(|dt| dt.to_rfc3339()),
            published_by: session.published_by,
            publish_permissions: session.publish_permissions,
            timeout_seconds: session.timeout_seconds,
            auto_close_at: session.auto_close_at.map(|dt| dt.to_rfc3339()),
            content_port: session.content_port,
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

pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListSessionsQuery>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<Vec<SessionResponse>>> {
    // Check session:list permission
    check_api_permission(&auth, &state, &permissions::SESSION_LIST)
        .await
        .map_err(|_| {
            ApiError::Forbidden("Insufficient permissions to list sessions".to_string())
        })?;

    let mut sessions = Session::find_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to list sessions: {}", e)))?;

    // For non-admin users, only show their own sessions
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Check if user is admin - only admin operator can see all sessions
    let is_admin = is_admin_user(&auth);

    // For regular users, only show their own sessions
    // Admins can see all sessions
    if !is_admin {
        sessions.retain(|s| s.created_by == *username);
    }

    // Filter by state if provided
    if let Some(state_filter) = query.state {
        sessions.retain(|s| s.state == state_filter);
    }

    let mut response = Vec::new();
    for session in sessions {
        response.push(SessionResponse::from_session(session, &state.db).await?);
    }

    Ok(Json(response))
}

pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SessionResponse>> {
    // Check session:get permission
    check_api_permission(&auth, &state, &permissions::SESSION_GET)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to get session".to_string()))?;

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by name (admin can access any session)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    Ok(Json(
        SessionResponse::from_session(session, &state.db).await?,
    ))
}

pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    tracing::info!(
        "Creating session with secrets: {} keys, instructions: {}, setup: {}, prompt: {}",
        req.secrets.len(),
        req.instructions.is_some(),
        req.setup.is_some(),
        req.prompt.is_some()
    );

    // Check session:create permission
    check_api_permission(&auth, &state, &permissions::SESSION_CREATE)
        .await
        .map_err(|_| {
            ApiError::Forbidden("Insufficient permissions to create session".to_string())
        })?;

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
                tracing::error!("Database error - code: {:?}, message: '{}'", db_err.code(), db_err.message());
                if let Some(code) = db_err.code() {
                    // Simplify the condition to catch the specific error
                    if code == "23000" || code == "1062" {
                        tracing::info!("Detected database constraint violation for session {}", req.name);
                        if db_err.message().contains("sessions.PRIMARY") || db_err.message().contains("Duplicate entry") {
                            tracing::info!("Confirmed duplicate session name constraint violation");
                            return ApiError::BadRequest(format!("Session name '{}' is already taken. Please choose a different name.", req.name));
                        }
                    }
                }
            }

            ApiError::Internal(anyhow::anyhow!("Failed to create session: {}", e))
        })?;

    // Add task to queue for session manager to create container with session parameters
    let payload = serde_json::json!({
        "secrets": req.secrets,
        "instructions": req.instructions,
        "setup": req.setup,
        "prompt": req.prompt,
        "principal": created_by,
        "principal_type": match &auth.principal {
            crate::shared::rbac::AuthPrincipal::Subject(_) => "User",
            crate::shared::rbac::AuthPrincipal::Operator(_) => "Operator",
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

pub async fn remix_session(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<RemixSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    // Check session:create permission (remixing creates a new session)
    check_api_permission(&auth, &state, &permissions::SESSION_CREATE)
        .await
        .map_err(|_| {
            ApiError::Forbidden("Insufficient permissions to remix session".to_string())
        })?;

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find parent session by ID or name (admin can remix any session, users can remix published sessions)
    let is_admin = is_admin_user(&auth);
    let parent = find_session_by_name(&state, &name, username, true).await?; // Allow finding any session for remix (permission check below)

    // Check remix permissions for non-owners
    if parent.created_by != *username && !is_admin {
        // Non-owner, non-admin can only remix if session is published
        if !parent.is_published {
            return Err(ApiError::Forbidden(
                "You can only remix your own sessions or published sessions".to_string(),
            ));
        }

        // Check published remix permissions
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
                "Code remix not permitted for this published session".to_string(),
            ));
        }
        if req.secrets
            && !publish_perms
                .get("secrets")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        {
            return Err(ApiError::Forbidden(
                "Secrets remix not permitted for this published session".to_string(),
            ));
        }
        // Content is always allowed - no permission check needed
    }

    // Get the principal name for task creation (remixer becomes owner)
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Store the remix options before moving req into Session::remix
    let copy_code = req.code;
    let copy_secrets = req.secrets;
    // Content is always copied
    let copy_content = true;
    let initial_prompt = req.prompt.clone();

    let session = Session::remix(&state.db, &parent.name, req, created_by)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to remix session: {}", e)))?;

    // Add task to queue for session manager to create container with remix options
    let task_payload = serde_json::json!({
        "remix": true,
        "parent_session_name": parent.name,
        "copy_code": copy_code,
        "copy_secrets": copy_secrets,
        "copy_content": copy_content,
        "prompt": initial_prompt,
        "principal": created_by,
        "principal_type": match &auth.principal {
            crate::shared::rbac::AuthPrincipal::Subject(_) => "User",
            crate::shared::rbac::AuthPrincipal::Operator(_) => "Operator",
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

    tracing::info!("Created session task for remixed session {}", session.name);

    Ok(Json(
        SessionResponse::from_session(session, &state.db).await?,
    ))
}

pub async fn close_session(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SessionResponse>> {
    tracing::info!("Close request received for session: {}", name);
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by ID or name (admin can close any session)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_name(&state, &name, created_by, is_admin).await?;

    tracing::info!("Found session in state: {}", session.state);

    // Check permission for updating sessions
    check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
        .await
        .map_err(|e| {
            tracing::error!("Permission check failed: {:?}", e);
            ApiError::Forbidden("Insufficient permissions to close session".to_string())
        })?;

    // Allow closing own sessions or admin can close any session
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    let is_admin = is_admin_user(&auth);
    if !is_admin && session.created_by != *username {
        return Err(ApiError::Forbidden(
            "Can only close your own sessions".to_string(),
        ));
    }
    tracing::info!("Permission check passed");

    // Check current state - cannot suspend if already suspended or in error

    if session.state == crate::shared::models::constants::SESSION_STATE_CLOSED {
        return Err(ApiError::BadRequest(
            "Session is already closed".to_string(),
        ));
    }
    if session.state == crate::shared::models::constants::SESSION_STATE_ERRORED {
        return Err(ApiError::BadRequest(
            "Cannot close session in error state".to_string(),
        ));
    }

    // Update session state to suspended
    let result = sqlx::query(
        r#"
        UPDATE sessions 
        SET state = ?
        WHERE id = ?
    "#,
    )
    .bind(crate::shared::models::constants::SESSION_STATE_CLOSED)
    .bind(&session.name)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error during suspend: {:?}", e);
        ApiError::Internal(anyhow::anyhow!("Failed to close session: {}", e))
    })?;

    tracing::info!(
        "Update query executed, rows affected: {}",
        result.rows_affected()
    );

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("Session not found".to_string()));
    }

    // Add task to destroy the container but keep volume
    sqlx::query(
        r#"
        INSERT INTO session_tasks (session_name, task_type, created_by, payload, status)
        VALUES (?, 'close_session', ?, '{}', 'pending')
        "#,
    )
    .bind(&session.name)
    .bind(&created_by)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create suspend task: {:?}", e);
        ApiError::Internal(anyhow::anyhow!("Failed to create suspend task: {}", e))
    })?;

    tracing::info!("Created suspend task for session {}", name);

    // Fetch updated session
    let updated_session = Session::find_by_name(&state.db, &name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch updated session: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;

    Ok(Json(
        SessionResponse::from_session(updated_session, &state.db).await?,
    ))
}

pub async fn restore_session(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<RestoreSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    // Check permission for updating sessions
    check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
        .await
        .map_err(|_| {
            ApiError::Forbidden("Insufficient permissions to restore session".to_string())
        })?;

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by ID or name (admin can find any session, but restore has ownership restrictions)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Check ownership: Even admins cannot restore other users' sessions (only remix)
    if session.created_by != *username {
        if is_admin {
            return Err(ApiError::Forbidden(
                "Admins cannot restore other users' sessions. Use remix instead.".to_string(),
            ));
        } else {
            return Err(ApiError::Forbidden(
                "You can only restore your own sessions.".to_string(),
            ));
        }
    }

    // Check current state - can only resume if suspended
    if session.state != crate::shared::models::constants::SESSION_STATE_CLOSED {
        return Err(ApiError::BadRequest(format!(
            "Cannot restore session in {} state - only closed sessions can be restored",
            session.state
        )));
    }

    // Update session state to init (will be set to idle by host when ready)
    let result = query(
        r#"
        UPDATE sessions 
        SET state = ?
        WHERE id = ?
        "#,
    )
    .bind(crate::shared::models::constants::SESSION_STATE_INIT)
    .bind(&session.name)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to restore session: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("Session not found".to_string()));
    }

    // Get the principal name for task creation
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Add task to restart the container with optional prompt
    let restore_payload = serde_json::json!({
        "prompt": req.prompt
    });

    sqlx::query(
        r#"
        INSERT INTO session_tasks (session_name, task_type, created_by, payload, status)
        VALUES (?, 'restore_session', ?, ?, 'pending')
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
    // Check permission for updating sessions
    check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
        .await
        .map_err(|_| {
            ApiError::Forbidden("Insufficient permissions to update session".to_string())
        })?;

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by ID or name (admin can access any session for update/delete)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

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
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Update the state with ownership verification
    let result = sqlx::query(
        "UPDATE sessions SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE id = ? AND created_by = ?"
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
    // Check permission for deleting sessions
    check_api_permission(&auth, &state, &permissions::SESSION_DELETE)
        .await
        .map_err(|_| {
            ApiError::Forbidden("Insufficient permissions to delete session".to_string())
        })?;

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by ID or name (admin can access any session for update/delete)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Sessions can be soft deleted in any state

    // Add task to queue for session manager to destroy container
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
    // Check permission for updating sessions
    check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
        .await
        .map_err(|_| {
            ApiError::Forbidden("Insufficient permissions to publish session".to_string())
        })?;

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by ID or name (admin can publish any session)
    let is_admin = is_admin_user(&auth);
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
        "secrets": req.secrets
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
    // Check permission for updating sessions
    check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
        .await
        .map_err(|_| {
            ApiError::Forbidden("Insufficient permissions to unpublish session".to_string())
        })?;

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session by ID or name (admin can unpublish any session)
    let is_admin = is_admin_user(&auth);
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

pub async fn update_session_to_busy(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Only the host container should be able to call this
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session (host token should match session ownership)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Update session to busy using the new method that clears auto_close_at
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
    // Only the host container should be able to call this
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find session (host token should match session ownership)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_name(&state, &name, username, is_admin).await?;

    // Update session to idle using the new method that sets auto_close_at
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
