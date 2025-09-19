use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::query;
use std::sync::Arc;
use chrono::{DateTime, Utc};

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::api::rest::rbac_enforcement::{check_api_permission, permissions};
use crate::shared::rbac::PermissionContext;
use crate::shared::models::{
    Agent, AppState, CreateAgentRequest, PublishAgentRequest, RemixAgentRequest,
    RestoreAgentRequest, UpdateAgentRequest, UpdateAgentStateRequest,
};

// Helper: determine if principal has admin-like privileges via RBAC (wildcard rule)
async fn is_admin_principal(auth: &AuthContext, state: &AppState) -> bool {
    let ctx = PermissionContext { api_group: "api".into(), resource: "*".into(), verb: "*".into() };
    match crate::api::auth::check_permission(&auth.principal, state, &ctx).await {
        Ok(true) => true,
        _ => false,
    }
}

#[derive(Debug, Serialize)]
pub struct AgentResponse {
    pub name: String, // Primary key - no more id field
    pub created_by: String,
    pub state: String,
    pub description: Option<String>,
    pub parent_agent_name: Option<String>, // Changed from parent_agent_id
    pub created_at: String,
    pub last_activity_at: Option<String>,
    pub metadata: serde_json::Value,
    pub tags: Vec<String>,
    pub is_published: bool,
    pub published_at: Option<String>,
    pub published_by: Option<String>,
    pub publish_permissions: serde_json::Value,
    pub idle_timeout_seconds: i32,
    pub busy_timeout_seconds: i32,
    pub idle_from: Option<String>,
    pub busy_from: Option<String>,
    // Removed: id, container_id, persistent_volume_id
}

#[derive(Debug, Deserialize)]
pub struct ListAgentsQuery {
    pub state: Option<String>,
}

impl AgentResponse {
    async fn from_agent(agent: Agent, _pool: &sqlx::MySqlPool) -> Result<Self, ApiError> {
        // Convert tags from JSON value to Vec<String>
        let tags: Vec<String> = match agent.tags {
            serde_json::Value::Array(arr) => arr
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            _ => Vec::new(),
        };
        Ok(Self {
            name: agent.name,
            created_by: agent.created_by,
            state: agent.state,
            description: agent.description,
            parent_agent_name: agent.parent_agent_name,
            created_at: agent.created_at.to_rfc3339(),
            last_activity_at: agent.last_activity_at.map(|dt| dt.to_rfc3339()),
            metadata: agent.metadata,
            tags,
            is_published: agent.is_published,
            published_at: agent.published_at.map(|dt| dt.to_rfc3339()),
            published_by: agent.published_by,
            publish_permissions: agent.publish_permissions,
            idle_timeout_seconds: agent.idle_timeout_seconds,
            busy_timeout_seconds: agent.busy_timeout_seconds,
            idle_from: agent.idle_from.map(|dt| dt.to_rfc3339()),
            busy_from: agent.busy_from.map(|dt| dt.to_rfc3339()),
        })
    }
}

// Helper function to find agent by name
async fn find_agent_by_name(
    state: &AppState,
    name: &str,
    created_by: &str,
    is_admin: bool,
) -> Result<Agent, ApiError> {
    // Try to find by name directly (names are globally unique)
    if let Some(agent) = Agent::find_by_name(&state.db, name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch agent: {}", e)))?
    {
        // Admins can access any agent, regular users only their own or published agents
        if is_admin || agent.created_by == created_by || agent.is_published {
            return Ok(agent);
        } else {
            return Err(ApiError::Forbidden(
                "Access denied to this agent".to_string(),
            ));
        }
    }

    Err(ApiError::NotFound("Agent not found".to_string()))
}

pub async fn list_agents(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListAgentsQuery>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<Vec<AgentResponse>>> {
    // Admins require explicit permission; non-admins can list only their own agents
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::AGENT_LIST)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions to list agents".to_string()))?;
    }

    let mut agents = Agent::find_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to list agents: {}", e)))?;

    // For non-admin users, only show their own agents
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Only admin operator can see all agents

    // For regular users, only show their own agents
    // Admins can see all agents
    if !is_admin {
        agents.retain(|s| s.created_by == *username);
    }

    // Filter by state if provided
    if let Some(state_filter) = query.state {
        agents.retain(|s| s.state == state_filter);
    }

    let mut response = Vec::new();
    for agent in agents {
        response.push(AgentResponse::from_agent(agent, &state.db).await?);
    }

    Ok(Json(response))
}

pub async fn get_agent(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<AgentResponse>> {
    // Admins require explicit permission; non-admins can access only their own agent
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::AGENT_GET)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions to get agent".to_string()))?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find agent by name (admin can access any agent)
    let agent = find_agent_by_name(&state, &name, username, is_admin).await?;

    Ok(Json(AgentResponse::from_agent(agent, &state.db).await?))
}

pub async fn create_agent(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateAgentRequest>,
) -> ApiResult<Json<AgentResponse>> {
    tracing::info!(
        "Creating agent with secrets: {} keys, instructions: {}, setup: {}, prompt: {}",
        req.secrets.len(),
        req.instructions.is_some(),
        req.setup.is_some(),
        req.prompt.is_some()
    );

    // Admins require explicit permission; non-admins can create their own agents
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::AGENT_CREATE)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions to create agent".to_string()))?;
    }

    // Get the principal name
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    let agent = Agent::create(&state.db, req.clone(), created_by)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create agent: {:?}", e);

            // Check for unique constraint violation on agent name
            if let sqlx::Error::Database(db_err) = &e {
                tracing::error!(
                    "Database error - code: {:?}, message: '{}'",
                    db_err.code(),
                    db_err.message()
                );
                if let Some(code) = db_err.code() {
                    // Simplify the condition to catch the specific error
                    if code == "23000" || code == "1062" {
                        let name_display = &req.name;
                        tracing::info!(
                            "Detected database constraint violation for agent {}",
                            name_display
                        );
                        if db_err.message().contains("agents.PRIMARY")
                            || db_err.message().contains("unique_agent_name")
                            || db_err.message().contains("Duplicate entry")
                        {
                            tracing::info!("Confirmed duplicate agent name constraint violation");
                            return ApiError::Conflict(format!(
                                "Agent name '{}' already exists. Choose a different name.",
                                name_display
                            ));
                        }
                    }
                }
            }

            ApiError::Internal(anyhow::anyhow!("Failed to create agent: {}", e))
        })?;

    // Add task to queue for agent manager to create container with agent parameters
    let payload = serde_json::json!({
        "secrets": req.secrets,
        "instructions": req.instructions,
        "setup": req.setup,
        "prompt": req.prompt,
        "principal": created_by,
        "principal_type": match &auth.principal {
            crate::shared::rbac::AuthPrincipal::Subject(_) => "User",
            crate::shared::rbac::AuthPrincipal::Operator(_) => "Admin",
        },
        "user_token": auth.token
    });

    sqlx::query(
        r#"
        INSERT INTO agent_tasks (agent_name, task_type, created_by, payload, status)
        VALUES (?, 'create_agent', ?, ?, 'pending')
        "#,
    )
    .bind(&agent.name)
    .bind(created_by)
    .bind(payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create agent task: {}", e)))?;

    tracing::info!("Created agent task for agent {}", agent.name);

    Ok(Json(AgentResponse::from_agent(agent, &state.db).await?))
}

pub async fn remix_agent(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<RemixAgentRequest>,
) -> ApiResult<Json<AgentResponse>> {
    // Admins require explicit permission; non-admins can remix according to publish/ownership checks
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::AGENT_CREATE)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions to remix agent".to_string()))?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find parent agent by ID or name (admin can remix any agent, users can remix published agents)
    let is_admin = is_admin_principal(&auth, &state).await;
    let parent = find_agent_by_name(&state, &name, username, true).await?; // Allow finding any agent for remix (permission check below)

    // Check remix permissions for non-owners
    if parent.created_by != *username && !is_admin {
        // Non-owner, non-admin can only remix if agent is published
        if !parent.is_published {
            return Err(ApiError::Forbidden(
                "You can only remix your own agents or published agents".to_string(),
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
                "Code remix not permitted for this published agent".to_string(),
            ));
        }
        if req.secrets
            && !publish_perms
                .get("secrets")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        {
            return Err(ApiError::Forbidden(
                "Secrets remix not permitted for this published agent".to_string(),
            ));
        }
        // Content is always allowed - no permission check needed
    }

    // Get the principal name for task creation (remixer becomes owner)
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Store the remix options before moving req into Agent::remix
    let copy_code = req.code;
    let copy_secrets = req.secrets;
    // Content is always copied
    let copy_content = true;
    let initial_prompt = req.prompt.clone();

    let agent = Agent::remix(&state.db, &parent.name, req.clone(), created_by)
        .await
        .map_err(|e| {
            // Provide a clearer error on duplicate name conflicts
            if let sqlx::Error::Database(db_err) = &e {
                if let Some(code) = db_err.code() {
                    // MySQL duplicate/constraint codes: 23000 (SQLSTATE), 1062 (ER_DUP_ENTRY)
                    if code == "23000" || code == "1062" {
                        if db_err.message().contains("agents.PRIMARY")
                            || db_err.message().contains("unique_agent_name")
                            || db_err.message().contains("Duplicate entry")
                        {
                            return ApiError::Conflict(format!(
                                "Agent name '{}' already exists. Choose a different name.",
                                req.name
                            ));
                        }
                    }
                }
            }
            ApiError::Internal(anyhow::anyhow!("Failed to remix agent: {}", e))
        })?;

    // Add task to queue for agent manager to create container with remix options
    let task_payload = serde_json::json!({
        "remix": true,
        "parent_agent_name": parent.name,
        "copy_code": copy_code,
        "copy_secrets": copy_secrets,
        "copy_content": copy_content,
        "prompt": initial_prompt,
        "principal": created_by,
        "principal_type": match &auth.principal {
            crate::shared::rbac::AuthPrincipal::Subject(_) => "User",
            crate::shared::rbac::AuthPrincipal::Operator(_) => "Admin",
        }
    });

    sqlx::query(
        r#"
        INSERT INTO agent_tasks (agent_name, task_type, created_by, payload, status)
        VALUES (?, 'create_agent', ?, ?, 'pending')
        "#,
    )
    .bind(&agent.name)
    .bind(created_by)
    .bind(task_payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create agent task: {}", e)))?;

    tracing::info!("Created agent task for remixed agent {}", agent.name);

    Ok(Json(AgentResponse::from_agent(agent, &state.db).await?))
}

#[derive(Debug, Deserialize)]
pub struct SleepAgentRequest {
    #[serde(default)]
    pub delay_seconds: Option<u64>,
    #[serde(default)]
    pub note: Option<String>,
}

pub async fn sleep_agent(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    maybe_req: Option<Json<SleepAgentRequest>>,
) -> ApiResult<Json<AgentResponse>> {
    tracing::info!("Sleep request received for agent: {}", name);
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find agent by ID or name (admin can sleep any agent)
    let is_admin = is_admin_principal(&auth, &state).await;
    let agent = find_agent_by_name(&state, &name, created_by, is_admin).await?;

    tracing::info!("Found agent in state: {}", agent.state);

    // Check permission for updating agents (admin only). Owners can sleep without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::AGENT_UPDATE)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions to sleep agent".to_string()))?;
    }

    // Allow sleeping own agents or admin can sleep any agent
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    let is_admin = is_admin_principal(&auth, &state).await;
    if !is_admin && agent.created_by != *username {
        return Err(ApiError::Forbidden(
            "Can only sleep your own agents".to_string(),
        ));
    }
    tracing::info!("Permission check passed");

    // Check current state - cannot sleep if already sleeping

    if agent.state == crate::shared::models::constants::AGENT_STATE_SLEPT {
        return Err(ApiError::BadRequest(
            "Agent is already sleeping".to_string(),
        ));
    }

    // Determine delay (min 5 seconds)
    // Try to parse JSON body; if absent or invalid, default to 5
    let mut delay_seconds = maybe_req
        .as_ref()
        .and_then(|r| r.delay_seconds)
        .unwrap_or(5);
    if delay_seconds < 5 { delay_seconds = 5; }
    // Add task to destroy the container but keep volume after delay
    let note = maybe_req
        .as_ref()
        .and_then(|r| r.note.clone())
        .and_then(|s| { let t = s.trim().to_string(); if t.is_empty() { None } else { Some(t) } });
    let payload = if let Some(n) = note {
        serde_json::json!({ "delay_seconds": delay_seconds, "note": n })
    } else {
        serde_json::json!({ "delay_seconds": delay_seconds })
    };
    sqlx::query(
        r#"
        INSERT INTO agent_tasks (agent_name, task_type, created_by, payload, status)
        VALUES (?, 'sleep_agent', ?, ?, 'pending')
        "#,
    )
    .bind(&agent.name)
    .bind(&created_by)
    .bind(payload)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create suspend task: {:?}", e);
        ApiError::Internal(anyhow::anyhow!("Failed to create suspend task: {}", e))
    })?;

    tracing::info!("Created suspend task for agent {}", name);

    // Fetch agent (state remains as-is until controller executes sleep)
    let updated_agent = Agent::find_by_name(&state.db, &name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch updated agent: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Agent not found".to_string()))?;

    Ok(Json(
        AgentResponse::from_agent(updated_agent, &state.db).await?,
    ))
}

pub async fn wake_agent(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<RestoreAgentRequest>,
) -> ApiResult<Json<AgentResponse>> {
    // Check permission for updating agents (admin only). Owners can wake without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::AGENT_UPDATE)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions to wake agent".to_string()))?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find agent by ID or name (admin can find any agent, but restore has ownership restrictions)
    let is_admin = is_admin_principal(&auth, &state).await;
    let agent = find_agent_by_name(&state, &name, username, is_admin).await?;

    // Check ownership: Even admins cannot wake other users' agents (only remix)
    if agent.created_by != *username {
        if is_admin {
            return Err(ApiError::Forbidden(
                "Admins cannot wake other users' agents. Use remix instead.".to_string(),
            ));
        } else {
            return Err(ApiError::Forbidden(
                "You can only wake your own agents.".to_string(),
            ));
        }
    }

    // Check current state - can only wake if sleeping
    if agent.state != crate::shared::models::constants::AGENT_STATE_SLEPT {
        return Err(ApiError::BadRequest(format!(
            "Cannot wake agent in {} state - only sleeping agents can be woken",
            agent.state
        )));
    }

    // Update agent state to INIT and bump activity timestamp.
    // Guard on current state to avoid races between check and update.
    let result = query(
        r#"
        UPDATE agents 
        SET state = ?, last_activity_at = CURRENT_TIMESTAMP
        WHERE name = ? AND state = ?
        "#,
    )
    .bind(crate::shared::models::constants::AGENT_STATE_INIT)
    .bind(&agent.name)
    .bind(crate::shared::models::constants::AGENT_STATE_SLEPT)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to wake agent: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("Agent not found".to_string()));
    }

    // Get the principal name for task creation
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Add task to restart the container with optional prompt
    let restore_payload = serde_json::json!({
        "prompt": req.prompt,
        "reason": "user_wake"
    });

    sqlx::query(
        r#"
        INSERT INTO agent_tasks (agent_name, task_type, created_by, payload, status)
        VALUES (?, 'wake_agent', ?, ?, 'pending')
        "#,
    )
    .bind(&agent.name)
    .bind(username)
    .bind(&restore_payload)
    .execute(&*state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create resume task: {:?}", e);
        ApiError::Internal(anyhow::anyhow!("Failed to create resume task: {}", e))
    })?;

    tracing::info!("Created resume task for agent {}", agent.name);

    // Fetch updated agent
    let updated_agent = Agent::find_by_name(&state.db, &agent.name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch updated agent: {}", e)))?
        .ok_or(ApiError::NotFound("Agent not found".to_string()))?;

    Ok(Json(
        AgentResponse::from_agent(updated_agent, &state.db).await?,
    ))
}

pub async fn update_agent(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<UpdateAgentRequest>,
) -> ApiResult<Json<AgentResponse>> {
    // Admins require explicit permission; owners can update without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::AGENT_UPDATE)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions to update agent".to_string()))?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find agent by ID or name (admin can access any agent for update/delete)
    let is_admin = is_admin_principal(&auth, &state).await;
    let agent = find_agent_by_name(&state, &name, username, is_admin).await?;
    // Enforce ownership: only admin or owner may update
    if !is_admin && agent.created_by != *username {
        return Err(ApiError::Forbidden(
            "You can only update your own agents".to_string(),
        ));
    }

    let updated_agent = Agent::update(&state.db, &agent.name, req)
        .await
        .map_err(|e| {
            let error_msg = e.to_string();
            if error_msg.contains("No fields to update") {
                ApiError::BadRequest(error_msg)
            } else if error_msg.contains("unique_agent_name")
                || error_msg.contains("Duplicate entry")
            {
                ApiError::BadRequest("A agent with this name already exists".to_string())
            } else {
                ApiError::Internal(anyhow::anyhow!("Failed to update agent: {}", e))
            }
        })?
        .ok_or(ApiError::NotFound("Agent not found".to_string()))?;

    Ok(Json(
        AgentResponse::from_agent(updated_agent, &state.db).await?,
    ))
}

pub async fn update_agent_state(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<UpdateAgentStateRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // Get agent and verify ownership (same pattern as other agent endpoints)
    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find agent by ID or name (admin can access any agent for update/delete)
    let is_admin = is_admin_principal(&auth, &state).await;
    let agent = find_agent_by_name(&state, &name, username, is_admin).await?;

    // Update the state with ownership verification
    let result = sqlx::query(
        "UPDATE agents SET state = ?, last_activity_at = CURRENT_TIMESTAMP WHERE name = ? AND created_by = ?"
    )
    .bind(&req.state)
    .bind(&agent.name)
    .bind(username)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update agent state: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound(
            "Agent not found or access denied".to_string(),
        ));
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "state": req.state
    })))
}

pub async fn delete_agent(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<()> {
    // Check permission for deleting agents (admin only). Owners can delete without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::AGENT_DELETE)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions to delete agent".to_string()))?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find agent by ID or name (admin can access any agent for update/delete)
    let is_admin = is_admin_principal(&auth, &state).await;
    let agent = find_agent_by_name(&state, &name, username, is_admin).await?;

    // Hard delete: schedule unpublish and container+volume removal, then remove DB row
    // Queue unpublish to remove any public content
    sqlx::query(
        r#"
        INSERT INTO agent_tasks (agent_name, task_type, created_by, payload, status)
        VALUES (?, 'unpublish_agent', ?, '{}', 'pending')
        "#,
    )
    .bind(&agent.name)
    .bind(username)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create unpublish task: {}", e)))?;

    // Add task to queue for agent manager to destroy container and cleanup volume
    sqlx::query(
        r#"
        INSERT INTO agent_tasks (agent_name, task_type, created_by, payload, status)
        VALUES (?, 'destroy_agent', ?, '{}', 'pending')
        "#,
    )
    .bind(&agent.name)
    .bind(username)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create destroy task: {}", e)))?;

    tracing::info!("Created destroy task for agent {}", agent.name);

    let deleted = Agent::delete(&state.db, &agent.name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to delete agent: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("Agent not found".to_string()));
    }

    Ok(())
}

pub async fn publish_agent(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<PublishAgentRequest>,
) -> ApiResult<Json<AgentResponse>> {
    // Check permission for updating agents (admin only). Owners can publish without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::AGENT_UPDATE)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions to publish agent".to_string()))?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find agent by ID or name (admin can publish any agent)
    let is_admin = is_admin_principal(&auth, &state).await;
    let agent = find_agent_by_name(&state, &name, username, is_admin).await?;

    // Check ownership (only owner or admin can publish)
    if !is_admin && agent.created_by != *username {
        return Err(ApiError::Forbidden(
            "You can only publish your own agents".to_string(),
        ));
    }

    // Publish the agent
    let published_agent = Agent::publish(&state.db, &agent.name, username, req.clone())
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to publish agent: {}", e)))?
        .ok_or(ApiError::NotFound("Agent not found".to_string()))?;

    // Create task to copy content files to public directory
    let payload = serde_json::json!({
        "content": req.content, // Content is always included in v0.4.0
        "code": req.code,
        "secrets": req.secrets
    });

    sqlx::query(
        r#"
        INSERT INTO agent_tasks (agent_name, task_type, created_by, payload, status)
        VALUES (?, 'publish_agent', ?, ?, 'pending')
        "#,
    )
    .bind(&agent.name)
    .bind(username)
    .bind(payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create publish task: {}", e)))?;

    tracing::info!("Created publish task for agent {}", agent.name);

    Ok(Json(
        AgentResponse::from_agent(published_agent, &state.db).await?,
    ))
}

pub async fn unpublish_agent(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<AgentResponse>> {
    // Check permission for updating agents (admin only). Owners can unpublish without RBAC grant
    if is_admin_principal(&auth, &state).await {
        check_api_permission(&auth, &state, &permissions::AGENT_UPDATE)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions to unpublish agent".to_string()))?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find agent by ID or name (admin can unpublish any agent)
    let is_admin = is_admin_principal(&auth, &state).await;
    let agent = find_agent_by_name(&state, &name, username, is_admin).await?;

    // Check ownership (only owner or admin can unpublish)
    if !is_admin && agent.created_by != *username {
        return Err(ApiError::Forbidden(
            "You can only unpublish your own agents".to_string(),
        ));
    }

    // Unpublish the agent
    let unpublished_agent = Agent::unpublish(&state.db, &agent.name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to unpublish agent: {}", e)))?
        .ok_or(ApiError::NotFound("Agent not found".to_string()))?;

    // Create task to remove content files from public directory
    let payload = serde_json::json!({});

    sqlx::query(
        r#"
        INSERT INTO agent_tasks (agent_name, task_type, created_by, payload, status)
        VALUES (?, 'unpublish_agent', ?, ?, 'pending')
        "#,
    )
    .bind(&agent.name)
    .bind(username)
    .bind(payload)
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create unpublish task: {}", e)))?;

    tracing::info!("Created unpublish task for agent {}", agent.name);

    Ok(Json(
        AgentResponse::from_agent(unpublished_agent, &state.db).await?,
    ))
}

pub async fn list_published_agents(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<AgentResponse>>> {
    // No authentication required for listing published agents (public access)

    let agents = Agent::find_published(&state.db).await.map_err(|e| {
        ApiError::Internal(anyhow::anyhow!("Failed to list published agents: {}", e))
    })?;

    let mut response = Vec::new();
    for agent in agents {
        response.push(AgentResponse::from_agent(agent, &state.db).await?);
    }

    Ok(Json(response))
}

pub async fn get_published_agent(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> ApiResult<Json<AgentResponse>> {
    // No authentication required for getting published agents (public access)

    let agent = Agent::find_by_name(&state.db, &name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch agent: {}", e)))?
        .ok_or(ApiError::NotFound("Agent not found".to_string()))?;

    // Check if agent is published
    if !agent.is_published {
        return Err(ApiError::NotFound(
            "Agent not found or not published".to_string(),
        ));
    }

    Ok(Json(AgentResponse::from_agent(agent, &state.db).await?))
}

// GET /agents/{name}/runtime — total runtime across sessions
pub async fn get_agent_runtime(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Permission: owner or admin
    let is_admin = is_admin_principal(&auth, &state).await;
    if is_admin {
        check_api_permission(&auth, &state, &permissions::AGENT_GET)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions to get agent runtime".to_string()))?;
    }

    // Get username for ownership check
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find agent (admin can access any agent)
    let agent = find_agent_by_name(&state, &name, username, is_admin).await?;

    // Fetch all responses for this agent (created_at + output JSON)
    let rows: Vec<(DateTime<Utc>, serde_json::Value)> = sqlx::query_as(
        r#"SELECT created_at, output FROM agent_responses WHERE agent_name = ? ORDER BY created_at ASC"#
    )
    .bind(&agent.name)
    .fetch_all(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch responses: {}", e)))?;

    // Sum runtime
    let mut total: i64 = 0;
    let mut last_woke: Option<DateTime<Utc>> = None;
    for (row_created_at, output) in rows.into_iter() {
        if let Some(items) = output.get("items").and_then(|v| v.as_array()) {
            for it in items {
                let t = it.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if t == "woke" {
                    let at = it
                        .get("at")
                        .and_then(|v| v.as_str())
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or(row_created_at);
                    last_woke = Some(at);
                } else if t == "slept" {
                    // Prefer embedded runtime_seconds, else compute delta
                    if let Some(rs) = it.get("runtime_seconds").and_then(|v| v.as_i64()) {
                        if rs > 0 { total += rs; }
                    } else {
                        let end_at = it
                            .get("at")
                            .and_then(|v| v.as_str())
                            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or(row_created_at);
                        let start_at = last_woke.unwrap_or(agent.created_at);
                        let delta = (end_at - start_at).num_seconds();
                        if delta > 0 { total += delta; }
                    }
                }
            }
        }
    }

    Ok(Json(serde_json::json!({
        "agent_name": agent.name,
        "total_runtime_seconds": total,
    })))
}

pub async fn update_agent_to_busy(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Only the agent container should be able to call this
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find agent (agent token should match agent ownership)
    let is_admin = is_admin_principal(&auth, &state).await;
    let agent = find_agent_by_name(&state, &name, username, is_admin).await?;

    // Update agent to busy: clear idle_from and set busy_from (strict busy timeout)
    Agent::update_agent_to_busy(&state.db, &agent.name)
        .await
        .map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("Failed to update agent to busy: {}", e))
        })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "state": "busy",
        "timeout_status": "paused"
    })))
}

pub async fn update_agent_to_idle(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Only the agent container should be able to call this
    let username = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
    };

    // Find agent (agent token should match agent ownership)
    let is_admin = is_admin_principal(&auth, &state).await;
    let agent = find_agent_by_name(&state, &name, username, is_admin).await?;

    // Update agent to idle: set idle_from and clear busy_from (idle timeout active)
    Agent::update_agent_to_idle(&state.db, &agent.name)
        .await
        .map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("Failed to update agent to idle: {}", e))
        })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "state": "idle",
        "timeout_status": "active"
    })))
}
