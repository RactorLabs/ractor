use axum::{
    extract::{Extension, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::shared::models::AppState;
use crate::shared::rbac::{AuthPrincipal, BlockedPrincipal, SubjectType, TokenResponse};

#[derive(Debug, Deserialize)]
pub struct CreateTokenRequest {
    pub principal: String,
    #[serde(rename = "type")]
    pub principal_type: String,
    #[serde(default)]
    pub ttl_hours: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BlockRequest {
    pub principal: String,
    #[serde(default)]
    #[serde(rename = "type")]
    pub principal_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub token_type: String,
    pub expires_at: String,
    pub user: String,
    pub role: String,
}

impl From<TokenResponse> for LoginResponse {
    fn from(token: TokenResponse) -> Self {
        Self {
            token: token.token,
            token_type: "Bearer".to_string(),
            expires_at: token.expires_at,
            user: String::new(),
            role: String::new(),
        }
    }
}

pub async fn me(Extension(auth): Extension<AuthContext>) -> ApiResult<Json<serde_json::Value>> {
    let (user, principal_type) = match &auth.principal {
        AuthPrincipal::Subject(s) => (&s.name, "User"),
        AuthPrincipal::Operator(op) => (&op.user, "Admin"),
    };

    Ok(Json(serde_json::json!({
        "user": user,
        "type": principal_type
    })))
}

pub async fn create_token(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateTokenRequest>,
) -> ApiResult<Json<LoginResponse>> {
    use crate::shared::rbac::RbacClaims;
    use chrono::{Duration, Utc};
    use jsonwebtoken::{encode, EncodingKey, Header};

    match &auth.principal {
        AuthPrincipal::Operator(op) if op.user == "admin" => {}
        _ => {
            return Err(ApiError::Forbidden(
                "Only admin can create tokens".to_string(),
            ))
        }
    }

    let principal_type = match req.principal_type.as_str() {
        "User" => SubjectType::Subject,
        "Admin" => SubjectType::Admin,
        _ => {
            return Err(ApiError::BadRequest(
                "Invalid type. Must be 'User' or 'Admin'".to_string(),
            ))
        }
    };

    let expiration = match req.ttl_hours.filter(|h| *h > 0) {
        Some(h) => Utc::now()
            .checked_add_signed(Duration::hours(h))
            .expect("valid timestamp"),
        None => Utc::now()
            .checked_add_signed(Duration::days(36_500))
            .expect("valid timestamp"),
    };

    let claims = RbacClaims {
        sub: req.principal.clone(),
        sub_type: principal_type,
        exp: expiration.timestamp() as usize,
        iat: Utc::now().timestamp() as usize,
        iss: "tsbx-rbac".to_string(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_ref()),
    )
    .map_err(ApiError::Jwt)?;

    Ok(Json(LoginResponse {
        token,
        token_type: "Bearer".to_string(),
        expires_at: expiration.to_rfc3339(),
        user: req.principal.clone(),
        role: match principal_type {
            SubjectType::Subject => "user".to_string(),
            SubjectType::Admin => "admin".to_string(),
        },
    }))
}

pub async fn list_blocked(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<Vec<BlockedPrincipal>>> {
    ensure_admin(&auth)?;
    let items = state.list_blocked_principals().await?;
    Ok(Json(items))
}

pub async fn block_principal(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<BlockRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    ensure_admin(&auth)?;
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
    ensure_admin(&auth)?;
    let t = parse_subject_type_opt(&req.principal_type)?;
    let deleted = state.unblock_principal(&req.principal, t).await?;
    Ok(Json(
        serde_json::json!({ "blocked": false, "deleted": deleted }),
    ))
}

fn ensure_admin(auth: &AuthContext) -> Result<(), ApiError> {
    match &auth.principal {
        AuthPrincipal::Operator(op) if op.user == "admin" => Ok(()),
        _ => Err(ApiError::Forbidden(
            "Only admin can perform this action".to_string(),
        )),
    }
}

fn parse_subject_type_opt(s: &Option<String>) -> Result<SubjectType, ApiError> {
    match s.as_deref() {
        None => Ok(SubjectType::Subject),
        Some("User") => Ok(SubjectType::Subject),
        Some("Admin") => Ok(SubjectType::Admin),
        Some(_) => Err(ApiError::BadRequest(
            "Invalid type. Must be 'User' or 'Admin'".to_string(),
        )),
    }
}
