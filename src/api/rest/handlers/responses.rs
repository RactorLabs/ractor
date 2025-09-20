use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::Row;
use std::sync::Arc;

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::shared::models::{
    AgentResponse, AppState, CreateResponseRequest, ResponseView, UpdateResponseRequest,
};

#[derive(Debug, serde::Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

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

    let result = list
        .into_iter()
        .map(|r| ResponseView {
            id: r.id,
            agent_name: r.agent_name,
            status: r.status,
            input_content: extract_input_content(&r.input),
            output_content: extract_output_content(&r.output),
            segments: extract_segments(&r.output),
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        })
        .collect();
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

    // Soft limit guard: block when history usage since cutoff meets/exceeds limit
    let limit_tokens = soft_limit_tokens();
    let cutoff = agent.context_cutoff_at;
    let used_tokens = estimate_history_tokens_since(&state.db, &agent_name, cutoff).await?;
    if used_tokens >= limit_tokens {
        return Err(ApiError::Conflict(format!(
            "Context is full ({} / {} tokens). Clear context via POST /api/v0/agents/{}/context/clear and try again.",
            used_tokens, limit_tokens, agent_name
        )));
    }

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

    // If agent is idle (or still init), mark busy to signal work enqueued
    if agent.state == crate::shared::models::constants::AGENT_STATE_IDLE
        || agent.state == crate::shared::models::constants::AGENT_STATE_INIT
    {
        sqlx::query(r#"UPDATE agents SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE name = ? AND state = ?"#)
            .bind(crate::shared::models::constants::AGENT_STATE_BUSY)
            .bind(&agent_name)
            .bind(agent.state)
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
                            input_content: extract_input_content(&cur.input),
                            output_content: extract_output_content(&cur.output),
                            segments: extract_segments(&cur.output),
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
        input_content: extract_input_content(&req.input),
        output_content: vec![],
        segments: vec![],
        created_at: now.clone(),
        updated_at: now,
    }))
}

fn soft_limit_tokens() -> i64 {
    std::env::var("CONTEXT_SOFT_LIMIT_TOKENS")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(100_000)
}

fn avg_chars_per_token() -> f64 {
    std::env::var("AVG_CHARS_PER_TOKEN")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(4.0)
}

async fn estimate_history_tokens_since(
    pool: &sqlx::MySqlPool,
    agent_name: &str,
    cutoff: Option<DateTime<Utc>>,
) -> Result<i64, ApiError> {
    let rows = if let Some(cut) = cutoff {
        sqlx::query(
            r#"SELECT input, output FROM agent_responses WHERE agent_name = ? AND created_at >= ? ORDER BY created_at ASC"#,
        )
        .bind(agent_name)
        .bind(cut)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            r#"SELECT input, output FROM agent_responses WHERE agent_name = ? ORDER BY created_at ASC"#,
        )
        .bind(agent_name)
        .fetch_all(pool)
        .await
    }
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?;

    let mut total_chars: i64 = 0;
    for row in rows {
        let input: serde_json::Value = row.try_get("input").unwrap_or(serde_json::json!({}));
        let output: serde_json::Value = row.try_get("output").unwrap_or(serde_json::json!({}));

        if let Some(user_text) = input.get("text").and_then(|v| v.as_str()) {
            total_chars += user_text.len() as i64;
        }
        if let Some(arr) = input.get("content").and_then(|v| v.as_array()) {
            for it in arr {
                let t = it.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if t.eq_ignore_ascii_case("text") {
                    if let Some(s) = it.get("content").and_then(|v| v.as_str()) {
                        total_chars += s.len() as i64;
                    }
                }
            }
        }
        if let Some(assistant_text) = output.get("text").and_then(|v| v.as_str()) {
            total_chars += assistant_text.len() as i64;
        }
        // Structured: count output tool_result ('output' et al.) content length
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
                                    match typ.as_str() {
                                        "markdown" => {
                                            if let Some(s) =
                                                item.get("content").and_then(|v| v.as_str())
                                            {
                                                total_chars += s.len() as i64;
                                            }
                                        }
                                        "json" => {
                                            let val = item
                                                .get("content")
                                                .cloned()
                                                .unwrap_or(serde_json::Value::Null);
                                            let s = val.to_string();
                                            total_chars += s.len() as i64;
                                        }
                                        "url" => {
                                            if let Some(s) =
                                                item.get("content").and_then(|v| v.as_str())
                                            {
                                                total_chars += s.len() as i64;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
            for it in items {
                if it.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                    if let Some(out) = it.get("output") {
                        if let Some(s) = out.as_str() {
                            total_chars += s.len() as i64;
                        } else {
                            total_chars += out.to_string().len() as i64;
                        }
                    }
                }
            }
        }
    }

    let est_tokens = ((total_chars as f64) / avg_chars_per_token()).ceil() as i64;
    Ok(est_tokens)
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
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("DB error: {}", e)))?
    {
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
        input_content: extract_input_content(&updated.input),
        output_content: extract_output_content(&updated.output),
        segments: extract_segments(&updated.output),
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
    Ok(Json(
        serde_json::json!({ "count": count, "agent_name": agent_name }),
    ))
}

fn extract_input_content(input: &Value) -> Vec<Value> {
    input
        .get("content")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

fn extract_output_content(output: &Value) -> Vec<Value> {
    if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
        for it in items.iter().rev() {
            if it.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                && it.get("tool").and_then(|v| v.as_str()) == Some("output")
            {
                if let Some(arr) = it
                    .get("output")
                    .and_then(|v| v.get("items"))
                    .and_then(|v| v.as_array())
                {
                    return arr.clone();
                }
            }
        }
    }
    vec![]
}

fn extract_segments(output: &Value) -> Vec<Value> {
    output
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}
