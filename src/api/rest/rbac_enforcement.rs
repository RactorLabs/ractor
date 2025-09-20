use axum::http::StatusCode;

use crate::api::auth::check_permission;
use crate::shared::models::AppState;
use crate::shared::rbac::PermissionContext;

use super::middleware::AuthContext;

// Define permissions as simple (&api_group, &resource, &verb) tuples
pub mod permissions {
    pub const OPERATOR_LIST: (&str, &str, &str) = ("api", "operators", "list");
    pub const OPERATOR_GET: (&str, &str, &str) = ("api", "operators", "get");
    pub const OPERATOR_CREATE: (&str, &str, &str) = ("api", "operators", "create");
    pub const OPERATOR_UPDATE: (&str, &str, &str) = ("api", "operators", "update");
    pub const OPERATOR_DELETE: (&str, &str, &str) = ("api", "operators", "delete");

    pub const AGENT_LIST: (&str, &str, &str) = ("api", "agents", "list");
    pub const AGENT_GET: (&str, &str, &str) = ("api", "agents", "get");
    pub const AGENT_CREATE: (&str, &str, &str) = ("api", "agents", "create");
    pub const AGENT_UPDATE: (&str, &str, &str) = ("api", "agents", "update");
    pub const AGENT_DELETE: (&str, &str, &str) = ("api", "agents", "delete");
}

pub async fn check_api_permission(
    auth: &AuthContext,
    app_state: &AppState,
    perm: &(&str, &str, &str),
) -> Result<(), StatusCode> {
    let ctx = PermissionContext {
        api_group: perm.0.to_string(),
        resource: perm.1.to_string(),
        verb: perm.2.to_string(),
    };

    let allowed = check_permission(&auth.principal, app_state, &ctx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if allowed {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}
