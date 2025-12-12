use axum::{
    routing::{get, post},
    Router,
};

use crate::mcp::handlers::{
    get_invocation, invoke_tool, list_servers, list_tools, sync_server, upsert_server,
};
use crate::mcp::state::McpState;

pub fn create_router(state: McpState) -> Router {
    Router::new()
        .route("/api/v0/mcp/servers", get(list_servers).post(upsert_server))
        .route("/api/v0/mcp/servers/{id}/sync", post(sync_server))
        .route("/api/v0/mcp/tools", get(list_tools))
        .route("/api/v0/mcp/invoke", post(invoke_tool))
        .route("/api/v0/mcp/invocations/{id}", get(get_invocation))
        .with_state(state)
}
