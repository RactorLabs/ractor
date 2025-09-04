pub mod docker_manager;
mod agent_manager;

pub use agent_manager::AgentManager;

use anyhow::Result;

pub async fn run() -> Result<()> {
    tracing::info!("Starting Raworc Operator...");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Validate required environment variables early
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        tracing::error!("ANTHROPIC_API_KEY environment variable is required but not set");
        return Err(anyhow::anyhow!(
            "ANTHROPIC_API_KEY environment variable is required"
        ));
    }

    tracing::info!("ANTHROPIC_API_KEY validated successfully");

    // Initialize agent manager
    let agent_manager = AgentManager::new(&database_url).await?;

    // Run agent manager
    if let Err(e) = agent_manager.run().await {
        tracing::error!("Agent manager error: {}", e);
    }

    Ok(())
}
