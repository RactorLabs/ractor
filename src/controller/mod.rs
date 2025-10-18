pub mod docker_manager;
mod session_manager;

pub use session_manager::SessionManager;

use anyhow::Result;

pub async fn run() -> Result<()> {
    tracing::info!("Starting Ractor Controller...");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Resolve OLLAMA_HOST or use default inside Docker network
    let ollama_host =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://ollama:11434".to_string());
    tracing::info!("Using OLLAMA_HOST: {}", ollama_host);

    // Initialize session manager and run
    let session_manager = SessionManager::new(&database_url).await?;
    if let Err(e) = session_manager.run().await {
        tracing::error!("Session manager error: {}", e);
    }
    Ok(())
}
