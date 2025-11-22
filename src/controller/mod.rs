pub mod docker_manager;
mod sandbox_manager;

pub use crate::shared::config as shared_config;
pub use crate::shared::inference as shared_inference;
pub use crate::shared::models::task as shared_task;

pub use sandbox_manager::SandboxManager;

use anyhow::{anyhow, Result};
use std::sync::Arc;

pub async fn run() -> Result<()> {
    tracing::info!("Starting TSBX Controller...");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let config_path = shared_config::resolve_config_path();
    let config = Arc::new(
        shared_config::TsbxConfig::load_from_path(&config_path)
            .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?,
    );
    tracing::info!("Loaded config from {}", config_path.display());

    // Initialize sandbox manager and run
    let inference_registry = Arc::new(
        config
            .build_inference_registry()
            .map_err(|e| anyhow!("Failed to build inference registry: {}", e))?,
    );

    let sandbox_manager = SandboxManager::new(&database_url, config, inference_registry).await?;
    if let Err(e) = sandbox_manager.run().await {
        tracing::error!("Sandbox manager error: {}", e);
    }
    Ok(())
}
