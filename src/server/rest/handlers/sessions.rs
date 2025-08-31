use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::query;
use std::sync::Arc;

use crate::shared::models::{AppState, Session, CreateSessionRequest, RemixSessionRequest, UpdateSessionRequest, UpdateSessionStateRequest};
use crate::server::rest::error::{ApiError, ApiResult};
use crate::server::rest::middleware::AuthContext;
use crate::server::rest::rbac_enforcement::{check_api_permission, permissions};

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub id: String,
    pub created_by: String,
    pub state: String,
    pub container_id: Option<String>,
    pub persistent_volume_id: Option<String>,
    pub parent_session_id: Option<String>,
    pub created_at: String,
    pub last_activity_at: Option<String>,
    pub metadata: serde_json::Value,
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
            state: session.state,
            container_id: session.container_id,
            persistent_volume_id: session.persistent_volume_id,
            parent_session_id: session.parent_session_id,
            created_at: session.created_at.to_rfc3339(),
            last_activity_at: session.last_activity_at.map(|dt| dt.to_rfc3339()),
            metadata: session.metadata,
        })
    }
}

pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListSessionsQuery>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<Vec<SessionResponse>>> {
    // Check session:list permission
    check_api_permission(&auth, &state, &permissions::SESSION_LIST, None)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to list sessions".to_string()))?;

    let mut sessions = Session::find_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to list sessions: {}", e)))?;

    // For non-admin users, only show their own sessions
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
    };
    
    // For regular users, only show their own sessions
    // Admins have permission to list all sessions and will see all sessions
    sessions.retain(|s| s.created_by == *username);

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
    check_api_permission(&auth, &state, &permissions::SESSION_GET, None)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to get session".to_string()))?;

    let session = Session::find_by_id(&state.db, &id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch session: {}", e)))?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    // Only allow access to own sessions (ownership-based access control)
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
    };
    
    if session.created_by != *username {
        return Err(ApiError::Forbidden("Can only access your own sessions".to_string()));
    }

    Ok(Json(SessionResponse::from_session(session, &state.db).await?))
}

pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateSessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    tracing::info!("Creating session with secrets: {} keys, instructions: {}, setup: {}", 
        req.secrets.len(), 
        req.instructions.is_some(), 
        req.setup.is_some());

    // Check session:create permission
    check_api_permission(&auth, &state, &permissions::SESSION_CREATE, None)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to create session".to_string()))?;

    // Get the principal name
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
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
        "setup": req.setup
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
    check_api_permission(&auth, &state, &permissions::SESSION_CREATE, None)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to remix session".to_string()))?;

    // Check if parent session exists and user has access to it
    let parent = Session::find_by_id(&state.db, &id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch parent session: {}", e)))?
        .ok_or(ApiError::NotFound("Parent session not found".to_string()))?;

    // Only allow remixing own sessions (ownership-based access control)
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
    };
    
    if parent.created_by != *username {
        return Err(ApiError::Forbidden("Can only remix your own sessions".to_string()));
    }

    let session = Session::remix(&state.db, &id, req)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to remix session: {}", e)))?;

    // Get the principal name for task creation
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
    };

    // Add task to queue for session manager to create container (same as create_session)
    sqlx::query(r#"
        INSERT INTO session_tasks (session_id, task_type, created_by, payload, status)
        VALUES (?, 'create_session', ?, ?, 'pending')
        "#
    )
    .bind(&session.id)
    .bind(created_by)
    .bind(serde_json::json!({}))
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create session task: {}", e)))?;
    
    tracing::info!("Created session task for remixed session {}", session.id);

    Ok(Json(SessionResponse::from_session(session, &state.db).await?))
}

// pub async fn update_session_state(
//     State(state): State<Arc<AppState>>,
//     Path(id): Path<String>,
//     Extension(auth): Extension<AuthContext>,
//     Json(req): Json<UpdateSessionStateRequest>,
// ) -> ApiResult<Json<SessionResponse>> {
//     use crate::shared::rbac::AuthPrincipal;
//     
// 
//     // Check if session exists and user has access
//     let session = Session::find_by_id(&state.db, session_id)
//         .await
//         .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch session: {}", e)))?
//         .ok_or(ApiError::NotFound("Session not found".to_string()))?;
// 
//     let username = match &auth.principal {
//         AuthPrincipal::Subject(s) => &s.name,
//         AuthPrincipal::ServiceAccount(sa) => &sa.user,
//     };
// 
//     // Check permission for updating sessions in the space
//     let can_update = check_api_permission(&auth, &state, &permissions::SESSION_UPDATE, Some(&session.space))
//         .await
//         .is_ok();
//     
//     if !can_update && &session.created_by != username {
//         return Err(ApiError::Forbidden("Cannot update other users' sessions".to_string()));
//     }
// 
//     // Store old state for comparison
//     let old_state = session.state;
//     let new_state = req.state;
//     
//     let updated_session = Session::update_state(&state.db, session_id, req)
//         .await
//         .map_err(|e| {
//             if e.to_string().contains("Invalid state transition") {
//                 ApiError::BadRequest(e.to_string())
//             } else {
//                 ApiError::Internal(anyhow::anyhow!("Failed to update session state: {}", e))
//             }
//         })?
//         .ok_or(ApiError::NotFound("Session not found".to_string()))?;
// 
//     // Add tasks for container state transitions
//     match (old_state, new_state) {
//         (SessionState::Init, SessionState::Ready) => {
//             // Container should be created by this point
//             tracing::debug!("Session {} transitioned to Ready", session_id);
//         }
//         (SessionState::Ready, SessionState::Idle) => {
//             // Add task to stop container
//             let _ = sqlx::query(r#"
//                 INSERT INTO session_tasks (session_id, task_type, payload, status)
//                 VALUES (?, 'stop_session', '{}', 'pending')
//                 "#
//             )
//             .bind(session_id)
//             .execute(&*state.db)
//             .await;
//         }
//         (SessionState::Idle, SessionState::Ready) => {
//             // Add task to reactivate container
//             let _ = sqlx::query(r#"
//                 INSERT INTO session_tasks (session_id, task_type, payload, status)
//                 VALUES (?, 'reactivate_session', '{}', 'pending')
//                 "#
//             )
//             .bind(session_id)
//             .execute(&*state.db)
//             .await;
//         }
//         _ => {
//             tracing::debug!("Session {} state transition {:?} -> {:?}", session_id, old_state, new_state);
//         }
//     }
// 
//     Ok(Json(SessionResponse::from_session(updated_session, &state.db).await?))
// }

pub async fn close_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SessionResponse>> {
    tracing::info!("Close request received for session: {}", id);
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
    }; 
    
    // Check if session exists and user has access
    let session = match Session::find_by_id(&state.db, &id).await {
        Ok(Some(s)) => s,
        Ok(None) => return Err(ApiError::NotFound("Session not found".to_string())),
        Err(e) => {
            tracing::error!("Database error: {:?}", e);
            return Err(ApiError::Internal(anyhow::anyhow!("Database error: {}", e)));
        }
    };
    
    tracing::info!("Found session in state: {}", session.state);

    // Check permission for updating sessions
    check_api_permission(&auth, &state, &permissions::SESSION_UPDATE, None)
        .await
        .map_err(|e| {
            tracing::error!("Permission check failed: {:?}", e);
            ApiError::Forbidden("Insufficient permissions to close session".to_string())
        })?;
    
    // Only allow closing own sessions (ownership-based access control)
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
    };
    
    if session.created_by != *username {
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
    .bind(id.clone())
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
    .bind(&id)
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
) -> ApiResult<Json<SessionResponse>> {
    // Check if session exists
    let session = Session::find_by_id(&state.db, &id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch session: {}", e)))?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    // Check permission for updating sessions
    check_api_permission(&auth, &state, &permissions::SESSION_UPDATE, None)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to restore session".to_string()))?;
    
    // Only allow restoring own sessions (ownership-based access control)
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
    };
    
    if session.created_by != *username {
        return Err(ApiError::Forbidden("Can only restore your own sessions".to_string()));
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
    .bind(id.clone())
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to restore session: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("Session not found".to_string()));
    }

    // Get the principal name for task creation
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
    };

    // Add task to restart the container
    sqlx::query(r#"
        INSERT INTO session_tasks (session_id, task_type, created_by, payload, status)
        VALUES (?, 'restore_session', ?, '{}', 'pending')
        "#
    )
    .bind(&id)
    .bind(created_by)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create resume task: {:?}", e);
        ApiError::Internal(anyhow::anyhow!("Failed to create resume task: {}", e))
    })?;
    
    tracing::info!("Created resume task for session {}", id);

    // Fetch updated session
    let updated_session = Session::find_by_id(&state.db, &id)
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
    // Check if session exists
    let session = Session::find_by_id(&state.db, &id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch session: {}", e)))?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    // Check permission for updating sessions
    check_api_permission(&auth, &state, &permissions::SESSION_UPDATE, None)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to update session".to_string()))?;
    
    // Only allow updating own sessions (ownership-based access control)
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
    };
    
    if session.created_by != *username {
        return Err(ApiError::Forbidden("Can only update your own sessions".to_string()));
    }

    let updated_session = Session::update(&state.db, &id, req)
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
    Extension(_auth): Extension<AuthContext>,
    Json(req): Json<UpdateSessionStateRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // For operator service accounts, allow state updates without permission checks
    // This is needed for the host agent to update session states
    
    // Just update the state directly - operators manage containers and need this access
    let result = sqlx::query(
        "UPDATE sessions SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE id = ?"
    )
    .bind(&req.state)
    .bind(&id)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update session state: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("Session not found".to_string()));
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
    // Check if session exists
    let session = Session::find_by_id(&state.db, &id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch session: {}", e)))?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    // Check permission for deleting sessions
    check_api_permission(&auth, &state, &permissions::SESSION_DELETE, None)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to delete session".to_string()))?;
    
    // Only allow deleting own sessions (ownership-based access control)
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
    };
    
    if session.created_by != *username {
        return Err(ApiError::Forbidden("Can only delete your own sessions".to_string()));
    }

    // Get the principal name
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
    };

    // Sessions can be soft deleted in any state

    // Add task to queue for session manager to destroy container
    sqlx::query(r#"
        INSERT INTO session_tasks (session_id, task_type, created_by, payload, status)
        VALUES (?, 'destroy_session', ?, '{}', 'pending')
        "#
    )
    .bind(&id)
    .bind(created_by)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create destroy task: {}", e)))?;
    
    tracing::info!("Created destroy task for session {}", id);

    let deleted = Session::delete(&state.db, &id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to delete session: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("Session not found".to_string()));
    }

    Ok(())
}