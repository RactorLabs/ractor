use axum::http::StatusCode;
use crate::shared::models::AppState;
use crate::server::rest::middleware::AuthContext;
use crate::shared::rbac::PermissionContext;
use crate::server::auth::check_permission;

/// Permission requirements for each API endpoint
#[allow(dead_code)]
pub struct PermissionRequirement {
    pub api_group: &'static str,
    pub resource: &'static str,
    pub verb: &'static str,
    pub space_scoped: bool,
}

impl PermissionRequirement {
    pub const fn new(api_group: &'static str, resource: &'static str, verb: &'static str, space_scoped: bool) -> Self {
        Self {
            api_group,
            resource,
            verb,
            space_scoped,
        }
    }
}

/// Check if user has permission for the requested action
pub async fn check_api_permission(
    auth: &AuthContext,
    state: &AppState,
    requirement: &PermissionRequirement,
    target_space: Option<&str>,
) -> Result<(), StatusCode> {
    let context = PermissionContext {
        api_group: requirement.api_group.to_string(),
        resource: requirement.resource.to_string(),
        verb: requirement.verb.to_string(),
        space: target_space.map(|s| s.to_string()),
    };

    tracing::info!(
        "Permission check: principal={} type={:?} api_group={} resource={} verb={} space={:?}",
        auth.principal.name(),
        auth.principal.subject_type(),
        context.api_group,
        context.resource,
        context.verb,
        context.space
    );

    let has_permission = check_permission(&auth.principal, state, &context)
        .await
        .map_err(|e| {
            tracing::error!("Permission check database error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!("Permission check result: {}", has_permission);

    if !has_permission {
        tracing::warn!("Permission denied for {} on {}/{}/{}", 
            auth.principal.name(), context.api_group, context.resource, context.verb);
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(())
}

/// Permission definitions for all API endpoints
pub mod permissions {
    use super::PermissionRequirement;

    // Service Account permissions
    pub const SERVICE_ACCOUNT_LIST: PermissionRequirement = 
        PermissionRequirement::new("api", "service-accounts", "list", false);
    pub const SERVICE_ACCOUNT_GET: PermissionRequirement = 
        PermissionRequirement::new("api", "service-accounts", "get", false);
    pub const SERVICE_ACCOUNT_CREATE: PermissionRequirement = 
        PermissionRequirement::new("api", "service-accounts", "create", false);
    pub const SERVICE_ACCOUNT_UPDATE: PermissionRequirement = 
        PermissionRequirement::new("api", "service-accounts", "update", false);
    pub const SERVICE_ACCOUNT_DELETE: PermissionRequirement = 
        PermissionRequirement::new("api", "service-accounts", "delete", false);

    // Role permissions
    pub const ROLE_LIST: PermissionRequirement = 
        PermissionRequirement::new("api", "roles", "list", false);
    pub const ROLE_GET: PermissionRequirement = 
        PermissionRequirement::new("api", "roles", "get", false);
    pub const ROLE_CREATE: PermissionRequirement = 
        PermissionRequirement::new("api", "roles", "create", false);
    #[allow(dead_code)]
    pub const ROLE_UPDATE: PermissionRequirement = 
        PermissionRequirement::new("api", "roles", "update", false);
    pub const ROLE_DELETE: PermissionRequirement = 
        PermissionRequirement::new("api", "roles", "delete", false);

    // Role Binding permissions
    pub const ROLE_BINDING_LIST: PermissionRequirement = 
        PermissionRequirement::new("api", "role-bindings", "list", false);
    pub const ROLE_BINDING_GET: PermissionRequirement = 
        PermissionRequirement::new("api", "role-bindings", "get", false);
    pub const ROLE_BINDING_CREATE: PermissionRequirement = 
        PermissionRequirement::new("api", "role-bindings", "create", false);
    #[allow(dead_code)]
    pub const ROLE_BINDING_UPDATE: PermissionRequirement = 
        PermissionRequirement::new("api", "role-bindings", "update", false);
    pub const ROLE_BINDING_DELETE: PermissionRequirement = 
        PermissionRequirement::new("api", "role-bindings", "delete", false);

    // Session permissions (space-scoped)
    #[allow(dead_code)]
    pub const SESSION_LIST: PermissionRequirement = 
        PermissionRequirement::new("api", "sessions", "list", true);
    #[allow(dead_code)]
    pub const SESSION_GET: PermissionRequirement = 
        PermissionRequirement::new("api", "sessions", "get", true);
    #[allow(dead_code)]
    pub const SESSION_CREATE: PermissionRequirement = 
        PermissionRequirement::new("api", "sessions", "create", true);
    pub const SESSION_UPDATE: PermissionRequirement = 
        PermissionRequirement::new("api", "sessions", "update", true);
    pub const SESSION_DELETE: PermissionRequirement = 
        PermissionRequirement::new("api", "sessions", "delete", true);
    #[allow(dead_code)]
    pub const SESSION_LIST_ALL: PermissionRequirement = 
        PermissionRequirement::new("api", "sessions", "list-all", false);

    // Space permissions (global)
    pub const SPACE_LIST: PermissionRequirement = 
        PermissionRequirement::new("api", "spaces", "list", false);
    pub const SPACE_GET: PermissionRequirement = 
        PermissionRequirement::new("api", "spaces", "get", false);
    pub const SPACE_CREATE: PermissionRequirement = 
        PermissionRequirement::new("api", "spaces", "create", false);
    pub const SPACE_UPDATE: PermissionRequirement = 
        PermissionRequirement::new("api", "spaces", "update", false);
    pub const SPACE_DELETE: PermissionRequirement = 
        PermissionRequirement::new("api", "spaces", "delete", false);

    // Space secrets permissions (space-scoped)
    pub const SPACE_SECRET_LIST: PermissionRequirement = 
        PermissionRequirement::new("api", "space-secrets", "list", true);
    pub const SPACE_SECRET_GET: PermissionRequirement = 
        PermissionRequirement::new("api", "space-secrets", "get", true);
    pub const SPACE_SECRET_CREATE: PermissionRequirement = 
        PermissionRequirement::new("api", "space-secrets", "create", true);
    pub const SPACE_SECRET_UPDATE: PermissionRequirement = 
        PermissionRequirement::new("api", "space-secrets", "update", true);
    pub const SPACE_SECRET_DELETE: PermissionRequirement = 
        PermissionRequirement::new("api", "space-secrets", "delete", true);
    pub const SPACE_SECRET_READ_VALUES: PermissionRequirement = 
        PermissionRequirement::new("api", "space-secrets", "read-values", true);
        
    // Space build permissions (space-scoped)
    pub const SPACE_BUILD: PermissionRequirement = 
        PermissionRequirement::new("api", "space-builds", "create", true);
}

/// Check if user can access a specific space
#[allow(dead_code)]
pub async fn check_space_access(
    auth: &AuthContext,
    state: &AppState,
    target_space: &str,
) -> Result<bool, StatusCode> {
    // Check if user has global access
    let global_context = PermissionContext {
        api_group: "*".to_string(),
        resource: "*".to_string(),
        verb: "*".to_string(),
        space: None,
    };

    let has_global = check_permission(&auth.principal, state, &global_context)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if has_global {
        return Ok(true);
    }

    // Check if user has access to the specific space
    let space_context = PermissionContext {
        api_group: "*".to_string(),
        resource: "*".to_string(),
        verb: "*".to_string(),
        space: Some(target_space.to_string()),
    };

    let has_space_access = check_permission(&auth.principal, state, &space_context)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(has_space_access)
}