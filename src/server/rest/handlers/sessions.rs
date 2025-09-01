use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::query;
use std::sync::Arc;

use crate::shared::models::{AppState, Session, CreateSessionRequest, RemixSessionRequest, UpdateSessionRequest, UpdateSessionStateRequest, RestoreSessionRequest, PublishSessionRequest};
use crate::server::rest::error::{ApiError, ApiResult};
use crate::server::rest::middleware::AuthContext;
use crate::server::rest::rbac_enforcement::{check_api_permission, permissions};

// Helper function to check if authenticated user is admin
fn is_admin_user(auth: &AuthContext) -> bool {
    match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Operator(op) => op.user == "admin",
        _ => false,
    }
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub id: String,
    pub created_by: String,
    pub name: Option<String>,
    pub state: String,
    pub container_id: Option<String>,
    pub persistent_volume_id: Option<String>,
    pub parent_session_id: Option<String>,
    pub created_at: String,
    pub last_activity_at: Option<String>,
    pub metadata: serde_json::Value,
    pub is_published: bool,
    pub published_at: Option<String>,
    pub published_by: Option<String>,
    pub publish_permissions: serde_json::Value,
    pub timeout_seconds: i32,
    pub auto_close_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    pub state: Option<String>,
}

impl SessionResponse {
    async fn from_session(session: Session, _pool: &sqlx::MySqlPool) -> Result<Self, ApiError> {
        Ok(Self {
            id: session.id,
            created_by: session.created_by,
            name: session.name,
            state: session.state,
            container_id: session.container_id,
            persistent_volume_id: session.persistent_volume_id,
            parent_session_id: session.parent_session_id,
            created_at: session.created_at.to_rfc3339(),
            last_activity_at: session.last_activity_at.map(|dt| dt.to_rfc3339()),
            metadata: session.metadata,
            is_published: session.is_published,
            published_at: session.published_at.map(|dt| dt.to_rfc3339()),
            published_by: session.published_by,
            publish_permissions: session.publish_permissions,
            timeout_seconds: session.timeout_seconds,
            auto_close_at: session.auto_close_at.map(|dt| dt.to_rfc3339()),
        })
    }
}

// Helper function to find session by ID or name
async fn find_session_by_id_or_name(
    state: &AppState, 
    id_or_name: &str, 
    created_by: &str,
    is_admin: bool
) -> Result<Session, ApiError> {
    // Try to find by ID first
    if let Some(session) = Session::find_by_id(&state.db, id_or_name).await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch session: {}", e)))? {
        // Admins can access any session, regular users only their own
        if is_admin || session.created_by == created_by {
            return Ok(session);
        }
    }
    
    // If not found by ID, try by name (only for owned sessions unless admin)
    if let Some(session) = Session::find_by_name(&state.db, id_or_name, created_by).await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch session by name: {}", e)))? {
        return Ok(session);
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
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to list sessions".to_string()))?;

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
    Path(id): Path<String>,
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
    
    // Find session by ID or name (admin can access any session)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_id_or_name(&state, &id, username, is_admin).await?;

    Ok(Json(SessionResponse::from_session(session, &state.db).await?))
}

pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    tracing::info!("Creating session with secrets: {} keys, instructions: {}, setup: {}, prompt: {}", 
        req.secrets.len(), 
        req.instructions.is_some(), 
        req.setup.is_some(),
        req.prompt.is_some());

    // Check session:create permission
    check_api_permission(&auth, &state, &permissions::SESSION_CREATE)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to create session".to_string()))?;


    // Get the principal name
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    let session = Session::create(&state.db, req.clone(), created_by)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create session: {:?}", e);
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

    sqlx::query(r#"
        INSERT INTO session_tasks (session_id, task_type, created_by, payload, status)
        VALUES (?, 'create_session', ?, ?, 'pending')
        "#
    )
    .bind(&session.id)
    .bind(created_by)
    .bind(payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create session task: {}", e)))?;
    
    tracing::info!("Created session task for session {}", session.id);

    Ok(Json(SessionResponse::from_session(session, &state.db).await?))
}

pub async fn remix_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<RemixSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    // Check session:create permission (remixing creates a new session)
    check_api_permission(&auth, &state, &permissions::SESSION_CREATE)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to remix session".to_string()))?;

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    
    // Find parent session by ID or name (admin can remix any session, users can remix published sessions)
    let is_admin = is_admin_user(&auth);
    let parent = find_session_by_id_or_name(&state, &id, username, true).await?; // Allow finding any session for remix (permission check below)

    // Check remix permissions for non-owners
    if parent.created_by != *username && !is_admin {
        // Non-owner, non-admin can only remix if session is published
        if !parent.is_published {
            return Err(ApiError::Forbidden("You can only remix your own sessions or published sessions".to_string()));
        }
        
        // Check published remix permissions
        let publish_perms = parent.publish_permissions.as_object()
            .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("Invalid publish permissions format")))?;
        
        if req.data && !publish_perms.get("data").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Err(ApiError::Forbidden("Data remix not permitted for this published session".to_string()));
        }
        if req.code && !publish_perms.get("code").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Err(ApiError::Forbidden("Code remix not permitted for this published session".to_string()));
        }
        if req.secrets && !publish_perms.get("secrets").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Err(ApiError::Forbidden("Secrets remix not permitted for this published session".to_string()));
        }
    }

    // Get the principal name for task creation (remixer becomes owner)
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Store the remix options before moving req into Session::remix
    let copy_data = req.data;
    let copy_code = req.code;
    let copy_secrets = req.secrets;
    let initial_prompt = req.prompt.clone();
    
    let session = Session::remix(&state.db, &parent.id, req, created_by)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to remix session: {}", e)))?;

    // Add task to queue for session manager to create container with remix options
    let task_payload = serde_json::json!({
        "remix": true,
        "parent_session_id": parent.id,
        "copy_data": copy_data,
        "copy_code": copy_code,
        "copy_secrets": copy_secrets,
        "prompt": initial_prompt,
        "principal": created_by,
        "principal_type": match &auth.principal {
            crate::shared::rbac::AuthPrincipal::Subject(_) => "User",
            crate::shared::rbac::AuthPrincipal::Operator(_) => "Operator",
        }
    });
    
    sqlx::query(r#"
        INSERT INTO session_tasks (session_id, task_type, created_by, payload, status)
        VALUES (?, 'create_session', ?, ?, 'pending')
        "#
    )
    .bind(&session.id)
    .bind(created_by)
    .bind(task_payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create session task: {}", e)))?;
    
    tracing::info!("Created session task for remixed session {}", session.id);

    Ok(Json(SessionResponse::from_session(session, &state.db).await?))
}


pub async fn close_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SessionResponse>> {
    tracing::info!("Close request received for session: {}", id);
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    }; 
    
    // Find session by ID or name (admin can close any session)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_id_or_name(&state, &id, created_by, is_admin).await?;
    
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
        return Err(ApiError::Forbidden("Can only close your own sessions".to_string()));
    }
    tracing::info!("Permission check passed");

    // Check current state - cannot suspend if already suspended or in error

    if session.state == crate::shared::models::constants::SESSION_STATE_CLOSED {
        return Err(ApiError::BadRequest("Session is already closed".to_string()));
    }
    if session.state == crate::shared::models::constants::SESSION_STATE_ERRORED {
        return Err(ApiError::BadRequest("Cannot close session in error state".to_string()));
    }

    // Update session state to suspended
    let result = sqlx::query(r#"
        UPDATE sessions 
        SET state = ?
        WHERE id = ?
    "#)
    .bind(crate::shared::models::constants::SESSION_STATE_CLOSED)
    .bind(&session.id)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error during suspend: {:?}", e);
        ApiError::Internal(anyhow::anyhow!("Failed to close session: {}", e))
    })?;

    tracing::info!("Update query executed, rows affected: {}", result.rows_affected());

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("Session not found".to_string()));
    }

    // Add task to destroy the container but keep volume
    sqlx::query(r#"
        INSERT INTO session_tasks (session_id, task_type, created_by, payload, status)
        VALUES (?, 'close_session', ?, '{}', 'pending')
        "#
    )
    .bind(&session.id)
    .bind(&created_by)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create suspend task: {:?}", e);
        ApiError::Internal(anyhow::anyhow!("Failed to create suspend task: {}", e))
    })?;
    
    tracing::info!("Created suspend task for session {}", id);

    // Fetch updated session
    let updated_session = Session::find_by_id(&state.db, &id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch updated session: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;

    Ok(Json(SessionResponse::from_session(updated_session, &state.db).await?))
}

pub async fn restore_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<RestoreSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    // Check permission for updating sessions
    check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to restore session".to_string()))?;
    
    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    
    // Find session by ID or name (admin can find any session, but restore has ownership restrictions)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_id_or_name(&state, &id, username, is_admin).await?;

    // Check ownership: Even admins cannot restore other users' sessions (only remix)
    if session.created_by != *username {
        if is_admin {
            return Err(ApiError::Forbidden("Admins cannot restore other users' sessions. Use remix instead.".to_string()));
        } else {
            return Err(ApiError::Forbidden("You can only restore your own sessions.".to_string()));
        }
    }

    // Check current state - can only resume if suspended
    if session.state != crate::shared::models::constants::SESSION_STATE_CLOSED {
        return Err(ApiError::BadRequest(format!("Cannot restore session in {} state - only closed sessions can be restored", session.state)));
    }

    // Update session state to idle
    let result = query(r#"
        UPDATE sessions 
        SET state = ?
        WHERE id = ?
        "#
    )
    .bind(crate::shared::models::constants::SESSION_STATE_IDLE)
    .bind(&session.id)
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
    
    sqlx::query(r#"
        INSERT INTO session_tasks (session_id, task_type, created_by, payload, status)
        VALUES (?, 'restore_session', ?, ?, 'pending')
        "#
    )
    .bind(&session.id)
    .bind(username)
    .bind(&restore_payload)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create resume task: {:?}", e);
        ApiError::Internal(anyhow::anyhow!("Failed to create resume task: {}", e))
    })?;
    
    tracing::info!("Created resume task for session {}", session.id);

    // Fetch updated session
    let updated_session = Session::find_by_id(&state.db, &session.id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch updated session: {}", e)))?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    Ok(Json(SessionResponse::from_session(updated_session, &state.db).await?))
}

pub async fn update_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<UpdateSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    // Check permission for updating sessions
    check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to update session".to_string()))?;
    
    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    
    // Find session by ID or name (admin can access any session for update/delete)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_id_or_name(&state, &id, username, is_admin).await?;

    let updated_session = Session::update(&state.db, &session.id, req)
        .await
        .map_err(|e| {
            if e.to_string().contains("No fields to update") {
                ApiError::BadRequest(e.to_string())
            } else {
                ApiError::Internal(anyhow::anyhow!("Failed to update session: {}", e))
            }
        })?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    Ok(Json(SessionResponse::from_session(updated_session, &state.db).await?))
}

pub async fn update_session_state(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
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
    let session = find_session_by_id_or_name(&state, &id, username, is_admin).await?;
    
    // Update the state with ownership verification
    let result = sqlx::query(
        "UPDATE sessions SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE id = ? AND created_by = ?"
    )
    .bind(&req.state)
    .bind(&session.id)
    .bind(username)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update session state: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("Session not found or access denied".to_string()));
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "state": req.state
    })))
}

pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<()> {
    // Check permission for deleting sessions
    check_api_permission(&auth, &state, &permissions::SESSION_DELETE)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to delete session".to_string()))?;
    
    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    
    // Find session by ID or name (admin can access any session for update/delete)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_id_or_name(&state, &id, username, is_admin).await?;

    // Sessions can be soft deleted in any state

    // Add task to queue for session manager to destroy container
    sqlx::query(r#"
        INSERT INTO session_tasks (session_id, task_type, created_by, payload, status)
        VALUES (?, 'destroy_session', ?, '{}', 'pending')
        "#
    )
    .bind(&session.id)
    .bind(username)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create destroy task: {}", e)))?;
    
    tracing::info!("Created destroy task for session {}", session.id);

    let deleted = Session::delete(&state.db, &session.id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to delete session: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("Session not found".to_string()));
    }

    Ok(())
}

pub async fn publish_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<PublishSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    // Check permission for updating sessions
    check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to publish session".to_string()))?;
    
    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    
    // Find session by ID or name (admin can publish any session)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_id_or_name(&state, &id, username, is_admin).await?;

    // Check ownership (only owner or admin can publish)
    if !is_admin && session.created_by != *username {
        return Err(ApiError::Forbidden("You can only publish your own sessions".to_string()));
    }

    // Publish the session
    let published_session = Session::publish(&state.db, &session.id, username, req)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to publish session: {}", e)))?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    Ok(Json(SessionResponse::from_session(published_session, &state.db).await?))
}

pub async fn unpublish_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SessionResponse>> {
    // Check permission for updating sessions
    check_api_permission(&auth, &state, &permissions::SESSION_UPDATE)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to unpublish session".to_string()))?;
    
    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    
    // Find session by ID or name (admin can unpublish any session)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_id_or_name(&state, &id, username, is_admin).await?;

    // Check ownership (only owner or admin can unpublish)
    if !is_admin && session.created_by != *username {
        return Err(ApiError::Forbidden("You can only unpublish your own sessions".to_string()));
    }

    // Unpublish the session
    let unpublished_session = Session::unpublish(&state.db, &session.id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to unpublish session: {}", e)))?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    Ok(Json(SessionResponse::from_session(unpublished_session, &state.db).await?))
}

pub async fn list_published_sessions(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<SessionResponse>>> {
    // No authentication required for listing published sessions (public access)
    
    let sessions = Session::find_published(&state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to list published sessions: {}", e)))?;

    let mut response = Vec::new();
    for session in sessions {
        response.push(SessionResponse::from_session(session, &state.db).await?);
    }

    Ok(Json(response))
}

pub async fn get_published_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<SessionResponse>> {
    // No authentication required for getting published sessions (public access)
    
    let session = Session::find_by_id(&state.db, &id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch session: {}", e)))?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    // Check if session is published
    if !session.is_published {
        return Err(ApiError::NotFound("Session not found or not published".to_string()));
    }

    Ok(Json(SessionResponse::from_session(session, &state.db).await?))
}

pub async fn update_session_to_busy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Only the host container should be able to call this
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    
    // Find session (host token should match session ownership)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_id_or_name(&state, &id, username, is_admin).await?;
    
    // Update session to busy using the new method that clears auto_close_at
    Session::update_session_to_busy(&state.db, &session.id).await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update session to busy: {}", e)))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "state": "busy",
        "timeout_status": "paused"
    })))
}

pub async fn update_session_to_idle(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Only the host container should be able to call this
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    
    // Find session (host token should match session ownership)
    let is_admin = is_admin_user(&auth);
    let session = find_session_by_id_or_name(&state, &id, username, is_admin).await?;
    
    // Update session to idle using the new method that sets auto_close_at
    Session::update_session_to_idle(&state.db, &session.id).await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update session to idle: {}", e)))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "state": "idle", 
        "timeout_status": "active"
    })))
}