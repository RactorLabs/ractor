use axum::{
    extract::{Path, State},
    Extension,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::shared::models::AppState;
use crate::shared::rbac::{RoleBinding, SubjectType};
use crate::server::rest::error::{ApiError, ApiResult};
use crate::server::rest::middleware::AuthContext;
use crate::server::rest::rbac_enforcement::{check_api_permission, permissions};

#[derive(Debug, Deserialize)]
pub struct CreateRoleBindingRequest {
    pub role_name: String,
    pub principal: String,
    pub principal_type: SubjectType,
}


#[derive(Debug, Serialize)]
pub struct RoleBindingResponse {
    pub id: String,
    pub role_name: String,
    pub principal: String,
    pub principal_type: SubjectType,
    pub created_at: String,
}


impl From<RoleBinding> for RoleBindingResponse {
    fn from(rb: RoleBinding) -> Self {
        Self {
            id: rb.id.map(|id| id.to_string()).unwrap_or_default(),
            role_name: rb.role_name,
            principal: rb.principal,
            principal_type: rb.principal_type,
            created_at: rb.created_at,
        }
    }
}

pub async fn list_role_bindings(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<RoleBindingResponse>>> {
    // Check permission
    check_api_permission(&auth, &state, &permissions::ROLE_BINDING_LIST)
        .await
        .map_err(|e| match e {
            axum::http::StatusCode::FORBIDDEN => ApiError::Forbidden("Insufficient permissions".to_string()),
            _ => ApiError::Internal(anyhow::anyhow!("Permission check failed")),
        })?;

    let bindings = state.get_all_role_bindings().await?;
    let response: Vec<RoleBindingResponse> = bindings.into_iter().map(Into::into).collect();
    Ok(Json(response))
}

pub async fn get_role_binding(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<RoleBindingResponse>> {
    // Check permission
    check_api_permission(&auth, &state, &permissions::ROLE_BINDING_GET)
        .await
        .map_err(|e| match e {
            axum::http::StatusCode::FORBIDDEN => ApiError::Forbidden("Insufficient permissions".to_string()),
            _ => ApiError::Internal(anyhow::anyhow!("Permission check failed")),
        })?;
    // Try to parse as UUID first, otherwise treat as name
    // For GET by ID, we need to search through all bindings since we only have compound keys
    let binding = state.get_all_role_bindings().await?
        .into_iter()
        .find(|rb| {
            if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
                rb.id == Some(uuid)
            } else {
                // Try matching as "role_name:principal" format
                format!("{}:{}", rb.role_name, rb.principal) == id
            }
        });
    
    let binding = binding.ok_or(ApiError::NotFound("Role binding not found".to_string()))?;
    Ok(Json(binding.into()))
}

pub async fn create_role_binding(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRoleBindingRequest>,
) -> ApiResult<Json<RoleBindingResponse>> {
    // Check permission
    check_api_permission(&auth, &state, &permissions::ROLE_BINDING_CREATE)
        .await
        .map_err(|e| match e {
            axum::http::StatusCode::FORBIDDEN => ApiError::Forbidden("Insufficient permissions".to_string()),
            _ => ApiError::Internal(anyhow::anyhow!("Permission check failed")),
        })?;
    let role_binding = RoleBinding {
        id: None,
        role_name: req.role_name,
        principal: req.principal,
        principal_type: req.principal_type,
        created_at: Utc::now().to_rfc3339(),
    };
    
    let created_binding = state.create_role_binding(&role_binding).await?;
    Ok(Json(created_binding.into()))
}

pub async fn delete_role_binding(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<()> {
    // Check permission
    check_api_permission(&auth, &state, &permissions::ROLE_BINDING_DELETE)
        .await
        .map_err(|e| match e {
            axum::http::StatusCode::FORBIDDEN => ApiError::Forbidden("Insufficient permissions".to_string()),
            _ => ApiError::Internal(anyhow::anyhow!("Permission check failed")),
        })?;
    // Parse the id as "role_name:principal" format
    let parts: Vec<&str> = id.split(':').collect();
    let deleted = if parts.len() == 2 {
        let role_name = parts[0];
        let principal = parts[1];
        state.delete_role_binding(role_name, principal).await?
    } else {
        // Try to find by UUID if it's in the old format
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            if let Some(binding) = state.get_all_role_bindings().await?
                .into_iter()
                .find(|rb| rb.id == Some(uuid)) {
                state.delete_role_binding(&binding.role_name, &binding.principal).await?
            } else {
                false
            }
        } else {
            false
        }
    };
    
    if !deleted {
        return Err(ApiError::NotFound("Role binding not found".to_string()));
    }
    
    Ok(())
}