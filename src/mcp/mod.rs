use std::fs;

use axum::Router;
use tracing::{error, info, warn};

use crate::mcp::routes::create_router;
use crate::mcp::state::{init_pool, McpState};
use crate::shared::logging;

pub mod client;
pub mod error;
pub mod handlers;
pub mod models;
pub mod output_schemas;
pub mod routes;
pub mod state;

pub async fn run_server() -> anyhow::Result<()> {
    // Initialize service logging
    let _ = logging::init_service_logging("/app/logs", "tsbx_mcp");

    let database_url =
        std::env::var("MCP_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL"))?;
    let host = std::env::var("MCP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("MCP_PORT").unwrap_or_else(|_| "9400".to_string());

    info!("Starting TSBX MCP service...");
    info!("Connecting to database...");
    let pool = init_pool(&database_url).await?;
    let state = McpState::new(pool);

    info!("Building routes...");
    let app: Router = create_router(state.clone());

    let bind_addr = format!("{host}:{port}");
    info!("Binding MCP service to {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    info!("MCP service ready at http://{}:{}/api/v0/mcp", host, port);

    let pid = std::process::id();
    let pid_file = "/tmp/tsbx_mcp.pid";
    if let Err(e) = fs::write(pid_file, pid.to_string()) {
        warn!("Could not write PID file: {}", e);
    }
    let pid_cleanup = pid_file.to_string();
    ctrlc::set_handler(move || {
        info!("Shutting down TSBX MCP...");
        let _ = fs::remove_file(&pid_cleanup);
        std::process::exit(0);
    })?;

    let result = axum::serve(listener, app).await;
    let _ = fs::remove_file(pid_file);

    if let Err(e) = result {
        error!("MCP server error: {}", e);
        return Err(anyhow::anyhow!(e));
    }

    Ok(())
}
