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

    // If agent is sleeping, wake via Controller before creating the response.
    // This ensures the agent's RAWORC_TASK_CREATED_AT precedes the response timestamp
    // so the agent will pick it up on wake.
    if agent.state == crate::shared::models::constants::AGENT_STATE_SLEPT {
        // Only the owner may wake their own agent (match wake_agent semantics)
        if agent.created_by != *created_by {
            return Err(ApiError::Forbidden(
                "You can only wake your own agents.".to_string(),
            ));
        }

        // Update agent state to INIT and bump activity timestamp if currently slept
        let result = sqlx::query(
            r#"
            UPDATE agents 
            SET state = 'init', last_activity_at = CURRENT_TIMESTAMP
            WHERE name = ? AND state = 'slept'
            "#,
        )
        .bind(&agent_name)
        .execute(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to wake agent: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(ApiError::NotFound("Agent not found".to_string()));
        }

        // Create wake task for Controller with a small prompt
        let wake_payload = serde_json::json!({ "prompt": "Incoming chat message" });
        sqlx::query(
            r#"
            INSERT INTO agent_tasks (agent_name, task_type, created_by, payload, status)
            VALUES (?, 'wake_agent', ?, ?, 'pending')
            "#,
        )
        .bind(&agent_name)
        .bind(created_by)
        .bind(&wake_payload)
        .execute(&*state.db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create wake task: {:?}", e);
            ApiError::Internal(anyhow::anyhow!("Failed to create wake task: {}", e))
        })?;
    }

    // If agent is idle, mark busy
    if agent.state == crate::shared::models::constants::AGENT_STATE_IDLE {
        sqlx::query(r#"UPDATE agents SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE name = ? AND state = ?"#)
            .bind(crate::shared::models::constants::AGENT_STATE_BUSY)
            .bind(&agent_name)
            .bind(crate::shared::models::constants::AGENT_STATE_IDLE)
            .execute(&*state.db)
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update agent state: {}", e)))?;
    }

    let created = AgentResponse::create(&state.db, &agent_name, created_by, req.clone())
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create response: {}", e)))?;

    // If background flag is false, block until terminal state or timeout
    let background = req.background.unwrap_or(true);
    if !background {
        use tokio::time::{sleep, Duration, Instant};

        let start = Instant::now();
        let timeout = Duration::from_secs(15 * 60); // 15 minutes
        let poll_interval = Duration::from_millis(500);

        loop {
            // Check timeout first
            if start.elapsed() >= timeout {
                return Err(ApiError::Timeout("Timed out waiting for response to complete".to_string()));
            }

            // Reload current response
            match AgentResponse::find_by_id(&state.db, &created.id).await {
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
                    // Response disappeared; treat as not found
                    return Err(ApiError::NotFound("Response not found".to_string()));
                }
                Err(e) => {
                    return Err(ApiError::Internal(anyhow::anyhow!(format!(
                        "Failed to fetch response: {}",
                        e
                    ))));
                }
            }

            sleep(poll_interval).await;
        }
    }

    Ok(Json(ResponseView {
        id: created.id,
        agent_name: created.agent_name,
        status: created.status,
        input: created.input,
        output: created.output,
        created_at: created.created_at.to_rfc3339(),
        updated_at: created.updated_at.to_rfc3339(),
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
