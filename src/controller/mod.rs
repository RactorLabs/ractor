pub mod docker_manager;
mod sandbox_manager;

pub use sandbox_manager::SandboxManager;

use anyhow::Result;

pub async fn run() -> Result<()> {
    tracing::info!("Starting TSBX Controller...");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let inference_url =
        std::env::var("TSBX_INFERENCE_URL").expect("TSBX_INFERENCE_URL must be set");
    if inference_url.trim().is_empty() {
        panic!("TSBX_INFERENCE_URL must not be empty");
    }
    tracing::info!("Using TSBX_INFERENCE_URL: {}", inference_url);

    // Initialize sandbox manager and run
    let sandbox_manager = SandboxManager::new(&database_url).await?;
    if let Err(e) = sandbox_manager.run().await {
        tracing::error!("Sandbox manager error: {}", e);
    }
    Ok(())
}
