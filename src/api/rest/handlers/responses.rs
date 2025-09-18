use axum::{extract::{Extension, Path, Query, State}, Json};
use std::sync::Arc;

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::shared::models::{AppState, AgentResponse, CreateResponseRequest, UpdateResponseRequest, ResponseView};

#[derive(Debug, serde::Deserialize)]
pub struct ListQuery { pub limit: Option<i64>, pub offset: Option<i64> }

pub async fn list_responses(
    State(state): State<Arc<AppState>>,
    Path(agent_name): Path<String>,
    Query(query): Query<ListQuery>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<Vec<ResponseView>>> {
    let _agent = crate::shared::models::Agent::find_by_name(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Agent not found".to_string()))?;

    let list = AgentResponse::find_by_agent(&state.db, &agent_name, query.limit, query.offset)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch responses: {}", e)))?;

    let result = list.into_iter().map(|r| ResponseView {
        id: r.id,
        agent_name: r.agent_name,
        status: r.status,
        input: r.input,
        output: r.output,
        created_at: r.created_at.to_rfc3339(),
        updated_at: r.updated_at.to_rfc3339(),
    }).collect();
    Ok(Json(result))
}

pub async fn create_response(
    State(state): State<Arc<AppState>>,
    Path(agent_name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateResponseRequest>,
) -> ApiResult<Json<ResponseView>> {
    use tokio::time::{sleep, Duration, Instant};

    let agent = crate::shared::models::Agent::find_by_name(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Agent not found".to_string()))?;

    // Block new responses when agent is busy
    if agent.state == crate::shared::models::constants::AGENT_STATE_BUSY {
        return Err(ApiError::Conflict("Agent is busy".to_string()));
    }

    // Resolve creator
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // If agent is sleeping, only owner can implicitly wake via this path
    if agent.state == crate::shared::models::constants::AGENT_STATE_SLEPT
        && agent.created_by != *created_by
    {
        return Err(ApiError::Forbidden(
            "You can only wake your own agents.".to_string(),
        ));
    }

    // If agent is idle, mark busy to signal work enqueued
    if agent.state == crate::shared::models::constants::AGENT_STATE_IDLE {
        sqlx::query(r#"UPDATE agents SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE name = ? AND state = ?"#)
            .bind(crate::shared::models::constants::AGENT_STATE_BUSY)
            .bind(&agent_name)
            .bind(crate::shared::models::constants::AGENT_STATE_IDLE)
            .execute(&*state.db)
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update agent state: {}", e)))?;
    }

    // Generate response id that Controller will use when inserting the DB row
    let response_id = uuid::Uuid::new_v4().to_string();

    // Enqueue Controller task to wake (if needed) and create the response row
    let payload = serde_json::json!({
        "response_id": response_id,
        "input": req.input,
        "wake_if_slept": true,
        "background": req.background.unwrap_or(true)
    });
    sqlx::query(
        r#"
        INSERT INTO agent_tasks (agent_name, task_type, created_by, payload, status)
        VALUES (?, 'create_response', ?, ?, 'pending')
        "#,
    )
    .bind(&agent_name)
    .bind(created_by)
    .bind(payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create response task: {}", e)))?;

    // If background flag is false, block until terminal state or timeout
    let background = req.background.unwrap_or(true);
    if !background {
        let start = Instant::now();
        let timeout = Duration::from_secs(15 * 60); // 15 minutes
        let poll_interval = Duration::from_millis(500);

        loop {
            // Check timeout first
            if start.elapsed() >= timeout {
                return Err(ApiError::Timeout(
                    "Timed out waiting for response to complete".to_string(),
                ));
            }

            // Reload current response by the preassigned id
            match AgentResponse::find_by_id(&state.db, &response_id).await {
                Ok(Some(cur)) => {
                    let status_lc = cur.status.to_lowercase();
                    if status_lc == "completed" || status_lc == "failed" {
                        return Ok(Json(ResponseView {
                            id: cur.id,
                            agent_name: cur.agent_name,
                            status: cur.status,
                            input: cur.input,
                            output: cur.output,
                            created_at: cur.created_at.to_rfc3339(),
                            updated_at: cur.updated_at.to_rfc3339(),
                        }));
                    }
                }
                Ok(None) => {
                    // Not inserted yet; keep waiting
                }
                Err(e) => {
                    return Err(ApiError::Internal(anyhow::anyhow!(
                        "Failed to fetch response: {}",
                        e
                    )));
                }
            }

            sleep(poll_interval).await;
        }
    }

    // Non-blocking request: return a stub ResponseView acknowledging enqueued work
    let now = chrono::Utc::now().to_rfc3339();
    Ok(Json(ResponseView {
        id: response_id,
        agent_name: agent_name,
        status: "pending".to_string(),
        input: req.input,
        output: serde_json::json!({ "text": "", "items": [] }),
        created_at: now.clone(),
        updated_at: now,
    }))
}

pub async fn update_response(
    State(state): State<Arc<AppState>>,
    Path((agent_name, response_id)): Path<(String, String)>,
    Extension(_auth): Extension<AuthContext>,
    Json(req): Json<UpdateResponseRequest>,
) -> ApiResult<Json<ResponseView>> {
    let _agent = crate::shared::models::Agent::find_by_name(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Agent not found".to_string()))?;

    // Check belongs
    if let Some(existing) = AgentResponse::find_by_id(&state.db, &response_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))? {
        if existing.agent_name != agent_name {
            return Err(ApiError::NotFound("Response not found".to_string()));
        }
    } else {
        return Err(ApiError::NotFound("Response not found".to_string()));
    }

    let updated = AgentResponse::update_by_id(&state.db, &response_id, req)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update response: {}", e)))?;

    Ok(Json(ResponseView {
        id: updated.id,
        agent_name: updated.agent_name,
        status: updated.status,
        input: updated.input,
        output: updated.output,
        created_at: updated.created_at.to_rfc3339(),
        updated_at: updated.updated_at.to_rfc3339(),
    }))
}

pub async fn get_response_count(
    State(state): State<Arc<AppState>>,
    Path(agent_name): Path<String>,
    Extension(_auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    let _agent = crate::shared::models::Agent::find_by_name(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Agent not found".to_string()))?;
    let count = AgentResponse::count_by_agent(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to count responses: {}", e)))?;
    Ok(Json(serde_json::json!({ "count": count, "agent_name": agent_name })))
}
