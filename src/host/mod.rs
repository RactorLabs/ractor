// Host (Computer Use Agent) modules
mod api;
mod claude;
mod config;
mod error;
mod guardrails;
mod message_handler;

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, error, warn};

pub async fn run(api_url: &str, session_id: &str) -> Result<()> {
    tracing::info!("Starting Raworc Host...");
    tracing::info!("Connecting to API: {}", api_url);
    tracing::info!("Session ID: {}", session_id);
    
    // Log which principal this Host is running as
    if let Ok(principal) = std::env::var("RAWORC_PRINCIPAL") {
        let principal_type = std::env::var("RAWORC_PRINCIPAL_TYPE").unwrap_or_else(|_| "Unknown".to_string());
        tracing::info!("Running as principal: {} ({})", principal, principal_type);
    }
    
    // Use RAWORC_TOKEN from environment (user's token set as secret)
    let api_token = std::env::var("RAWORC_TOKEN")
        .map_err(|_| anyhow::anyhow!("RAWORC_TOKEN environment variable is required"))?;
    
    // Debug: Log the RAWORC token being used (partially masked for security)
    let masked_token = if api_token.len() > 20 {
        format!("{}...{}", &api_token[..20], &api_token[api_token.len()-8..])
    } else {
        "<too-short>".to_string()
    };
    tracing::info!("Using RAWORC_TOKEN: {}", masked_token);
    
    // Get Claude API key from environment - ANTHROPIC_API_KEY is required
    let claude_api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY environment variable is required"))?;
    
    // Debug: Log the API key being used (partially masked for security)
    let masked_key = if claude_api_key.len() > 10 {
        format!("{}...{}", &claude_api_key[..10], &claude_api_key[claude_api_key.len()-4..])
    } else {
        "<too-short>".to_string()
    };
    tracing::info!("Using ANTHROPIC_API_KEY: {}", masked_key);
    
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
    
    // Initialize session directories
    let session_dirs = [
        "/session",
        "/session/code", 
        "/session/data",
        "/session/secrets"
    ];
    
    for dir in session_dirs.iter() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            warn!("Failed to create directory {}: {}", dir, e);
        }
    }
    
    // Wait for and execute setup script if it becomes available
    let setup_script = std::path::Path::new("/session/code/setup.sh");
    
    // Wait up to 10 seconds for the setup script to be written by operator
    let mut attempts = 0;
    let max_attempts = 20; // 10 seconds with 500ms intervals
    
    while !setup_script.exists() && attempts < max_attempts {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        attempts += 1;
    }
    
    if setup_script.exists() {
        info!("Executing setup script: /session/code/setup.sh");
        match std::process::Command::new("bash")
            .arg("/session/code/setup.sh")
            .current_dir("/session")
            .output() 
        {
            Ok(output) => {
                if output.status.success() {
                    info!("Setup script executed successfully");
                    if !output.stdout.is_empty() {
                        info!("Setup stdout: {}", String::from_utf8_lossy(&output.stdout));
                    }
                } else {
                    error!("Setup script failed with exit code: {:?}", output.status.code());
                    if !output.stderr.is_empty() {
                        error!("Setup stderr: {}", String::from_utf8_lossy(&output.stderr));
                    }
                }
            }
            Err(e) => {
                error!("Failed to execute setup script: {}", e);
            }
        }
    } else {
        info!("No setup script found at /session/code/setup.sh");
    }
    
    // Set working directory to session directory
    if let Err(e) = std::env::set_current_dir("/session") {
        warn!("Failed to set working directory to /session: {}", e);
    } else {
        info!("Set working directory to /session");
    }

    // Initialize message handler
    let message_handler = message_handler::MessageHandler::new(
        api_client.clone(),
        claude_client.clone(),
        guardrails.clone(),
    );

    // Initialize processed message tracking to prevent reprocessing on restore
    if let Err(e) = message_handler.initialize_processed_tracking().await {
        warn!("Failed to initialize processed tracking: {}, proceeding anyway", e);
    }
    
    info!("Host initialized, starting message polling loop...");
    
    // Main polling loop
    loop {
        match message_handler.poll_and_process().await {
            Ok(count) => {
                if count > 0 {
                    info!("Processed {} messages", count);
                }
            }
            Err(e) => {
                error!("Error processing messages: {}", e);
                // Continue polling even on errors
            }
        }
        
        // Wait before next poll
        tokio::time::sleep(config.polling_interval).await;
    }
}

