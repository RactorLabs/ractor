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
        .route("/agents/{name}/wake", post(handlers::agents::wake_agent))
        .route("/agents/{name}/remix", post(handlers::agents::remix_agent))
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
            put(handlers::responses::update_response),
        )
        .route(
            "/agents/{name}/responses/count",
            get(handlers::responses::get_response_count),
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
