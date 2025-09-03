use axum::{
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
        .route("/version", get(version))
        .route("/operators/{name}/login", post(auth::login))
        // Public session endpoints (no auth required)
        .route("/published/sessions", get(handlers::sessions::list_published_sessions))
        .route("/published/sessions/{id}", get(handlers::sessions::get_published_session));
    
    // Protected routes
    let protected_routes = Router::new()
        .route("/auth", get(auth::me))
        .route("/auth/token", post(auth::create_token))
        // Operator endpoints
        .route("/operators", get(handlers::operators::list_operators))
        .route("/operators", post(handlers::operators::create_operator))
        .route("/operators/{name}", get(handlers::operators::get_operator))
        .route("/operators/{name}", put(handlers::operators::update_operator))
        .route("/operators/{name}", delete(handlers::operators::delete_operator))
        .route("/operators/{name}/password", put(handlers::operators::update_operator_password))
        // Session endpoints
        .route("/sessions", get(handlers::sessions::list_sessions))
        .route("/sessions", post(handlers::sessions::create_session))
        .route("/sessions/{id}", get(handlers::sessions::get_session))
        .route("/sessions/{id}", put(handlers::sessions::update_session))
        .route("/sessions/{id}/state", put(handlers::sessions::update_session_state))
        .route("/sessions/{id}/busy", post(handlers::sessions::update_session_to_busy))
        .route("/sessions/{id}/idle", post(handlers::sessions::update_session_to_idle))
        .route("/sessions/{id}/close", post(handlers::sessions::close_session))
        .route("/sessions/{id}/restore", post(handlers::sessions::restore_session))
        .route("/sessions/{id}/remix", post(handlers::sessions::remix_session))
        .route("/sessions/{id}/publish", post(handlers::sessions::publish_session))
        .route("/sessions/{id}/unpublish", post(handlers::sessions::unpublish_session))
        .route("/sessions/{id}", delete(handlers::sessions::delete_session))
        // Message endpoints
        .route("/sessions/{id}/messages", get(handlers::messages::list_messages))
        .route("/sessions/{id}/messages", post(handlers::messages::create_message))
        .route("/sessions/{id}/messages/count", get(handlers::messages::get_message_count))
        .route("/sessions/{id}/messages", delete(handlers::messages::clear_messages))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    let api_routes = public_routes.merge(protected_routes).with_state(state.clone());

    Router::new()
        .nest("/api/v0", api_routes)
        .layer(middleware::from_fn(request_logging_middleware))
        .layer(TraceLayer::new_for_http())
}


async fn version() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "version": "0.3.8",
        "api": "v0"
    }))
}