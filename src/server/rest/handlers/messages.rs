use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use std::sync::Arc;
use sqlx;

use crate::shared::models::{
    AppState, SessionMessage, CreateMessageRequest, MessageResponse, ListMessagesQuery
};
use crate::shared::models::constants::{SESSION_STATE_IDLE, SESSION_STATE_BUSY, SESSION_STATE_CLOSED};
use crate::server::rest::error::{ApiError, ApiResult};
use crate::server::rest::middleware::AuthContext;

pub async fn create_message(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateMessageRequest>,
) -> ApiResult<Json<MessageResponse>> {
    
    // Verify session exists and user has access
    let session = crate::shared::models::Session::find_by_id(&state.db, &session_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;
    
    // Check if session is closed and needs reactivation
    if session.state == crate::shared::models::constants::SESSION_STATE_CLOSED {
        tracing::info!("Auto-restoring closed session {} due to new message", session_id);
        
        // Update session state to IDLE (container will be restored)
        sqlx::query(r#"UPDATE sessions SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE id = ? AND state = ?"#
        )
        .bind(SESSION_STATE_IDLE)
        .bind(&session_id)
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
            INSERT INTO session_tasks (session_id, task_type, created_by, payload, status)
            VALUES (?, 'restore_session', ?, ?, 'pending')
            "#
        )
        .bind(&session_id)
        .bind(&session.created_by)  // Use session owner for proper token generation
        .bind(payload.to_string())
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create restore task: {}", e)))?;
        
        tracing::info!("Restore task created for session {} - container will be recreated", session_id);
    } else if session.state == crate::shared::models::constants::SESSION_STATE_IDLE {
        // Update session to BUSY when processing a message
        sqlx::query(r#"UPDATE sessions SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE id = ? AND state = ?"#
        )
        .bind(SESSION_STATE_BUSY)
        .bind(&session_id)
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
    let message = SessionMessage::create(&state.db, &session_id, created_by, req)
        .await
        .map_err(|e| {
            eprintln!("Database error creating message: {e:?}");
            ApiError::Internal(anyhow::anyhow!("Failed to create message: {}", e))
        })?;
    
    Ok(Json(MessageResponse {
        id: message.id,
        session_id: message.session_id,
        role: message.role,
        content: message.content,
        metadata: message.metadata,
        created_at: message.created_at.to_rfc3339(),
    }))
}

pub async fn list_messages(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Query(query): Query<ListMessagesQuery>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<Vec<MessageResponse>>> {
    // Verify session exists
    let _session = crate::shared::models::Session::find_by_id(&state.db, &session_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;
    
    // Get messages - simplified for now
    let messages = SessionMessage::find_by_session(
        &state.db, 
        &session_id,
        query.limit,
        query.offset
    )
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch messages: {}", e)))?;
    
    // Convert to MessageResponse
    let response_messages: Vec<MessageResponse> = messages.into_iter().map(|msg| MessageResponse {
        id: msg.id,
        session_id: msg.session_id,
        role: msg.role,
        content: msg.content,
        metadata: msg.metadata,
        created_at: msg.created_at.to_rfc3339(),
    }).collect();
    
    Ok(Json(response_messages))
}

pub async fn get_message_count(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Verify session exists
    let _session = crate::shared::models::Session::find_by_id(&state.db, &session_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;
    
    let count = SessionMessage::count_by_session(&state.db, &session_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to count messages: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "count": count,
        "session_id": session_id
    })))
}

pub async fn clear_messages(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Verify session exists
    let _session = crate::shared::models::Session::find_by_id(&state.db, &session_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;
    
    let deleted_count = SessionMessage::delete_by_session(&state.db, &session_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to delete messages: {}", e)))?;
    
    Ok(Json(serde_json::json!({
        "deleted": deleted_count,
        "session_id": session_id
    })))
}