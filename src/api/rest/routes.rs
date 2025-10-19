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
        // Public session endpoints (no auth required)
        .route(
            "/published/sessions",
            get(handlers::sessions::list_published_sessions),
        )
        .route(
            "/published/sessions/{name}",
            get(handlers::sessions::get_published_session),
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
        // Session endpoints
        .route("/sessions", get(handlers::sessions::list_sessions))
        .route("/sessions", post(handlers::sessions::create_session))
        .route("/sessions/{name}", get(handlers::sessions::get_session))
        .route("/sessions/{name}", put(handlers::sessions::update_session))
        .route(
            "/sessions/{name}/state",
            put(handlers::sessions::update_session_state),
        )
        .route(
            "/sessions/{name}/busy",
            post(handlers::sessions::update_session_to_busy),
        )
        .route(
            "/sessions/{name}/idle",
            post(handlers::sessions::update_session_to_idle),
        )
        .route(
            "/sessions/{name}/sleep",
            post(handlers::sessions::sleep_session),
        )
        .route(
            "/sessions/{name}/cancel",
            post(handlers::sessions::cancel_active_response),
        )
        .route(
            "/sessions/{name}/wake",
            post(handlers::sessions::wake_session),
        )
        .route(
            "/sessions/{name}/runtime",
            get(handlers::sessions::get_session_runtime),
        )
        .route(
            "/sessions/{name}/branch",
            post(handlers::sessions::branch_session),
        )
        .route(
            "/sessions/{name}/context",
            get(handlers::sessions::get_session_context),
        )
        .route(
            "/sessions/{name}/context/clear",
            post(handlers::sessions::clear_session_context),
        )
        .route(
            "/sessions/{name}/context/compact",
            post(handlers::sessions::compact_session_context),
        )
        .route(
            "/sessions/{name}/context/usage",
            post(handlers::sessions::update_session_context_usage),
        )
        .route(
            "/sessions/{name}/publish",
            post(handlers::sessions::publish_session),
        )
        .route(
            "/sessions/{name}/unpublish",
            post(handlers::sessions::unpublish_session),
        )
        .route(
            "/sessions/{name}",
            delete(handlers::sessions::delete_session),
        )
        // Message endpoints removed in favor of Responses
        // Response endpoints (composite model)
        .route(
            "/sessions/{name}/responses",
            get(handlers::responses::list_responses),
        )
        .route(
            "/sessions/{name}/responses",
            post(handlers::responses::create_response),
        )
        .route(
            "/sessions/{name}/responses/{id}",
            get(handlers::responses::get_response_by_id).put(handlers::responses::update_response),
        )
        .route(
            "/sessions/{name}/responses/count",
            get(handlers::responses::get_response_count),
        )
        // Global response lookup by id
        .route(
            "/responses/{id}",
            get(handlers::responses::get_response_global_by_id),
        )
        // Session files (read-only)
        .route(
            "/sessions/{name}/files/read/{*path}",
            get(handlers::sessions::read_session_file),
        )
        .route(
            "/sessions/{name}/files/metadata/{*path}",
            get(handlers::sessions::get_session_file_metadata),
        )
        .route(
            "/sessions/{name}/files/list/{*path}",
            get(handlers::sessions::list_session_files),
        )
        .route(
            "/sessions/{name}/files/list",
            get(handlers::sessions::list_session_files_root),
        )
        .route(
            "/sessions/{name}/files/delete/{*path}",
            delete(handlers::sessions::delete_session_file),
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
