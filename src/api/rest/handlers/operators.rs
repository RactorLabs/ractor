use axum::{
    extract::{Path, State},
    Extension, Json,
};
use bcrypt::{hash, DEFAULT_COST};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::api::rest::rbac_enforcement::{check_api_permission, permissions};
use crate::shared::models::AppState;
use crate::shared::rbac::{Operator, PermissionContext};

#[derive(Debug, Deserialize)]
pub struct CreateOperatorRequest {
    pub user: String,
    pub pass: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

// Custom deserializer for strict optional boolean validation
fn deserialize_strict_option_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Visitor};

    struct StrictOptionBoolVisitor;

    impl<'de> Visitor<'de> for StrictOptionBoolVisitor {
        type Value = Option<bool>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a boolean value (true or false) or null")
        }

        fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Some(value))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(None)
        }

        // Reject all other types
        fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Err(E::custom("expected boolean or null, found string"))
        }

        fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Err(E::custom("expected boolean or null, found integer"))
        }

        fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Err(E::custom("expected boolean or null, found integer"))
        }

        fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Err(E::custom("expected boolean or null, found number"))
        }
    }

    deserializer.deserialize_option(StrictOptionBoolVisitor)
}

#[derive(Debug, Deserialize)]
pub struct UpdateOperatorRequest {
    pub description: Option<String>,
    #[serde(deserialize_with = "deserialize_strict_option_bool")]
    pub active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct OperatorResponse {
    pub user: String,
    pub description: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_login_at: Option<String>,
}

impl From<Operator> for OperatorResponse {
    fn from(op: Operator) -> Self {
        Self {
            user: op.user,
            description: op.description,
            active: op.active,
            created_at: op.created_at,
            updated_at: op.updated_at,
            last_login_at: op.last_login_at,
        }
    }
}

pub async fn list_operators(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<OperatorResponse>>> {
    // Principals with wildcard permission can list all; others only see themselves
    let is_admin = crate::api::auth::check_permission(
        &auth.principal,
        &state,
        &PermissionContext { api_group: "api".into(), resource: "*".into(), verb: "*".into() },
    )
    .await
    .unwrap_or(false);
    if is_admin {
        // Check permission (admin role should allow list)
        check_api_permission(&auth, &state, &permissions::OPERATOR_LIST)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions".to_string()))?;

        let operators = state.get_all_operators().await?;
        let response: Vec<OperatorResponse> = operators.into_iter().map(Into::into).collect();
        Ok(Json(response))
    } else {
        // Return only the authenticated operator
        let self_name = match &auth.principal {
            crate::shared::rbac::AuthPrincipal::Operator(op) => op.user.clone(),
            crate::shared::rbac::AuthPrincipal::Subject(s) => s.name.clone(),
        };
        let mut result = Vec::new();
        if let Some(op) = state.get_operator(&self_name).await? { result.push(OperatorResponse::from(op)); }
        Ok(Json(result))
    }
}

pub async fn get_operator(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> ApiResult<Json<OperatorResponse>> {
    // Allow self-read without RBAC; otherwise require permission
    let self_name = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Operator(op) => &op.user,
        crate::shared::rbac::AuthPrincipal::Subject(s) => &s.name,
    };
    let is_self = self_name == &name;
    if !is_self {
        check_api_permission(&auth, &state, &permissions::OPERATOR_GET)
            .await
            .map_err(|_| ApiError::Forbidden("Insufficient permissions".to_string()))?;
    }
    // Principals with wildcard permission can read others; others only self
    let is_admin = crate::api::auth::check_permission(
        &auth.principal,
        &state,
        &PermissionContext { api_group: "api".into(), resource: "*".into(), verb: "*".into() },
    )
    .await
    .unwrap_or(false);
    if !is_admin {
        if self_name != &name {
            return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
        }
    }
    let operator = state.get_operator(&name).await?;

    let operator = operator.ok_or(ApiError::NotFound("Operator not found".to_string()))?;
    Ok(Json(operator.into()))
}

pub async fn create_operator(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateOperatorRequest>,
) -> ApiResult<Json<OperatorResponse>> {
    // Check permission
    check_api_permission(&auth, &state, &permissions::OPERATOR_CREATE)
        .await
        .map_err(|e| match e {
            axum::http::StatusCode::FORBIDDEN => {
                ApiError::Forbidden("Insufficient permissions".to_string())
            }
            _ => ApiError::Internal(anyhow::anyhow!("Permission check failed")),
        })?;
    // Check if already exists
    if let Ok(Some(_)) = state.get_operator(&req.user).await {
        return Err(ApiError::Conflict("Operator already exists".to_string()));
    }

    let pass_hash = hash(&req.pass, DEFAULT_COST)?;
    let operator = state
        .create_operator(&req.user, &pass_hash, req.description)
        .await?;

    Ok(Json(operator.into()))
}

pub async fn delete_operator(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> ApiResult<()> {
    // Check permission
    check_api_permission(&auth, &state, &permissions::OPERATOR_DELETE)
        .await
        .map_err(|e| match e {
            axum::http::StatusCode::FORBIDDEN => {
                ApiError::Forbidden("Insufficient permissions".to_string())
            }
            _ => ApiError::Internal(anyhow::anyhow!("Permission check failed")),
        })?;
    // Operators use name as primary key now
    let deleted = state.delete_operator(&name).await?;

    if !deleted {
        return Err(ApiError::NotFound("Operator not found".to_string()));
    }

    Ok(())
}

pub async fn update_operator_password(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(req): Json<UpdatePasswordRequest>,
) -> ApiResult<()> {
    // Check permission - users can update their own password, admins can update any
    let is_self = match &auth.principal {
        crate::shared::rbac::AuthPrincipal::Operator(op) => op.user == name,
        _ => false,
    };

    if !is_self {
        check_api_permission(&auth, &state, &permissions::OPERATOR_UPDATE)
            .await
            .map_err(|e| match e {
                axum::http::StatusCode::FORBIDDEN => {
                    ApiError::Forbidden("Insufficient permissions".to_string())
                }
                _ => ApiError::Internal(anyhow::anyhow!("Permission check failed")),
            })?;
    }
    use bcrypt::verify;

    // Get the operator first
    let operator = state.get_operator(&name).await?;
    let operator = operator.ok_or(ApiError::NotFound("Operator not found".to_string()))?;

    // Verify current password
    if !verify(&req.current_password, &operator.pass_hash)? {
        return Err(ApiError::Unauthorized);
    }

    // Hash new password
    let new_pass_hash = hash(&req.new_password, DEFAULT_COST)?;

    // Update password - use name as primary key
    let updated = state
        .update_operator_password(&operator.user, &new_pass_hash)
        .await?;

    if !updated {
        return Err(ApiError::NotFound("Operator not found".to_string()));
    }

    Ok(())
}

pub async fn update_operator(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(req): Json<UpdateOperatorRequest>,
) -> ApiResult<Json<OperatorResponse>> {
    // Check permission
    check_api_permission(&auth, &state, &permissions::OPERATOR_UPDATE)
        .await
        .map_err(|e| match e {
            axum::http::StatusCode::FORBIDDEN => {
                ApiError::Forbidden("Insufficient permissions".to_string())
            }
            _ => ApiError::Internal(anyhow::anyhow!("Permission check failed")),
        })?;
    // Check if operator exists
    let operator = state.get_operator(&name).await?;
    let operator = operator.ok_or(ApiError::NotFound("Operator not found".to_string()))?;

    // Update the operator - use name as primary key
    let updated = state
        .update_operator(&operator.user, req.description, req.active)
        .await?;

    if !updated {
        return Err(ApiError::NotFound("Operator not found".to_string()));
    }

    // Fetch the updated operator
    let updated_operator = state
        .get_operator(&name)
        .await?
        .ok_or(ApiError::NotFound("Operator not found".to_string()))?;

    Ok(Json(updated_operator.into()))
}
