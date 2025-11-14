use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use crate::api::rest::{
    handlers, logging_middleware::request_logging_middleware, middleware::auth_middleware,
};
use crate::shared::models::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    // Public routes
    let public_routes = Router::new().route("/version", get(version)).route(
        "/auth/operators/{name}/login",
        post(handlers::operators::login),
    );

    // Protected routes
    let protected_routes = Router::new()
        .route("/auth", get(handlers::auth::me))
        .route("/auth/token", post(handlers::auth::create_token))
        // Security / Blocklist (admin only)
        .route("/auth/blocklist", get(handlers::auth::list_blocked))
        .route(
            "/auth/blocklist/block",
            post(handlers::auth::block_principal),
        )
        .route(
            "/auth/blocklist/unblock",
            post(handlers::auth::unblock_principal),
        )
        // Operator endpoints
        .route("/auth/operators", get(handlers::operators::list_operators))
        .route(
            "/auth/operators",
            post(handlers::operators::create_operator),
        )
        .route(
            "/auth/operators/{name}",
            get(handlers::operators::get_operator),
        )
        .route(
            "/auth/operators/{name}",
            put(handlers::operators::update_operator),
        )
        .route(
            "/auth/operators/{name}",
            delete(handlers::operators::delete_operator),
        )
        .route(
            "/auth/operators/{name}/password",
            put(handlers::operators::update_operator_password),
        )
        // Sandbox endpoints
        .route("/sandboxes", get(handlers::sandboxes::list_sandboxes))
        .route("/sandboxes", post(handlers::sandboxes::create_sandbox))
        .route("/sandboxes/{id}", get(handlers::sandboxes::get_sandbox))
        .route("/sandboxes/{id}", put(handlers::sandboxes::update_sandbox))
        .route(
            "/sandboxes/{id}/state",
            put(handlers::sandboxes::update_sandbox_state),
        )
        .route(
            "/sandboxes/{id}/state/busy",
            post(handlers::sandboxes::update_sandbox_to_busy),
        )
        .route(
            "/sandboxes/{id}/state/idle",
            post(handlers::sandboxes::update_sandbox_to_idle),
        )
        .route(
            "/sandboxes/{id}/runtime",
            get(handlers::sandboxes::get_sandbox_runtime),
        )
        .route(
            "/sandboxes/{id}/top",
            get(handlers::sandboxes::get_sandbox_top),
        )
        .route(
            "/sandboxes/{id}/inference_usage",
            post(handlers::sandboxes::record_inference_usage),
        )
        .route(
            "/sandboxes/{id}",
            delete(handlers::sandboxes::terminate_sandbox),
        )
        // Snapshot endpoints
        .route("/snapshots", get(handlers::snapshots::list_snapshots))
        .route("/snapshots/{id}", get(handlers::snapshots::get_snapshot))
        .route(
            "/snapshots/{id}",
            delete(handlers::snapshots::delete_snapshot),
        )
        .route(
            "/snapshots/{id}/create",
            post(handlers::snapshots::create_from_snapshot),
        )
        .route(
            "/snapshots/{id}/files/read/{*path}",
            get(handlers::snapshots::read_snapshot_file),
        )
        .route(
            "/snapshots/{id}/files/metadata/{*path}",
            get(handlers::snapshots::get_snapshot_file_metadata),
        )
        .route(
            "/snapshots/{id}/files/list/{*path}",
            get(handlers::snapshots::list_snapshot_files),
        )
        .route(
            "/snapshots/{id}/files/list",
            get(handlers::snapshots::list_snapshot_files_root),
        )
        .route(
            "/sandboxes/{id}/snapshots",
            get(handlers::snapshots::list_sandbox_snapshots),
        )
        .route(
            "/sandboxes/{id}/snapshots",
            post(handlers::snapshots::create_snapshot),
        )
        // Task endpoints (composite model)
        .route(
            "/sandboxes/{id}/tasks",
            get(handlers::sandboxes::list_tasks),
        )
        .route(
            "/sandboxes/{id}/tasks",
            post(handlers::sandboxes::create_task),
        )
        .route(
            "/sandboxes/{id}/tasks/{task_id}",
            get(handlers::sandboxes::get_task_by_id).put(handlers::sandboxes::update_task),
        )
        .route(
            "/sandboxes/{id}/tasks/{task_id}/cancel",
            post(handlers::sandboxes::cancel_task),
        )
        .route(
            "/sandboxes/{id}/tasks/count",
            get(handlers::sandboxes::get_task_count),
        )
        // Sandbox files (read-only)
        .route(
            "/sandboxes/{id}/files/read/{*path}",
            get(handlers::sandboxes::read_sandbox_file),
        )
        .route(
            "/sandboxes/{id}/files/metadata/{*path}",
            get(handlers::sandboxes::get_sandbox_file_metadata),
        )
        .route(
            "/sandboxes/{id}/files/list/{*path}",
            get(handlers::sandboxes::list_sandbox_files),
        )
        .route(
            "/sandboxes/{id}/files/list",
            get(handlers::sandboxes::list_sandbox_files_root),
        )
        .route(
            "/sandboxes/{id}/files/delete/{*path}",
            delete(handlers::sandboxes::delete_sandbox_file),
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
