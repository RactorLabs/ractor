use anyhow::Result;

#[path = "../shared/logging.rs"]
mod logging;
#[path = "../operator/mod.rs"]
mod operator;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize service logging
    let _ = logging::init_service_logging("/app/logs", "raworc_operator");
    
    // Run the operator service
    operator::run().await
}