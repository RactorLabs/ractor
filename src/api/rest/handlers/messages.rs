use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use openai_harmony::{load_harmony_encoding, HarmonyEncodingName};
use openai_harmony::chat::{Message as HMessage, Role as HRole, Content as HContent, Author as HAuthor};
use sqlx;
use std::sync::Arc;

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::shared::models::constants::{
    AGENT_STATE_BUSY, AGENT_STATE_IDLE, AGENT_STATE_INIT, AGENT_STATE_SLEPT,
};
use crate::shared::models::{
    message::UpdateMessageRequest, AgentMessage, AppState, CreateMessageRequest, ListMessagesQuery, MessageResponse,
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

    // Context guard: if compacting, block; else if user posting would overflow, block and request compact
    let role_is_user = req.role.as_str() == crate::shared::models::constants::MESSAGE_ROLE_USER;
    if role_is_user {
        if agent
            .metadata
            .get("compact_in_progress")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return Err(ApiError::Conflict(
                "Compaction in progress — please wait for summary before sending more messages".to_string(),
            ));
        }
        let (used_tokens_candidate, max_tokens) =
            compute_context_usage(&state, &agent_name, Some(req.content.clone()))
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to compute context: {}", e)))?;
        if used_tokens_candidate > max_tokens {
            return Err(ApiError::BadRequest(
                "Context is full — please compact before sending more messages".to_string(),
            ));
        }
    }

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
        author_name: message.author_name,
        recipient: message.recipient,
        channel: message.channel,
        content: message.content,
        content_type: message.content_type,
        content_json: message.content_json,
        metadata: message.metadata,
        created_at: message.created_at.to_rfc3339(),
    }))
}

pub async fn get_context_usage(
    State(state): State<Arc<AppState>>,
    Path(agent_name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Verify agent exists
    let _agent = crate::shared::models::Agent::find_by_name(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Agent not found".to_string()))?;

    let (used, max) = compute_context_usage(&state, &agent_name, None)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to compute context: {}", e)))?;
    let remaining = if used > max { 0 } else { max - used };

    Ok(Json(serde_json::json!({
        "agent_name": agent_name,
        "max_tokens": max,
        "used_tokens": used,
        "remaining_tokens": remaining,
        "computed_at": chrono::Utc::now().to_rfc3339(),
    })))
}

pub async fn request_compact(
    State(state): State<Arc<AppState>>,
    Path(agent_name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Verify agent exists
    let agent = crate::shared::models::Agent::find_by_name(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Agent not found".to_string()))?;

    // If compaction already in progress, allow retry only if timed out
    if agent
        .metadata
        .get("compact_in_progress")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        let timeout_secs: i64 = std::env::var("RAWORC_COMPACT_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(600);
        let mut timed_out = true; // default to allow if timestamp missing
        if let Some(ts) = agent
            .metadata
            .get("compact_requested_at")
            .and_then(|v| v.as_str())
        {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                let dt_utc = dt.with_timezone(&chrono::Utc);
                let age = (chrono::Utc::now() - dt_utc).num_seconds();
                timed_out = age >= timeout_secs;
            }
        }
        if !timed_out {
            return Err(ApiError::Conflict(
                "Compaction already in progress for this agent".to_string(),
            ));
        }
        // else proceed and refresh request below
    }

    // Set compact_from and mark compaction in progress
    let now = chrono::Utc::now();
    let mut meta = agent.metadata.clone();
    let obj_ref_exists = meta.is_object();
    if !obj_ref_exists { meta = serde_json::json!({}); }
    let obj = meta.as_object_mut().expect("metadata object");
    obj.insert("compact_from".to_string(), serde_json::json!(now.to_rfc3339()));
    obj.insert("compact_requested_at".to_string(), serde_json::json!(now.to_rfc3339()));
    obj.insert("compact_in_progress".to_string(), serde_json::json!(true));
    let _updated = crate::shared::models::Agent::update(
        &state.db,
        &agent_name,
        crate::shared::models::UpdateAgentRequest { metadata: Some(meta.clone()), description: None, tags: None, idle_timeout_seconds: None, busy_timeout_seconds: None }
    ).await.map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update agent metadata: {}", e)))?;

    // Mark agent state busy (so UI reflects processing)
    if let Err(e) = crate::shared::models::Agent::update_agent_to_busy(&state.db, &agent_name).await {
        tracing::warn!("Failed to set agent busy for compaction: {}", e);
    }

    // Post a compact request message to guide the agent to summarize
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };
    let content = "Please compact the prior conversation into a concise summary capturing goals, decisions, constraints, and outputs. Return a single final message that will serve as the new baseline context going forward.".to_string();
    let req = CreateMessageRequest {
        role: crate::shared::models::constants::MESSAGE_ROLE_USER.to_string(),
        content,
        metadata: serde_json::json!({ "type": "compact_request" }),
        author_name: None,
        recipient: None,
        channel: None,
        content_type: None,
        content_json: None,
    };
    let m = AgentMessage::create(&state.db, &agent_name, created_by, req)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create compact request: {}", e)))?;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "compact_from": now.to_rfc3339(),
        "message_id": m.id,
    })))
}

async fn compute_context_usage(
    state: &AppState,
    agent_name: &str,
    candidate_user_text: Option<String>,
) -> Result<(usize, usize), anyhow::Error> {
    let max_tokens: usize = std::env::var("RAWORC_CONTEXT_MAX_TOKENS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100_000);

    // Fetch agent and messages
    let agent = crate::shared::models::Agent::find_by_name(&state.db, agent_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Agent not found"))?;
    let mut since_ts: Option<chrono::DateTime<chrono::Utc>> = None;
    if let Some(obj) = agent.metadata.as_object() {
        if let Some(ts) = obj.get("compact_from").and_then(|v| v.as_str()) {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                since_ts = Some(dt.with_timezone(&chrono::Utc));
            }
        }
    }

    let messages = crate::shared::models::message::AgentMessage::find_by_agent(
        &state.db,
        agent_name,
        Some(1000),
        Some(0),
    )
    .await?;

    let enc = load_harmony_encoding(HarmonyEncodingName::HarmonyGptOss)?;
    let mut hm: Vec<HMessage> = Vec::new();
    for msg in messages {
        if let Some(since) = since_ts {
            if msg.created_at < since { continue; }
        }
        match msg.role.as_str() {
            crate::shared::models::constants::MESSAGE_ROLE_USER => {
                hm.push(HMessage::from_role_and_content(HRole::User, msg.content.clone()));
            }
            crate::shared::models::constants::MESSAGE_ROLE_AGENT => {
                // If Harmony segments exist, include assistant final and tool results; else treat as final only
                if let Some(cj) = msg.content_json.as_ref() {
                    if let Some(segs) = cj.get("harmony").and_then(|h| h.get("segments")).and_then(|v| v.as_array()) {
                        for seg in segs {
                            let t = seg.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            match t {
                                "final" => {
                                    if let Some(txt) = seg.get("text").and_then(|v| v.as_str()) {
                                        hm.push(HMessage {
                                            author: HAuthor::new(HRole::Assistant, "assistant".to_string()),
                                            recipient: None,
                                            content: vec![HContent::from(txt.to_string())],
                                            channel: Some("final".to_string()),
                                            content_type: None,
                                        });
                                    }
                                }
                                "tool_result" => {
                                    let tool = seg.get("tool").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let output_val = seg.get("output");
                                    let output = match output_val {
                                        Some(serde_json::Value::String(s)) => s.clone(),
                                        Some(v) => serde_json::to_string(v).unwrap_or_default(),
                                        None => String::new(),
                                    };
                                    if !tool.is_empty() {
                                        hm.push(HMessage {
                                            author: HAuthor::new(HRole::Tool, tool),
                                            recipient: None,
                                            content: vec![HContent::from(output)],
                                            channel: None,
                                            content_type: None,
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }
                        continue;
                    }
                }
                hm.push(HMessage {
                    author: HAuthor::new(HRole::Assistant, "assistant".to_string()),
                    recipient: None,
                    content: vec![HContent::from(msg.content.clone())],
                    channel: Some("final".to_string()),
                    content_type: None,
                });
            }
            _ => {}
        }
    }

    if let Some(text) = candidate_user_text {
        hm.push(HMessage::from_role_and_content(HRole::User, text));
    }

    // Render
    let conv = openai_harmony::chat::Conversation::from_messages(hm);
    let toks = enc.render_conversation_for_completion(&conv, HRole::Assistant, None)?;
    Ok((toks.len(), max_tokens))
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
    let messages = AgentMessage::find_by_agent(&state.db, &agent_name, query.limit, query.offset)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch messages: {}", e)))?;

    // Convert to MessageResponse
    let response_messages: Vec<MessageResponse> = messages
        .into_iter()
        .map(|msg| MessageResponse {
            id: msg.id,
            agent_name: msg.agent_name,
            role: msg.role,
            author_name: msg.author_name,
            recipient: msg.recipient,
            channel: msg.channel,
            content: msg.content,
            content_type: msg.content_type,
            content_json: msg.content_json,
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

// Removed clear_messages endpoint: compaction uses a baseline only; no deletions.

pub async fn update_message(
    State(state): State<Arc<AppState>>,
    Path((agent_name, message_id)): Path<(String, String)>,
    Extension(_auth): Extension<AuthContext>,
    Json(req): Json<UpdateMessageRequest>,
) -> ApiResult<Json<MessageResponse>> {
    // Verify agent exists
    let _agent = crate::shared::models::Agent::find_by_name(&state.db, &agent_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Agent not found".to_string()))?;

    // Verify message belongs to agent
    let existing = AgentMessage::find_by_id(&state.db, &message_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Database error: {}", e)))?;
    let existing = existing.ok_or_else(|| ApiError::NotFound("Message not found".to_string()))?;
    if existing.agent_name != agent_name {
        return Err(ApiError::BadRequest("Message does not belong to agent".to_string()));
    }

    let updated = AgentMessage::update_by_id(&state.db, &message_id, req)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update message: {}", e)))?;

    Ok(Json(MessageResponse {
        id: updated.id,
        agent_name: updated.agent_name,
        role: updated.role,
        author_name: updated.author_name,
        recipient: updated.recipient,
        channel: updated.channel,
        content: updated.content,
        content_type: updated.content_type,
        content_json: updated.content_json,
        metadata: updated.metadata,
        created_at: updated.created_at.to_rfc3339(),
    }))
}
