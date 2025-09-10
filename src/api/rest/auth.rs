use axum::{
    extract::{Extension, Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::api::auth::{authenticate_operator, create_operator_jwt};
use crate::api::rest::error::{ApiError, ApiResult};
use crate::shared::models::AppState;
use crate::shared::rbac::TokenResponse;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub pass: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTokenRequest {
    pub principal: String,
    #[serde(rename = "type")]
    pub principal_type: String, // "User" or "Admin"
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
            user: String::new(), // Will be filled in by the caller
            role: String::new(), // Will be filled in by the caller
        }
    }
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Json<LoginResponse>> {
    tracing::debug!("Login attempt for operator: {}", &name);

    let operator = match authenticate_operator(&state, &name, &req.pass).await {
        Ok(Some(account)) => account,
        Ok(None) => {
            tracing::debug!("Authentication failed: invalid credentials for {}", &name);
            return Err(ApiError::Unauthorized);
        }
        Err(e) => {
            tracing::error!(
                "Database error during authentication for {}: {:?}",
                &name,
                e
            );
            return Err(ApiError::Database(e));
        }
    };

    // Update last login timestamp
    let _ = state.update_last_login(&name).await;

    let token_response = create_operator_jwt(&operator, &state.jwt_secret, 24)?;

    // Include user info in response
    let mut response: LoginResponse = token_response.into();
    response.user = operator.user.clone();
    response.role = "admin".to_string();

    Ok(Json(response))
}

pub async fn me(
    Extension(auth): Extension<crate::api::rest::middleware::AuthContext>,
) -> ApiResult<Json<serde_json::Value>> {
    use crate::shared::rbac::AuthPrincipal;

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
    Extension(auth): Extension<crate::api::rest::middleware::AuthContext>,
    Json(req): Json<CreateTokenRequest>,
) -> ApiResult<Json<LoginResponse>> {
    use crate::shared::rbac::{AuthPrincipal, RbacClaims, SubjectType};
    use chrono::{Duration, Utc};
    use jsonwebtoken::{encode, EncodingKey, Header};

    // Explicitly check that only admin operators can create tokens
    match &auth.principal {
        AuthPrincipal::Operator(op) => {
            if op.user != "admin" {
                return Err(ApiError::Forbidden(
                    "Only admin can create tokens".to_string(),
                ));
            }
        }
        AuthPrincipal::Subject(_) => {
            return Err(ApiError::Forbidden(
                "Only admin can create tokens".to_string(),
            ));
        }
    }

    tracing::info!(
        "Creating token for principal: {} type: {}",
        req.principal,
        req.principal_type
    );

    // Parse principal type
    let principal_type = match req.principal_type.as_str() {
        "User" => SubjectType::Subject,
        "Admin" => SubjectType::Admin,
        _ => {
            return Err(ApiError::BadRequest(
                "Invalid type. Must be 'User' or 'Admin'".to_string(),
            ))
        }
    };

    // Create JWT claims
    let expires_in = 24; // 24 hours
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(expires_in))
        .expect("valid timestamp");
    let claims = RbacClaims {
        sub: req.principal.clone(),
        sub_type: match principal_type {
            SubjectType::Subject => SubjectType::Subject,
            SubjectType::Admin => SubjectType::Admin,
        },
        exp: expiration.timestamp() as usize,
        iat: Utc::now().timestamp() as usize,
        iss: "raworc-rbac".to_string(),
    };

    // Sign JWT
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
