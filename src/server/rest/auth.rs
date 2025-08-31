use axum::{
    extract::{Extension, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::server::auth::{authenticate_service_account, create_service_account_jwt};
use crate::shared::models::AppState;
use crate::shared::rbac::TokenResponse;
use crate::server::rest::error::{ApiError, ApiResult};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub user: String,
    pub pass: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTokenRequest {
    pub principal: String,
    pub principal_type: String, // "User" or "ServiceAccount"
}


#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub token_type: String,
    pub expires_at: String,
}

impl From<TokenResponse> for LoginResponse {
    fn from(token: TokenResponse) -> Self {
        Self {
            token: token.token,
            token_type: "Bearer".to_string(),
            expires_at: token.expires_at,
        }
    }
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Json<LoginResponse>> {
    tracing::debug!("Login attempt for user: {}", &req.user);
    
    let service_account = match authenticate_service_account(
        &state,
        &req.user,
        &req.pass,
    )
    .await {
        Ok(Some(account)) => account,
        Ok(None) => {
            tracing::debug!("Authentication failed: invalid credentials for {}", &req.user);
            return Err(ApiError::Unauthorized);
        },
        Err(e) => {
            tracing::error!("Database error during authentication for {}: {:?}", &req.user, e);
            return Err(ApiError::Database(e));
        }
    };

    // Update last login timestamp
    let _ = state.update_last_login(&req.user).await;

    let token_response = create_service_account_jwt(&service_account, &state.jwt_secret, 24)?;
    
    Ok(Json(token_response.into()))
}


pub async fn me(
    Extension(auth): Extension<crate::server::rest::middleware::AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    use crate::shared::rbac::AuthPrincipal;
    
    let (user, principal_type) = match &auth.principal {
        AuthPrincipal::Subject(s) => (&s.name, "Subject"),
        AuthPrincipal::ServiceAccount(sa) => (&sa.user, "ServiceAccount"),
    };
    
    Ok(Json(serde_json::json!({
        "user": user,
        "type": principal_type
    })))
}

pub async fn create_token(
    State(state): State<Arc<AppState>>,
    Extension(_auth): Extension<crate::server::rest::middleware::AuthContext>,
    Json(req): Json<CreateTokenRequest>,
) -> ApiResult<Json<LoginResponse>> {
    use crate::shared::rbac::{SubjectType, RbacClaims};
    use jsonwebtoken::{encode, EncodingKey, Header};
    use chrono::{Duration, Utc};

    // Only admin can create tokens (implied by auth middleware)
    tracing::info!("Creating token for principal: {} type: {}", req.principal, req.principal_type);

    // Parse principal type
    let principal_type = match req.principal_type.as_str() {
        "User" => SubjectType::Subject,
        "ServiceAccount" => SubjectType::ServiceAccount,
        _ => return Err(ApiError::BadRequest("Invalid principal_type. Must be 'User' or 'ServiceAccount'".to_string())),
    };

    // Create JWT claims for the principal (non-admin, limited access)
    let exp = Utc::now() + Duration::hours(24);
    let claims = RbacClaims {
        sub: req.principal.clone(),
        sub_type: principal_type,
        exp: exp.timestamp() as usize,
        iat: Utc::now().timestamp() as usize,
        iss: "raworc-auth".to_string(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_ref()),
    ).map_err(|e| ApiError::Internal(anyhow::anyhow!("JWT encoding failed: {}", e)))?;

    let token_response = TokenResponse {
        token,
        expires_at: exp.to_rfc3339(),
    };

    Ok(Json(token_response.into()))
}