use anyhow::Result;

#[path = "../shared/mod.rs"]
mod shared;
#[path = "../server/mod.rs"]
mod server;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize service logging
    let _ = shared::logging::init_service_logging("/app/logs", "raworc_server");
    
    // Run the API server
    server::rest::server::run_rest_server().await
}