mod agent_manager;
pub mod docker_manager;

pub use agent_manager::AgentManager;

use anyhow::Result;

pub async fn run() -> Result<()> {
    tracing::info!("Starting Raworc Controller...");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Resolve OLLAMA_HOST or use default inside Docker network
    let ollama_host =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://ollama:11434".to_string());
    tracing::info!("Using OLLAMA_HOST: {}", ollama_host);

    // Initialize agent manager and run
    let agent_manager = AgentManager::new(&database_url).await?;
    if let Err(e) = agent_manager.run().await {
        tracing::error!("Agent manager error: {}", e);
    }
    Ok(())
}
