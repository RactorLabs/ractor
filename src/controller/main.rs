use anyhow::Result;

#[path = "../controller/mod.rs"]
mod controller;
#[path = "../shared/logging.rs"]
mod logging;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize service logging
    let _ = logging::init_service_logging("/app/logs", "raworc_controller");

    // Run the controller service
    controller::run().await
}
