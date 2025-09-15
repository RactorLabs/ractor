mod agent_manager;
pub mod docker_manager;

pub use agent_manager::AgentManager;

use anyhow::Result;

pub async fn run() -> Result<()> {
    tracing::info!("Starting Raworc Controller...");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Resolve RAWORC_GPT_URL or use default inside Docker network
    let gpt_url =
        std::env::var("RAWORC_GPT_URL").unwrap_or_else(|_| "http://raworc_gpt:6000".to_string());
    tracing::info!("Using RAWORC_GPT_URL: {}", gpt_url);

    // Initialize agent manager
    let agent_manager = AgentManager::new(&database_url).await?;

    // Run agent manager
    if let Err(e) = agent_manager.run().await {
        tracing::error!("Agent manager error: {}", e);
    }

    Ok(())
}
