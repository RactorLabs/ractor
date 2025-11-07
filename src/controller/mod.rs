pub mod docker_manager;
mod sandbox_manager;

pub use sandbox_manager::SandboxManager;

use anyhow::Result;

pub async fn run() -> Result<()> {
    tracing::info!("Starting TaskSandbox Controller...");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let inference_url = std::env::var("TSBX_INFERENCE_URL")
        .unwrap_or_else(|_| "https://api.positron.ai/v1".to_string());
    tracing::info!("Using TSBX_INFERENCE_URL: {}", inference_url);

    // Initialize sandbox manager and run
    let sandbox_manager = SandboxManager::new(&database_url).await?;
    if let Err(e) = sandbox_manager.run().await {
        tracing::error!("Sandbox manager error: {}", e);
    }
    Ok(())
}
