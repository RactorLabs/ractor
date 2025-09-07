use crate::server::auth::check_permission;
use crate::server::rest::middleware::AuthContext;
use crate::shared::models::AppState;
use crate::shared::rbac::PermissionContext;
use axum::http::StatusCode;

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
        tracing::warn!(
            "Permission denied for {} on {}/{}/{}",
            auth.principal.name(),
            context.api_group,
            context.resource,
            context.verb
        );
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(())
}

/// Permission definitions for all API endpoints
pub mod permissions {
    use super::PermissionRequirement;

    // Operator permissions
    pub const OPERATOR_LIST: PermissionRequirement =
        PermissionRequirement::new("api", "operators", "list");
    pub const OPERATOR_GET: PermissionRequirement =
        PermissionRequirement::new("api", "operators", "get");
    pub const OPERATOR_CREATE: PermissionRequirement =
        PermissionRequirement::new("api", "operators", "create");
    pub const OPERATOR_UPDATE: PermissionRequirement =
        PermissionRequirement::new("api", "operators", "update");
    pub const OPERATOR_DELETE: PermissionRequirement =
        PermissionRequirement::new("api", "operators", "delete");

    // Agent permissions
    #[allow(dead_code)]
    pub const AGENT_LIST: PermissionRequirement =
        PermissionRequirement::new("api", "agents", "list");
    #[allow(dead_code)]
    pub const AGENT_GET: PermissionRequirement = PermissionRequirement::new("api", "agents", "get");
    #[allow(dead_code)]
    pub const AGENT_CREATE: PermissionRequirement =
        PermissionRequirement::new("api", "agents", "create");
    pub const AGENT_UPDATE: PermissionRequirement =
        PermissionRequirement::new("api", "agents", "update");
    pub const AGENT_DELETE: PermissionRequirement =
        PermissionRequirement::new("api", "agents", "delete");
    #[allow(dead_code)]
    pub const AGENT_LIST_ALL: PermissionRequirement =
        PermissionRequirement::new("api", "agents", "list-all");
}
