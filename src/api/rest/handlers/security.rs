use axum::{
    extract::{Extension, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::shared::models::AppState;
use crate::shared::rbac::{AuthPrincipal, BlockedPrincipal, SubjectType};

#[derive(Debug, Deserialize)]
pub struct BlockRequest {
    pub principal: String,
    #[serde(default)]
    #[serde(rename = "type")]
    pub principal_type: Option<String>, // Optional; defaults to "User"
}

fn ensure_admin_only(auth: &AuthContext) -> Result<(), ApiError> {
    match &auth.principal {
        AuthPrincipal::Operator(op) if op.user == "admin" => Ok(()),
        _ => Err(ApiError::Forbidden(
            "Only admin can perform this action".to_string(),
        )),
    }
}

fn parse_subject_type_opt(s: &Option<String>) -> Result<SubjectType, ApiError> {
    match s.as_deref() {
        None => Ok(SubjectType::Subject), // default
        Some("User") => Ok(SubjectType::Subject),
        Some("Admin") => Ok(SubjectType::Admin),
        Some(_) => Err(ApiError::BadRequest(
            "Invalid type. Must be 'User' or 'Admin'".to_string(),
        )),
    }
}

pub async fn list_blocked(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<Vec<BlockedPrincipal>>> {
    ensure_admin_only(&auth)?;
    let items = state.list_blocked_principals().await?;
    Ok(Json(items))
}

pub async fn block_principal(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<BlockRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    ensure_admin_only(&auth)?;
    let t = parse_subject_type_opt(&req.principal_type)?;
    let created = state.block_principal(&req.principal, t).await?;
    Ok(Json(
        serde_json::json!({ "blocked": true, "created": created }),
    ))
}

pub async fn unblock_principal(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<BlockRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    ensure_admin_only(&auth)?;
    let t = parse_subject_type_opt(&req.principal_type)?;
    let deleted = state.unblock_principal(&req.principal, t).await?;
    Ok(Json(
        serde_json::json!({ "blocked": false, "deleted": deleted }),
    ))
}
