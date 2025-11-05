pub mod docker_manager;
mod sandbox_manager;

pub use sandbox_manager::SandboxManager;

use anyhow::Result;

pub async fn run() -> Result<()> {
    tracing::info!("Starting TaskSandbox Controller...");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Resolve OLLAMA_HOST or use default inside Docker network
    let ollama_host =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://ollama:11434".to_string());
    tracing::info!("Using OLLAMA_HOST: {}", ollama_host);

    // Initialize sandbox manager and run
    let sandbox_manager = SandboxManager::new(&database_url).await?;
    if let Err(e) = sandbox_manager.run().await {
        tracing::error!("Sandbox manager error: {}", e);
    }
    Ok(())
}
