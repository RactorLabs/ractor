use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use crate::server::rest::{
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
            "/published/agents/{id}",
            get(handlers::agents::get_published_agent),
        );

    // Protected routes
    let protected_routes = Router::new()
        .route("/auth", get(auth::me))
        .route("/auth/token", post(auth::create_token))
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
        .route("/agents/{id}", get(handlers::agents::get_agent))
        .route("/agents/{id}", put(handlers::agents::update_agent))
        .route(
            "/agents/{id}/state",
            put(handlers::agents::update_agent_state),
        )
        .route(
            "/agents/{id}/busy",
            post(handlers::agents::update_agent_to_busy),
        )
        .route(
            "/agents/{id}/idle",
            post(handlers::agents::update_agent_to_idle),
        )
        .route(
            "/agents/{id}/sleep",
            post(handlers::agents::sleep_agent),
        )
        .route(
            "/agents/{id}/wake",
            post(handlers::agents::wake_agent),
        )
        .route(
            "/agents/{id}/remix",
            post(handlers::agents::remix_agent),
        )
        .route(
            "/agents/{id}/publish",
            post(handlers::agents::publish_agent),
        )
        .route(
            "/agents/{id}/unpublish",
            post(handlers::agents::unpublish_agent),
        )
        .route("/agents/{id}", delete(handlers::agents::delete_agent))
        // Message endpoints
        .route(
            "/agents/{id}/messages",
            get(handlers::messages::list_messages),
        )
        .route(
            "/agents/{id}/messages",
            post(handlers::messages::create_message),
        )
        .route(
            "/agents/{id}/messages/count",
            get(handlers::messages::get_message_count),
        )
        .route(
            "/agents/{id}/messages",
            delete(handlers::messages::clear_messages),
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
        "version": "0.4.0",
        "api": "v0"
    }))
}
