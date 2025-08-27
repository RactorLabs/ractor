pub mod docker_manager;
mod session_manager;
mod build_manager;
pub mod space_builder;

pub use session_manager::SessionManager;
pub use build_manager::BuildManager;

use anyhow::Result;

pub async fn run() -> Result<()> {
    tracing::info!("Starting Raworc Operator...");
    
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    
    // Initialize managers
    let session_manager = SessionManager::new(&database_url).await?;
    let build_manager = {
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;
        BuildManager::new(std::sync::Arc::new(pool))
    };
    
    // Run both managers concurrently
    let session_task = tokio::spawn(async move {
        if let Err(e) = session_manager.run().await {
            tracing::error!("Session manager error: {}", e);
        }
    });
    
    let build_task = tokio::spawn(async move {
        loop {
            if let Err(e) = build_manager.poll_and_process_tasks().await {
                tracing::error!("Build manager error: {}", e);
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });
    
    // Wait for either task to complete (they shouldn't under normal operation)
    tokio::select! {
        _ = session_task => tracing::error!("Session manager task completed unexpectedly"),
        _ = build_task => tracing::error!("Build manager task completed unexpectedly"),
    }
    
    Ok(())
}