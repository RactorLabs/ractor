use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::shared::models::AppState;
use crate::server::rest::error::{ApiError, ApiResult};
use crate::server::rest::middleware::AuthContext;
use crate::server::rest::rbac_enforcement::{check_api_permission, permissions};

#[derive(Debug, Serialize)]
pub struct SpaceSecret {
    pub space: String,
    pub key_name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: String,
}


#[derive(Debug, Deserialize)]
pub struct CreateSpaceSecretRequest {
    pub key_name: String,
    pub value: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSpaceSecretRequest {
    pub value: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListSecretsQuery {
    #[serde(default)]
    pub show_values: bool,
}

/// List all secrets in a space
pub async fn list_space_secrets(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
    Query(query): Query<ListSecretsQuery>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<Vec<serde_json::Value>>> {
    // Check permission for reading space secrets
    check_api_permission(&auth, &state, &permissions::SPACE_SECRET_LIST, Some(&space))
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to list space secrets".to_string()))?;

    // Check if space exists
    let _space = state.get_space(&space)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch space: {}", e)))?
        .ok_or(ApiError::NotFound("Space not found".to_string()))?;

    let secrets = state.get_space_secrets(&space)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch space secrets: {}", e)))?;

    let response: Vec<serde_json::Value> = if query.show_values {
        // Only admins or users with special permission can see values
        check_api_permission(&auth, &state, &permissions::SPACE_SECRET_READ_VALUES, Some(&space))
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions to view secret values".to_string()))?;
        
        secrets.into_iter().map(|s| serde_json::to_value(s).unwrap()).collect()
    } else {
        // Return secrets without values
        secrets.into_iter().map(|s| {
            serde_json::json!({
                "space": s.space,
                "key_name": s.key_name,
                "description": s.description,
                "created_at": s.created_at,
                "updated_at": s.updated_at,
                "created_by": s.created_by
            })
        }).collect()
    };

    Ok(Json(response))
}

/// Get a specific secret in a space
pub async fn get_space_secret(
    State(state): State<Arc<AppState>>,
    Path((space, key_name)): Path<(String, String)>,
    Query(query): Query<ListSecretsQuery>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    // Check permission for reading space secrets
    check_api_permission(&auth, &state, &permissions::SPACE_SECRET_GET, Some(&space))
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to get space secret".to_string()))?;

    let secret = state.get_space_secret(&space, &key_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch space secret: {}", e)))?
        .ok_or(ApiError::NotFound("Space secret not found".to_string()))?;

    let response = if query.show_values {
        // Only admins or users with special permission can see values
        check_api_permission(&auth, &state, &permissions::SPACE_SECRET_READ_VALUES, Some(&space))
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions to view secret value".to_string()))?;
        
        serde_json::to_value(secret).unwrap()
    } else {
        serde_json::json!({
            "space": secret.space,
            "key_name": secret.key_name,
            "description": secret.description,
            "created_at": secret.created_at,
            "updated_at": secret.updated_at,
            "created_by": secret.created_by
        })
    };

    Ok(Json(response))
}

/// Create a new secret in a space
pub async fn create_space_secret(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateSpaceSecretRequest>,
) -> ApiResult<Json<SpaceSecret>> {
    // Check permission for creating space secrets
    check_api_permission(&auth, &state, &permissions::SPACE_SECRET_CREATE, Some(&space))
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to create space secret".to_string()))?;

    // Check if space exists
    let _space = state.get_space(&space)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch space: {}", e)))?
        .ok_or(ApiError::NotFound("Space not found".to_string()))?;

    // Get the principal name
    let created_by = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
        crate::shared::rbac::AuthPrincipal::ServiceAccount(sa) => &sa.user,
    };

    let secret = state.create_space_secret(
        &space,
        &req.key_name,
        &req.value, // TODO: Encrypt this value
        req.description.as_deref(),
        created_by,
    )
    .await
    .map_err(|e| match e {
        crate::shared::models::DatabaseError::Unique(_) => {
            ApiError::Conflict("A secret with this key already exists in the space".to_string())
        }
        _ => ApiError::Internal(anyhow::anyhow!("Failed to create space secret: {}", e))
    })?;

    Ok(Json(SpaceSecret {
        space: secret.space,
        key_name: secret.key_name,
        description: secret.description,
        created_at: secret.created_at,
        updated_at: secret.updated_at,
        created_by: secret.created_by,
    }))
}

/// Update a secret in a space
pub async fn update_space_secret(
    State(state): State<Arc<AppState>>,
    Path((space, key_name)): Path<(String, String)>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<UpdateSpaceSecretRequest>,
) -> ApiResult<Json<SpaceSecret>> {
    // Check permission for updating space secrets
    check_api_permission(&auth, &state, &permissions::SPACE_SECRET_UPDATE, Some(&space))
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to update space secret".to_string()))?;

    // Check if secret exists
    let _secret = state.get_space_secret(&space, &key_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch space secret: {}", e)))?
        .ok_or(ApiError::NotFound("Space secret not found".to_string()))?;

    let updated = state.update_space_secret(
        &space,
        &key_name,
        req.value.as_deref(), // TODO: Encrypt this value if provided
        req.description.as_deref(),
    )
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to update space secret: {}", e)))?;

    if !updated {
        return Err(ApiError::NotFound("Space secret not found".to_string()));
    }

    // Fetch the updated secret
    let secret = state.get_space_secret(&space, &key_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch updated space secret: {}", e)))?
        .ok_or(ApiError::Internal(anyhow::anyhow!("Secret disappeared after update")))?;

    Ok(Json(SpaceSecret {
        space: secret.space,
        key_name: secret.key_name,
        description: secret.description,
        created_at: secret.created_at,
        updated_at: secret.updated_at,
        created_by: secret.created_by,
    }))
}

/// Delete a secret from a space
pub async fn delete_space_secret(
    State(state): State<Arc<AppState>>,
    Path((space, key_name)): Path<(String, String)>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<()> {
    // Check permission for deleting space secrets
    check_api_permission(&auth, &state, &permissions::SPACE_SECRET_DELETE, Some(&space))
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions to delete space secret".to_string()))?;

    let deleted = state.delete_space_secret(&space, &key_name)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to delete space secret: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("Space secret not found".to_string()));
    }

    Ok(())
}