use crate::server::rest::error::ApiError;
use crate::shared::models::{AppState, Agent, CreateAgentRequest, UpdateAgentRequest, AgentStatusUpdate};
use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Json, Result as AxumResult},
};
use tracing::{info, warn, error};



/// List agents in a specific space
pub async fn list_space_agents(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
) -> AxumResult<Json<Vec<Agent>>> {
    info!("Listing agents for space: {}", space);

    match Agent::find_all(&state.db, Some(&space)).await {
        Ok(agents) => {
            info!("Found {} agents in space {}", agents.len(), space);
            Ok(Json(agents))
        }
        Err(e) => {
            error!("Failed to list agents in space {}: {}", space, e);
            Err(ApiError::Internal(anyhow::anyhow!("Failed to list agents: {}", e)).into())
        }
    }
}

/// Get a specific agent by space and name
pub async fn get_agent(
    State(state): State<Arc<AppState>>,
    Path((space, name)): Path<(String, String)>,
) -> AxumResult<Json<Agent>> {
    info!("Getting agent {} in space {}", name, space);

    match Agent::find_by_name(&state.db, &space, &name).await {
        Ok(Some(agent)) => {
            info!("Found agent: {}", name);
            Ok(Json(agent))
        }
        Ok(None) => {
            warn!("Agent {} not found in space {}", name, space);
            Err(ApiError::NotFound(format!("Agent '{}' not found in space '{}'", name, space)).into())
        }
        Err(e) => {
            error!("Failed to get agent {}: {}", name, e);
            Err(ApiError::Internal(anyhow::anyhow!("Failed to get agent: {}", e)).into())
        }
    }
}

/// Create a new agent
pub async fn create_agent(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
    Json(req): Json<CreateAgentRequest>,
) -> AxumResult<(StatusCode, Json<Agent>)> {
    info!("Creating agent {} in space {}", req.name, space);

    // TODO: Get actual user from JWT
    let created_by = "system";

    match Agent::create(&state.db, &space, req, created_by).await {
        Ok(agent) => {
            info!("Agent {} created successfully in space {}", agent.name, agent.space);
            Ok((StatusCode::CREATED, Json(agent)))
        }
        Err(sqlx::Error::Protocol(msg)) if msg.contains("already exists") => {
            warn!("Agent creation failed - already exists: {}", msg);
            Err(ApiError::Conflict(msg).into())
        }
        Err(sqlx::Error::Protocol(msg)) if msg.contains("Port") && msg.contains("already in use") => {
            warn!("Agent creation failed - port in use: {}", msg);
            Err(ApiError::Conflict(msg).into())
        }
        Err(e) => {
            error!("Failed to create agent: {}", e);
            Err(ApiError::Internal(anyhow::anyhow!("Failed to create agent: {}", e)).into())
        }
    }
}

/// Update an existing agent
pub async fn update_agent(
    State(state): State<Arc<AppState>>,
    Path((space, name)): Path<(String, String)>,
    Json(req): Json<UpdateAgentRequest>,
) -> AxumResult<Json<Agent>> {
    info!("Updating agent {} in space {}", name, space);

    match Agent::update(&state.db, &space, &name, req).await {
        Ok(Some(agent)) => {
            info!("Agent {} updated successfully", name);
            Ok(Json(agent))
        }
        Ok(None) => {
            warn!("Agent {} not found for update in space {}", name, space);
            Err(ApiError::NotFound(format!("Agent '{}' not found in space '{}'", name, space)).into())
        }
        Err(e) => {
            error!("Failed to update agent {}: {}", name, e);
            Err(ApiError::Internal(anyhow::anyhow!("Failed to update agent: {}", e)).into())
        }
    }
}

/// Update agent status (used by operator for deployment status)
pub async fn update_agent_status(
    State(state): State<Arc<AppState>>,
    Path((space, name)): Path<(String, String)>,
    Json(status_update): Json<AgentStatusUpdate>,
) -> AxumResult<Json<Agent>> {
    info!("Updating status for agent {} in space {} to {}", name, space, status_update.status);

    match Agent::update_status(&state.db, &space, &name, status_update).await {
        Ok(Some(agent)) => {
            info!("Agent {} status updated successfully", name);
            Ok(Json(agent))
        }
        Ok(None) => {
            warn!("Agent {} not found for status update in space {}", name, space);
            Err(ApiError::NotFound(format!("Agent '{}' not found in space '{}'", name, space)).into())
        }
        Err(e) => {
            error!("Failed to update agent {} status: {}", name, e);
            Err(ApiError::Internal(anyhow::anyhow!("Failed to update agent status: {}", e)).into())
        }
    }
}

/// Delete an agent
pub async fn delete_agent(
    State(state): State<Arc<AppState>>,
    Path((space, name)): Path<(String, String)>,
) -> AxumResult<StatusCode> {
    info!("Deleting agent {} in space {}", name, space);

    match Agent::delete(&state.db, &space, &name).await {
        Ok(true) => {
            info!("Agent {} deleted successfully", name);
            Ok(StatusCode::NO_CONTENT)
        }
        Ok(false) => {
            warn!("Agent {} not found for deletion in space {}", name, space);
            Err(ApiError::NotFound(format!("Agent '{}' not found in space '{}'", name, space)).into())
        }
        Err(e) => {
            error!("Failed to delete agent {}: {}", name, e);
            Err(ApiError::Internal(anyhow::anyhow!("Failed to delete agent: {}", e)).into())
        }
    }
}

/// List running agents in a space
pub async fn list_running_agents(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
) -> AxumResult<Json<Vec<Agent>>> {
    info!("Listing running agents in space {}", space);

    match Agent::find_running_agents(&state.db, &space).await {
        Ok(agents) => {
            info!("Found {} running agents in space {}", agents.len(), space);
            Ok(Json(agents))
        }
        Err(e) => {
            error!("Failed to list running agents: {}", e);
            Err(ApiError::Internal(anyhow::anyhow!("Failed to list running agents: {}", e)).into())
        }
    }
}

/// Deploy an agent (build and start)
pub async fn deploy_agent(
    State(state): State<Arc<AppState>>,
    Path((space, name)): Path<(String, String)>,
) -> AxumResult<Json<Agent>> {
    info!("Deploying agent {} in space {}", name, space);

    // Get the agent first
    let _agent = match Agent::find_by_name(&state.db, &space, &name).await {
        Ok(Some(agent)) => agent,
        Ok(None) => {
            warn!("Agent {} not found for deployment in space {}", name, space);
            return Err(ApiError::NotFound(format!("Agent '{}' not found in space '{}'", name, space)).into());
        }
        Err(e) => {
            error!("Failed to get agent {} for deployment: {}", name, e);
            return Err(ApiError::Internal(anyhow::anyhow!("Failed to get agent: {}", e)).into());
        }
    };

    // Update status to building
    let status_update = AgentStatusUpdate {
        status: "building".to_string(),
    };

    match Agent::update_status(&state.db, &space, &name, status_update).await {
        Ok(Some(updated_agent)) => {
            info!("Agent {} deployment initiated", name);
            
            // TODO: Send deployment task to operator via message queue
            // For now, just return the agent with building status
            
            Ok(Json(updated_agent))
        }
        Ok(None) => {
            warn!("Agent {} disappeared during deployment", name);
            Err(ApiError::NotFound(format!("Agent '{}' not found", name)).into())
        }
        Err(e) => {
            error!("Failed to update agent {} status for deployment: {}", name, e);
            Err(ApiError::Internal(anyhow::anyhow!("Failed to initiate deployment: {}", e)).into())
        }
    }
}

/// Stop an agent
pub async fn stop_agent(
    State(state): State<Arc<AppState>>,
    Path((space, name)): Path<(String, String)>,
) -> AxumResult<Json<Agent>> {
    info!("Stopping agent {} in space {}", name, space);

    // Update status to stopped
    let status_update = AgentStatusUpdate {
        status: "stopped".to_string(),
    };

    match Agent::update_status(&state.db, &space, &name, status_update).await {
        Ok(Some(updated_agent)) => {
            info!("Agent {} stop initiated", name);
            
            // TODO: Send stop task to operator via message queue
            
            Ok(Json(updated_agent))
        }
        Ok(None) => {
            warn!("Agent {} not found for stopping in space {}", name, space);
            Err(ApiError::NotFound(format!("Agent '{}' not found in space '{}'", name, space)).into())
        }
        Err(e) => {
            error!("Failed to stop agent {}: {}", name, e);
            Err(ApiError::Internal(anyhow::anyhow!("Failed to stop agent: {}", e)).into())
        }
    }
}