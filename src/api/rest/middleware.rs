use crate::api::auth::decode_jwt;
use crate::api::rest::error::ApiError;
use crate::shared::models::AppState;
use crate::shared::rbac::{AuthPrincipal, Subject, SubjectType};
use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::info;

#[derive(Clone)]
pub struct AuthContext {
    pub principal: AuthPrincipal,
    pub token: String,
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Skip auth for public endpoints
    let path = request.uri().path();
    if path == "/api/v0/version" || path.starts_with("/api/v0/auth/") || path.contains("/login") {
        return Ok(next.run(request).await);
    }

    // Extract token from Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(ApiError::Unauthorized)?;

    // Decode and validate JWT
    let claims = decode_jwt(token, &state.jwt_secret).map_err(|_| ApiError::Unauthorized)?;

    // Get principal from claims
    let principal = match claims.sub_type {
        SubjectType::Admin => {
            let operator = state
                .get_operator(&claims.sub)
                .await
                .map_err(ApiError::Database)?
                .ok_or(ApiError::Unauthorized)?;
            AuthPrincipal::Operator(operator)
        }
        SubjectType::Subject => AuthPrincipal::Subject(Subject {
            name: claims.sub.clone(),
        }),
    };

    // Enforce blocklist for non-admin principals on protected routes
    if let AuthPrincipal::Subject(s) = &principal {
        let blocked = state
            .is_principal_blocked(&s.name, SubjectType::Subject)
            .await
            .map_err(ApiError::Database)?;
        if blocked {
            return Err(ApiError::Forbidden("Principal is blocked".to_string()));
        }
    }

    // Store auth context in request extensions
    let auth_context = AuthContext {
        principal: principal.clone(),
        token: token.to_string(),
    };
    request.extensions_mut().insert(auth_context);

    // Log the authenticated API request
    let method = request.method().clone();
    let uri = request.uri().clone();
    let user = match &principal {
        AuthPrincipal::Subject(s) => &s.name,
        AuthPrincipal::Operator(op) => &op.user,
    };

    info!(
        method = %method,
        path = %uri.path(),
        user = %user,
        "API request"
    );

    Ok(next.run(request).await)
}
