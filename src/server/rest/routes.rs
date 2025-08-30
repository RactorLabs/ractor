use axum::{
    http::StatusCode,
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use crate::shared::models::AppState;
use crate::server::rest::{auth, handlers, middleware::auth_middleware, logging_middleware::request_logging_middleware};

pub fn create_router(state: Arc<AppState>) -> Router {
    // Public routes
    let public_routes = Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
        .route("/auth/login", post(auth::login));
    
    // Protected routes
    let protected_routes = Router::new()
        .route("/auth/me", get(auth::me))
        // Service account endpoints
        .route("/service-accounts", get(handlers::service_accounts::list_service_accounts))
        .route("/service-accounts", post(handlers::service_accounts::create_service_account))
        .route("/service-accounts/{id}", get(handlers::service_accounts::get_service_account))
        .route("/service-accounts/{id}", put(handlers::service_accounts::update_service_account))
        .route("/service-accounts/{id}", delete(handlers::service_accounts::delete_service_account))
        .route("/service-accounts/{id}/password", put(handlers::service_accounts::update_service_account_password))
        // Role endpoints
        .route("/roles", get(handlers::roles::list_roles))
        .route("/roles", post(handlers::roles::create_role))
        .route("/roles/{id}", get(handlers::roles::get_role))
        .route("/roles/{id}", delete(handlers::roles::delete_role))
        // Role binding endpoints
        .route("/role-bindings", get(handlers::role_bindings::list_role_bindings))
        .route("/role-bindings", post(handlers::role_bindings::create_role_binding))
        .route("/role-bindings/{id}", get(handlers::role_bindings::get_role_binding))
        .route("/role-bindings/{id}", delete(handlers::role_bindings::delete_role_binding))
        // Session endpoints
        .route("/sessions", get(handlers::sessions::list_sessions))
        .route("/sessions", post(handlers::sessions::create_session))
        .route("/sessions/{id}", get(handlers::sessions::get_session))
        .route("/sessions/{id}", put(handlers::sessions::update_session))
        .route("/sessions/{id}/state", put(handlers::sessions::update_session_state))
        .route("/sessions/{id}/close", post(handlers::sessions::close_session))
        .route("/sessions/{id}/restore", post(handlers::sessions::restore_session))
        .route("/sessions/{id}/remix", post(handlers::sessions::remix_session))
        .route("/sessions/{id}", delete(handlers::sessions::delete_session))
        // Message endpoints
        .route("/sessions/{id}/messages", get(handlers::messages::list_messages))
        .route("/sessions/{id}/messages", post(handlers::messages::create_message))
        .route("/sessions/{id}/messages/count", get(handlers::messages::get_message_count))
        .route("/sessions/{id}/messages", delete(handlers::messages::clear_messages))
        // Space endpoints
        .route("/spaces", get(handlers::spaces::list_spaces))
        .route("/spaces", post(handlers::spaces::create_space))
        .route("/spaces/{space}", get(handlers::spaces::get_space))
        .route("/spaces/{space}", put(handlers::spaces::update_space))
        .route("/spaces/{space}", delete(handlers::spaces::delete_space))
        // Space secrets endpoints
        .route("/spaces/{space}/secrets", get(handlers::space_secrets::list_space_secrets))
        .route("/spaces/{space}/secrets", post(handlers::space_secrets::create_space_secret))
        .route("/spaces/{space}/secrets/{key_name}", get(handlers::space_secrets::get_space_secret))
        .route("/spaces/{space}/secrets/{key_name}", put(handlers::space_secrets::update_space_secret))
        .route("/spaces/{space}/secrets/{key_name}", delete(handlers::space_secrets::delete_space_secret))
        // Agent endpoints
        .route("/spaces/{space}/agents", get(handlers::agents::list_space_agents))
        .route("/spaces/{space}/agents", post(handlers::agents::create_agent))
        .route("/spaces/{space}/agents/{name}", get(handlers::agents::get_agent))
        .route("/spaces/{space}/agents/{name}", put(handlers::agents::update_agent))
        .route("/spaces/{space}/agents/{name}", delete(handlers::agents::delete_agent))
        .route("/spaces/{space}/agents/{name}/status", axum::routing::patch(handlers::agents::update_agent_status))
        .route("/spaces/{space}/agents/{name}/deploy", post(handlers::agents::deploy_agent))
        .route("/spaces/{space}/agents/{name}/stop", post(handlers::agents::stop_agent))
        .route("/spaces/{space}/agents/running", get(handlers::agents::list_running_agents))
        // Agent logs endpoint
        .route("/spaces/{space}/agents/{name}/logs", get(handlers::agent_logs::get_agent_logs))
        // Space build endpoints
        .route("/spaces/{space}/build", post(handlers::space_build::build_space))
        .route("/spaces/{space}/build/latest", get(handlers::space_build::get_latest_build))
        .route("/spaces/{space}/build/{build_id}", get(handlers::space_build::get_build_status))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    let api_routes = public_routes.merge(protected_routes).with_state(state.clone());

    Router::new()
        .nest("/api/v0", api_routes)
        .layer(middleware::from_fn(request_logging_middleware))
        .layer(TraceLayer::new_for_http())
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn version() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "version": "0.3.0",
        "api": "v0"
    }))
}