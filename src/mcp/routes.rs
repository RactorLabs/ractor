use axum::{
    routing::{get, post},
    Router,
};

use crate::mcp::handlers::{
    batch_invoke, create_tool_example, get_invocation, invoke_tool, list_servers,
    list_tool_examples, list_tools, live_search_tools, sync_server, upsert_server,
};
use crate::mcp::state::McpState;

pub fn create_router(state: McpState) -> Router {
    Router::new()
        .route("/api/v0/mcp/servers", get(list_servers).post(upsert_server))
        .route("/api/v0/mcp/servers/{id}/sync", post(sync_server))
        .route("/api/v0/mcp/tools", get(list_tools))
        .route("/api/v0/mcp/tools/search", get(list_tools))
        .route("/api/v0/mcp/tools/live_search", get(live_search_tools))
        .route(
            "/api/v0/mcp/tools/{id}/examples",
            get(list_tool_examples).post(create_tool_example),
        )
        .route("/api/v0/mcp/invoke", post(invoke_tool))
        .route("/api/v0/mcp/invoke/batch", post(batch_invoke))
        .route("/api/v0/mcp/invocations/{id}", get(get_invocation))
        .with_state(state)
}
