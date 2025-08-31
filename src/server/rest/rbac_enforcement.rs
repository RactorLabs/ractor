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
}

impl PermissionRequirement {
    pub const fn new(api_group: &'static str, resource: &'static str, verb: &'static str) -> Self {
        Self {
            api_group,
            resource,
            verb,
        }
    }
}

/// Check if user has permission for the requested action
pub async fn check_api_permission(
    auth: &AuthContext,
    state: &AppState,
    requirement: &PermissionRequirement,
) -> Result<(), StatusCode> {
    let context = PermissionContext {
        api_group: requirement.api_group.to_string(),
        resource: requirement.resource.to_string(),
        verb: requirement.verb.to_string(),
    };

    tracing::info!(
        "Permission check: principal={} type={:?} api_group={} resource={} verb={}",
        auth.principal.name(),
        auth.principal.subject_type(),
        context.api_group,
        context.resource,
        context.verb
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
        PermissionRequirement::new("api", "service-accounts", "list");
    pub const SERVICE_ACCOUNT_GET: PermissionRequirement = 
        PermissionRequirement::new("api", "service-accounts", "get");
    pub const SERVICE_ACCOUNT_CREATE: PermissionRequirement = 
        PermissionRequirement::new("api", "service-accounts", "create");
    pub const SERVICE_ACCOUNT_UPDATE: PermissionRequirement = 
        PermissionRequirement::new("api", "service-accounts", "update");
    pub const SERVICE_ACCOUNT_DELETE: PermissionRequirement = 
        PermissionRequirement::new("api", "service-accounts", "delete");

    // Role permissions
    pub const ROLE_LIST: PermissionRequirement = 
        PermissionRequirement::new("api", "roles", "list");
    pub const ROLE_GET: PermissionRequirement = 
        PermissionRequirement::new("api", "roles", "get");
    pub const ROLE_CREATE: PermissionRequirement = 
        PermissionRequirement::new("api", "roles", "create");
    #[allow(dead_code)]
    pub const ROLE_UPDATE: PermissionRequirement = 
        PermissionRequirement::new("api", "roles", "update");
    pub const ROLE_DELETE: PermissionRequirement = 
        PermissionRequirement::new("api", "roles", "delete");

    // Role Binding permissions
    pub const ROLE_BINDING_LIST: PermissionRequirement = 
        PermissionRequirement::new("api", "role-bindings", "list");
    pub const ROLE_BINDING_GET: PermissionRequirement = 
        PermissionRequirement::new("api", "role-bindings", "get");
    pub const ROLE_BINDING_CREATE: PermissionRequirement = 
        PermissionRequirement::new("api", "role-bindings", "create");
    #[allow(dead_code)]
    pub const ROLE_BINDING_UPDATE: PermissionRequirement = 
        PermissionRequirement::new("api", "role-bindings", "update");
    pub const ROLE_BINDING_DELETE: PermissionRequirement = 
        PermissionRequirement::new("api", "role-bindings", "delete");

    // Session permissions
    #[allow(dead_code)]
    pub const SESSION_LIST: PermissionRequirement = 
        PermissionRequirement::new("api", "sessions", "list");
    #[allow(dead_code)]
    pub const SESSION_GET: PermissionRequirement = 
        PermissionRequirement::new("api", "sessions", "get");
    #[allow(dead_code)]
    pub const SESSION_CREATE: PermissionRequirement = 
        PermissionRequirement::new("api", "sessions", "create");
    pub const SESSION_UPDATE: PermissionRequirement = 
        PermissionRequirement::new("api", "sessions", "update");
    pub const SESSION_DELETE: PermissionRequirement = 
        PermissionRequirement::new("api", "sessions", "delete");
    #[allow(dead_code)]
    pub const SESSION_LIST_ALL: PermissionRequirement = 
        PermissionRequirement::new("api", "sessions", "list-all");

}