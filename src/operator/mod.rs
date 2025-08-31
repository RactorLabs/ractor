pub mod docker_manager;
mod session_manager;

pub use session_manager::SessionManager;

use anyhow::Result;

pub async fn run() -> Result<()> {
    tracing::info!("Starting Raworc Operator...");
    
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    
    // Initialize session manager
    let session_manager = SessionManager::new(&database_url).await?;
    
    // Run session manager
    if let Err(e) = session_manager.run().await {
        tracing::error!("Session manager error: {}", e);
    }
    
    Ok(())
}