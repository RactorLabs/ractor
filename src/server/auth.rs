use crate::shared::{AppState};
use crate::shared::models::DatabaseError;
use crate::shared::rbac::{
    AuthPrincipal, PermissionContext, RbacAuthz, RbacClaims, ServiceAccount, SubjectType,
    TokenResponse,
};
use anyhow::Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};




// Legacy Auth guard for backward compatibility during migration


// JWT utility functions for RBAC
pub fn create_service_account_jwt(
    service_account: &ServiceAccount,
    secret: &str,
    duration_hours: i64,
) -> Result<TokenResponse> {
    let exp = Utc::now()
        .checked_add_signed(Duration::hours(duration_hours))
        .expect("valid timestamp");

    let claims = RbacClaims {
        sub: service_account.user.clone(),
        sub_type: SubjectType::ServiceAccount,
        exp: exp.timestamp() as usize,
        iat: Utc::now().timestamp() as usize,
        iss: "raworc-rbac".to_string(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )?;

    Ok(TokenResponse {
        token,
        expires_at: exp.to_rfc3339(),
    })
}


pub fn decode_rbac_jwt(token: &str, secret: &str) -> Result<RbacClaims> {
    let token_data: TokenData<RbacClaims> = decode(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )?;

    Ok(token_data.claims)
}

// Permission checking function
pub async fn check_permission(
    principal: &AuthPrincipal,
    app_state: &AppState,
    context: &PermissionContext,
) -> Result<bool, DatabaseError> {
    // Get all roles and role bindings
    let roles = app_state.get_all_roles().await?;
    let role_bindings = app_state
        .get_role_bindings_for_subject(
            principal.name(),
            principal.subject_type(),
        )
        .await?;

    tracing::info!(
        "RBAC check: found {} roles, {} role bindings for principal {}",
        roles.len(),
        role_bindings.len(),
        principal.name()
    );

    for binding in &role_bindings {
        tracing::info!(
            "Role binding: principal={} role={}",
            binding.principal,
            binding.role_name
        );
    }

    // Use RBAC authorization engine
    let result = RbacAuthz::has_permission(principal, &roles, &role_bindings, context);
    tracing::info!("RBAC authorization result: {}", result);
    Ok(result)
}


// Authentication functions
pub async fn authenticate_service_account(
    app_state: &AppState,
    user: &str,
    pass: &str,
) -> Result<Option<ServiceAccount>, DatabaseError> {
    if let Some(service_account) = app_state.get_service_account(user).await? {
        if service_account.active {
            let is_valid = bcrypt::verify(pass, &service_account.pass_hash).unwrap_or(false);
            if is_valid {
                return Ok(Some(service_account));
            }
        }
    }
    Ok(None)
}

// Exported JWT functions for REST API
pub fn decode_jwt(token: &str, secret: &str) -> Result<RbacClaims> {
    decode_rbac_jwt(token, secret)
}

// Function removed - was unused
