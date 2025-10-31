use anyhow::Result;

#[path = "../api/mod.rs"]
mod api;
#[path = "../shared/mod.rs"]
mod shared;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize service logging
    let _ = shared::logging::init_service_logging("/app/logs", "tsbx_api");

    // Run the API server
    api::rest::api::run_rest_server().await
}
