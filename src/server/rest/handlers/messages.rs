use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use sqlx;
use std::sync::Arc;

use crate::server::rest::error::{ApiError, ApiResult};
use crate::server::rest::middleware::AuthContext;
use crate::shared::models::constants::{
    AGENT_STATE_BUSY, AGENT_STATE_SLEPT, AGENT_STATE_IDLE, AGENT_STATE_INIT,
};
use crate::shared::models::{
    AppState, CreateMessageRequest, ListMessagesQuery, MessageResponse, AgentMessage,
};

pub async fn create_message(
    State(state): State<Arc<AppState>>,
    Path(agent_name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateMessageRequest>,
) -> ApiResult<Json<MessageResponse>> {
    // Verify agent exists and user has access
    let agent = crate::shared::models::Agent::find_by_name(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Agent not found".to_string()))?;

    // Check if agent is sleeping and needs reactivation
    if agent.state == crate::shared::models::constants::AGENT_STATE_SLEPT {
        tracing::info!(
            "Auto-waking sleeping agent {} due to new message",
            agent_name
        );

        // Update agent state to INIT (will be set to idle by agent when ready)
        sqlx::query(r#"UPDATE agents SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE name = ? AND state = ?"#
        )
        .bind(AGENT_STATE_INIT)
        .bind(&agent_name)
        .bind(AGENT_STATE_SLEPT)
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update agent state: {}", e)))?;

        // Add task to reactivate container with this message queued
        let payload = serde_json::json!({
            "auto_wake": true,
            "triggered_by_message": true
        });

        sqlx::query(
            r#"
            INSERT INTO agent_tasks (agent_name, task_type, created_by, payload, status)
            VALUES (?, 'wake_agent', ?, ?, 'pending')
            "#,
        )
        .bind(&agent_name)
        .bind(&agent.created_by) // Use agent owner for proper token generation
        .bind(payload.to_string())
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create wake task: {}", e)))?;

        tracing::info!(
            "Wake task created for agent {} - container will be recreated",
            agent_name
        );
    } else if agent.state == crate::shared::models::constants::AGENT_STATE_IDLE {
        // Update agent to BUSY when processing a message
        sqlx::query(r#"UPDATE agents SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE name = ? AND state = ?"#
        )
        .bind(AGENT_STATE_BUSY)
        .bind(&agent_name)
        .bind(AGENT_STATE_IDLE)
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update agent state: {}", e)))?;
    }

    // Get the principal name
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Create the message
    let message = AgentMessage::create(&state.db, &agent_name, created_by, req)
        .await
        .map_err(|e| {
            eprintln!("Database error creating message: {e:?}");
            ApiError::Internal(anyhow::anyhow!("Failed to create message: {}", e))
        })?;

    Ok(Json(MessageResponse {
        id: message.id,
        agent_name: message.agent_name,
        role: message.role,
        content: message.content,
        metadata: message.metadata,
        created_at: message.created_at.to_rfc3339(),
    }))
}

pub async fn list_messages(
    State(state): State<Arc<AppState>>,
    Path(agent_name): Path<String>,
    Query(query): Query<ListMessagesQuery>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<Vec<MessageResponse>>> {
    // Validate query parameters
    if let Some(limit) = query.limit {
        if limit < 0 {
            return Err(ApiError::BadRequest(
                "limit must be non-negative".to_string(),
            ));
        }
        if limit > 1000 {
            return Err(ApiError::BadRequest(
                "limit must not exceed 1000".to_string(),
            ));
        }
    }

    if let Some(offset) = query.offset {
        if offset < 0 {
            return Err(ApiError::BadRequest(
                "offset must be non-negative".to_string(),
            ));
        }
    }

    // Verify agent exists
    let _agent = crate::shared::models::Agent::find_by_name(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Agent not found".to_string()))?;

    // Get messages - simplified for now
    let messages =
        AgentMessage::find_by_agent(&state.db, &agent_name, query.limit, query.offset)
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch messages: {}", e)))?;

    // Convert to MessageResponse
    let response_messages: Vec<MessageResponse> = messages
        .into_iter()
        .map(|msg| MessageResponse {
            id: msg.id,
            agent_name: msg.agent_name,
            role: msg.role,
            content: msg.content,
            metadata: msg.metadata,
            created_at: msg.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(response_messages))
}

pub async fn get_message_count(
    State(state): State<Arc<AppState>>,
    Path(agent_name): Path<String>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Verify agent exists
    let _agent = crate::shared::models::Agent::find_by_name(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Agent not found".to_string()))?;

    let count = AgentMessage::count_by_agent(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to count messages: {}", e)))?;

    Ok(Json(serde_json::json!({
        "count": count,
        "agent_name": agent_name
    })))
}

pub async fn clear_messages(
    State(state): State<Arc<AppState>>,
    Path(agent_name): Path<String>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Verify agent exists
    let _agent = crate::shared::models::Agent::find_by_name(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Agent not found".to_string()))?;

    let deleted_count = AgentMessage::delete_by_agent(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to delete messages: {}", e)))?;

    Ok(Json(serde_json::json!({
        "deleted": deleted_count,
        "agent_name": agent_name
    })))
}
