use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use std::sync::Arc;
use sqlx;

use crate::shared::models::{
    AppState, SessionMessage, CreateMessageRequest, MessageResponse, ListMessagesQuery
};
use crate::shared::models::constants::{SESSION_STATE_INIT, SESSION_STATE_IDLE, SESSION_STATE_BUSY, SESSION_STATE_CLOSED};
use crate::server::rest::error::{ApiError, ApiResult};
use crate::server::rest::middleware::AuthContext;

pub async fn create_message(
    State(state): State<Arc<AppState>>,
    Path(session_name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateMessageRequest>,
) -> ApiResult<Json<MessageResponse>> {
    
    // Verify session exists and user has access
    let session = crate::shared::models::Session::find_by_name(&state.db, &session_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;
    
    // Check if session is closed and needs reactivation
    if session.state == crate::shared::models::constants::SESSION_STATE_CLOSED {
        tracing::info!("Auto-restoring closed session {} due to new message", session_name);
        
        // Update session state to INIT (will be set to idle by host when ready)
        sqlx::query(r#"UPDATE sessions SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE name = ? AND state = ?"#
        )
        .bind(SESSION_STATE_INIT)
        .bind(&session_name)
        .bind(SESSION_STATE_CLOSED)
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update session state: {}", e)))?;
        
        // Add task to reactivate container with this message queued
        let payload = serde_json::json!({
            "auto_restore": true,
            "triggered_by_message": true
        });
        
        sqlx::query(r#"
            INSERT INTO session_tasks (session_name, task_type, created_by, payload, status)
            VALUES (?, 'restore_session', ?, ?, 'pending')
            "#
        )
        .bind(&session_name)
        .bind(&session.created_by)  // Use session owner for proper token generation
        .bind(payload.to_string())
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create restore task: {}", e)))?;
        
        tracing::info!("Restore task created for session {} - container will be recreated", session_name);
    } else if session.state == crate::shared::models::constants::SESSION_STATE_IDLE {
        // Update session to BUSY when processing a message
        sqlx::query(r#"UPDATE sessions SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE name = ? AND state = ?"#
        )
        .bind(SESSION_STATE_BUSY)
        .bind(&session_name)
        .bind(SESSION_STATE_IDLE)
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update session state: {}", e)))?;
    }
    
    // Get the principal name
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    
    // Create the message
    let message = SessionMessage::create(&state.db, &session_name, created_by, req)
        .await
        .map_err(|e| {
            eprintln!("Database error creating message: {e:?}");
            ApiError::Internal(anyhow::anyhow!("Failed to create message: {}", e))
        })?;
    
    Ok(Json(MessageResponse {
        id: message.id,
        session_name: message.session_name,
        role: message.role,
        content: message.content,
        metadata: message.metadata,
        created_at: message.created_at.to_rfc3339(),
    }))
}

pub async fn list_messages(
    State(state): State<Arc<AppState>>,
    Path(session_name): Path<String>,
    Query(query): Query<ListMessagesQuery>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<Vec<MessageResponse>>> {
    // Validate query parameters
    if let Some(limit) = query.limit {
        if limit < 0 {
            return Err(ApiError::BadRequest("limit must be non-negative".to_string()));
        }
        if limit > 1000 {
            return Err(ApiError::BadRequest("limit must not exceed 1000".to_string()));
        }
    }
    
    if let Some(offset) = query.offset {
        if offset < 0 {
            return Err(ApiError::BadRequest("offset must be non-negative".to_string()));
        }
    }

    // Verify session exists
    let _session = crate::shared::models::Session::find_by_name(&state.db, &session_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;
    
    // Get messages - simplified for now
    let messages = SessionMessage::find_by_session(
        &state.db, 
        &session_name,
        query.limit,
        query.offset
    )
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch messages: {}", e)))?;
    
    // Convert to MessageResponse
    let response_messages: Vec<MessageResponse> = messages.into_iter().map(|msg| MessageResponse {
        id: msg.id,
        session_name: msg.session_name,
        role: msg.role,
        content: msg.content,
        metadata: msg.metadata,
        created_at: msg.created_at.to_rfc3339(),
    }).collect();
    
    Ok(Json(response_messages))
}

pub async fn get_message_count(
    State(state): State<Arc<AppState>>,
    Path(session_name): Path<String>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Verify session exists
    let _session = crate::shared::models::Session::find_by_name(&state.db, &session_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;
    
    let count = SessionMessage::count_by_session(&state.db, &session_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to count messages: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "count": count,
        "session_name": session_name
    })))
}

pub async fn clear_messages(
    State(state): State<Arc<AppState>>,
    Path(session_name): Path<String>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Verify session exists
    let _session = crate::shared::models::Session::find_by_name(&state.db, &session_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;
    
    let deleted_count = SessionMessage::delete_by_session(&state.db, &session_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to delete messages: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "deleted": deleted_count,
        "session_name": session_name
    })))
}