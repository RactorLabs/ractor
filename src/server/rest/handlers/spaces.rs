use axum::{
    extract::{Extension, Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::shared::models::{AppState, Space};
use crate::server::rest::error::{ApiError, ApiResult};
use crate::server::rest::middleware::AuthContext;
use crate::server::rest::rbac_enforcement::{check_api_permission, permissions};

#[derive(Debug, Serialize)]
pub struct SpaceResponse {
    pub name: String,
    pub description: Option<String>,
    pub settings: serde_json::Value,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: String,
}

impl From<Space> for SpaceResponse {
    fn from(space: Space) -> Self {
        Self {
            name: space.name,
            description: space.description,
            settings: space.settings,
            active: space.active,
            created_at: space.created_at,
            updated_at: space.updated_at,
            created_by: space.created_by,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateSpaceRequest {
    pub name: String,
    pub description: Option<String>,
    pub settings: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSpaceRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub settings: Option<serde_json::Value>,
    pub active: Option<bool>,
}

/// List all spaces
pub async fn list_spaces(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<Vec<SpaceResponse>>> {
    // Check permission for listing spaces
    check_api_permission(&auth, &state, &permissions::SPACE_LIST, None)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to list spaces".to_string()))?;

    let spaces = state.get_all_spaces()
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch spaces: {}", e)))?;

    let response: Vec<SpaceResponse> = spaces.into_iter().map(SpaceResponse::from).collect();
    Ok(Json(response))
}

/// Get a specific space
pub async fn get_space(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SpaceResponse>> {
    // Check permission for reading spaces
    check_api_permission(&auth, &state, &permissions::SPACE_GET, None)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to get space".to_string()))?;

    let space = state.get_space(&space)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch space: {}", e)))?
        .ok_or(ApiError::NotFound("Space not found".to_string()))?;

    Ok(Json(SpaceResponse::from(space)))
}

/// Create a new space
pub async fn create_space(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateSpaceRequest>,
) -> ApiResult<Json<SpaceResponse>> {
    // Check permission for creating spaces
    check_api_permission(&auth, &state, &permissions::SPACE_CREATE, None)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to create space".to_string()))?;

    // Validate space name format
    if !req.name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.') {
        return Err(ApiError::BadRequest("Space name must contain only alphanumeric characters, underscores, hyphens, and dots".to_string()));
    }

    // Get the principal name
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
    };

    let space = state.create_space(
        &req.name,
        req.description.as_deref(),
        req.settings.unwrap_or_else(|| serde_json::json!({})),
        created_by,
    )
    .await
    .map_err(|e| match e {
        crate::shared::models::DatabaseError::Unique(_) => {
            ApiError::Conflict("A space with this name already exists".to_string())
        }
        _ => ApiError::Internal(anyhow::anyhow!("Failed to create space: {}", e))
    })?;

    Ok(Json(SpaceResponse::from(space)))
}

/// Update a space
pub async fn update_space(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<UpdateSpaceRequest>,
) -> ApiResult<Json<SpaceResponse>> {
    // Check permission for updating spaces
    check_api_permission(&auth, &state, &permissions::SPACE_UPDATE, None)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to update space".to_string()))?;

    // Check if space exists
    let _space = state.get_space(&space)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch space: {}", e)))?
        .ok_or(ApiError::NotFound("Space not found".to_string()))?;

    let updated = state.update_space(
        &space,
        req.name.as_deref(),
        req.description.as_deref(),
        req.settings,
        req.active,
    )
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update space: {}", e)))?;

    if !updated {
        return Err(ApiError::NotFound("Space not found".to_string()));
    }

    // Fetch the updated space
    let space = state.get_space(&space)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch updated space: {}", e)))?
        .ok_or(ApiError::Internal(anyhow::anyhow!("Space disappeared after update")))?;

    Ok(Json(SpaceResponse::from(space)))
}

/// Delete a space
pub async fn delete_space(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<()> {
    // Check permission for deleting spaces
    check_api_permission(&auth, &state, &permissions::SPACE_DELETE, None)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to delete space".to_string()))?;

    // Prevent deletion of default space
    if space == "default" {
        return Err(ApiError::BadRequest("Cannot delete the default space".to_string()));
    }

    let deleted = state.delete_space(&space)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to delete space: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("Space not found".to_string()));
    }

    Ok(())
}