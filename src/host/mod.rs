// Host agent modules
mod agent_manager;
mod api;
mod claude;
mod config;
mod error;
mod guardrails;
mod message_handler;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error, warn};

pub async fn run(api_url: &str, session_id: &str, api_key: &str) -> Result<()> {
    tracing::info!("Starting Raworc Host Agent...");
    tracing::info!("Connecting to API: {}", api_url);
    tracing::info!("Session ID: {}", session_id);
    
    // Use RAWORC_API_TOKEN from environment if available (set by operator), otherwise use provided api_key
    let api_token = std::env::var("RAWORC_API_TOKEN").unwrap_or_else(|_| api_key.to_string());
    
    // Get space name from environment (set by operator)
    let _space = std::env::var("RAWORC_SPACE_ID").unwrap_or_else(|_| "default".to_string());
    
    // Get Claude API key from environment - ANTHROPIC_API_KEY is required
    let claude_api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY environment variable is required"))?;
    
    // Initialize configuration
    let config = Arc::new(config::Config {
        session_id: session_id.to_string(),
        api_url: api_url.to_string(),
        api_token,
        polling_interval: std::time::Duration::from_secs(2),
    });
    
    // Initialize API client
    let api_client = Arc::new(api::RaworcClient::new(config.clone()));
    
    // Initialize Claude client
    let claude_client = match claude::ClaudeClient::new(&claude_api_key) {
        Ok(client) => Arc::new(client),
        Err(e) => {
            tracing::error!("Failed to initialize Claude client: {}", e);
            return Err(anyhow::anyhow!("Failed to initialize Claude client: {}", e));
        }
    };
    
    // Initialize guardrails
    let guardrails = Arc::new(guardrails::Guardrails::new());
    
    // Agent client no longer needed - using on-demand execution instead
    
    
    // No preemptive deployment needed for on-demand execution
    info!("Using on-demand agent execution - agents will be prepared when needed");
    
    // Initialize session directories
    let session_dirs = [
        "/session",
        "/session/agents", 
        "/session/cache",
        "/session/tmp"
    ];
    
    for dir in session_dirs.iter() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            warn!("Failed to create directory {}: {}", dir, e);
        }
    }
    
    // Initialize agent manager
    let mut agent_manager = agent_manager::AgentManager::new(api_client.clone(), claude_client.clone());
    if let Err(e) = agent_manager.initialize().await {
        warn!("Failed to initialize agent manager: {}, proceeding with Claude-only mode", e);
    }
    let agent_manager = Arc::new(Mutex::new(agent_manager));
    
    // Set working directory to session directory
    if let Err(e) = std::env::set_current_dir("/session") {
        warn!("Failed to set working directory to /session: {}", e);
        if let Err(set_err) = std::env::set_current_dir("/session") {
            warn!("Failed to set working directory after creation: {}", set_err);
        } else {
            info!("Set working directory to /session");
        }
    } else {
        info!("Set working directory to /session");
    }

    // Initialize message handler
    let message_handler = message_handler::MessageHandler::new(
        api_client.clone(),
        claude_client.clone(),
        guardrails.clone(),
        agent_manager.clone(),
    );

    // Initialize processed message tracking to prevent reprocessing on restore
    if let Err(e) = message_handler.initialize_processed_tracking().await {
        warn!("Failed to initialize processed tracking: {}, proceeding anyway", e);
    }
    
    // Using Claude API directly for all processing
    info!("Claude API client ready for message processing");
    
    info!("Host agent initialized, starting message polling loop...");
    
    // No health monitoring needed for on-demand execution
    
    // Main polling loop with robust error handling
    loop {
        info!("Starting polling cycle...");
        
        match message_handler.poll_and_process().await {
            Ok(count) => {
                if count > 0 {
                    info!("Processed {} messages", count);
                } else {
                    info!("Polling: no new messages to process");
                }
            }
            Err(e) => {
                error!("Error processing messages: {}", e);
                // Continue polling even on errors
            }
        }
        
        info!("Waiting {} seconds before next poll...", config.polling_interval.as_secs());
        
        // Wait before next poll
        tokio::time::sleep(config.polling_interval).await;
    }
}

