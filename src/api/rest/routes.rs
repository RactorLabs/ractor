use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use crate::api::rest::{
    auth, handlers, logging_middleware::request_logging_middleware, middleware::auth_middleware,
};
use crate::shared::models::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    // Public routes
    let public_routes = Router::new()
        .route("/version", get(version))
        .route("/operators/{name}/login", post(auth::login))
        // Public agent endpoints (no auth required)
        .route(
            "/published/agents",
            get(handlers::agents::list_published_agents),
        )
        .route(
            "/published/agents/{name}",
            get(handlers::agents::get_published_agent),
        );

    // Protected routes
    let protected_routes = Router::new()
        .route("/auth", get(auth::me))
        .route("/auth/token", post(auth::create_token))
        // Security / Blocklist (admin only)
        .route("/blocklist", get(handlers::security::list_blocked))
        .route(
            "/blocklist/block",
            post(handlers::security::block_principal),
        )
        .route(
            "/blocklist/unblock",
            post(handlers::security::unblock_principal),
        )
        // Operator endpoints
        .route("/operators", get(handlers::operators::list_operators))
        .route("/operators", post(handlers::operators::create_operator))
        .route("/operators/{name}", get(handlers::operators::get_operator))
        .route(
            "/operators/{name}",
            put(handlers::operators::update_operator),
        )
        .route(
            "/operators/{name}",
            delete(handlers::operators::delete_operator),
        )
        .route(
            "/operators/{name}/password",
            put(handlers::operators::update_operator_password),
        )
        // Agent endpoints
        .route("/agents", get(handlers::agents::list_agents))
        .route("/agents", post(handlers::agents::create_agent))
        .route("/agents/{name}", get(handlers::agents::get_agent))
        .route("/agents/{name}", put(handlers::agents::update_agent))
        .route(
            "/agents/{name}/state",
            put(handlers::agents::update_agent_state),
        )
        .route(
            "/agents/{name}/busy",
            post(handlers::agents::update_agent_to_busy),
        )
        .route(
            "/agents/{name}/idle",
            post(handlers::agents::update_agent_to_idle),
        )
        .route("/agents/{name}/sleep", post(handlers::agents::sleep_agent))
        .route("/agents/{name}/cancel", post(handlers::agents::cancel_active_response))
        .route("/agents/{name}/wake", post(handlers::agents::wake_agent))
        .route(
            "/agents/{name}/runtime",
            get(handlers::agents::get_agent_runtime),
        )
        .route("/agents/{name}/remix", post(handlers::agents::remix_agent))
        .route(
            "/agents/{name}/context",
            get(handlers::agents::get_agent_context),
        )
        .route(
            "/agents/{name}/context/clear",
            post(handlers::agents::clear_agent_context),
        )
        .route(
            "/agents/{name}/context/compact",
            post(handlers::agents::compact_agent_context),
        )
        .route(
            "/agents/{name}/publish",
            post(handlers::agents::publish_agent),
        )
        .route(
            "/agents/{name}/unpublish",
            post(handlers::agents::unpublish_agent),
        )
        .route("/agents/{name}", delete(handlers::agents::delete_agent))
        // Message endpoints removed in favor of Responses
        // Response endpoints (composite model)
        .route(
            "/agents/{name}/responses",
            get(handlers::responses::list_responses),
        )
        .route(
            "/agents/{name}/responses",
            post(handlers::responses::create_response),
        )
        .route(
            "/agents/{name}/responses/{id}",
            get(handlers::responses::get_response_by_id).put(handlers::responses::update_response),
        )
        .route(
            "/agents/{name}/responses/count",
            get(handlers::responses::get_response_count),
        )
        // Agent files (read-only)
        .route(
            "/agents/{name}/files/read/{*path}",
            get(handlers::agents::read_agent_file),
        )
        .route(
            "/agents/{name}/files/metadata/{*path}",
            get(handlers::agents::get_agent_file_metadata),
        )
        .route(
            "/agents/{name}/files/list/{*path}",
            get(handlers::agents::list_agent_files),
        )
        .route(
            "/agents/{name}/files/list",
            get(handlers::agents::list_agent_files_root),
        )
        .route(
            "/agents/{name}/files/delete/{*path}",
            delete(handlers::agents::delete_agent_file),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let api_routes = public_routes
        .merge(protected_routes)
        .with_state(state.clone());

    Router::new()
        .nest("/api/v0", api_routes)
        .layer(middleware::from_fn(request_logging_middleware))
        .layer(TraceLayer::new_for_http())
}

async fn version() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "version": "0.5.3",
        "api": "v0"
    }))
}
