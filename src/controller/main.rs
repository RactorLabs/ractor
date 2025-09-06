use anyhow::Result;

#[path = "../shared/logging.rs"]
mod logging;
#[path = "../controller/mod.rs"]
mod controller;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize service logging
    let _ = logging::init_service_logging("/app/logs", "raworc_controller");

    // Run the controller service
    controller::run().await
}
