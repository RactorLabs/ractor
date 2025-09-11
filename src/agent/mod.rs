// Agent (Computer Use Agent) modules
mod api;
mod builtin_tools;
mod config;
mod error;
mod guardrails;
mod message_handler;
mod ollama;
mod tool_registry;
mod tools;

use anyhow::Result;
use std::sync::Arc;
use tracing::{error, info, warn};

pub async fn run(api_url: &str, agent_name: &str) -> Result<()> {
    tracing::info!("Starting Raworc Agent...");
    tracing::info!("Connecting to API: {}", api_url);
    tracing::info!("Agent Name: {}", agent_name);

    // Log which principal this Agent is running as
    if let Ok(principal) = std::env::var("RAWORC_PRINCIPAL") {
        let principal_type =
            std::env::var("RAWORC_PRINCIPAL_TYPE").unwrap_or_else(|_| "Unknown".to_string());
        tracing::info!("Running as principal: {} ({})", principal, principal_type);
    }

    // Use RAWORC_TOKEN from environment (user's token set as secret)
    let api_token = std::env::var("RAWORC_TOKEN")
        .map_err(|_| anyhow::anyhow!("RAWORC_TOKEN environment variable is required"))?;

    // Debug: Log the RAWORC token being used (partially masked for security)
    let masked_token = if api_token.len() > 20 {
        format!(
            "{}...{}",
            &api_token[..20],
            &api_token[api_token.len() - 8..]
        )
    } else {
        "<too-short>".to_string()
    };
    tracing::info!("Using RAWORC_TOKEN: {}", masked_token);

    // Resolve Ollama host from environment; required (no default)
    let mut ollama_host = std::env::var("OLLAMA_HOST")
        .map_err(|_| anyhow::anyhow!("OLLAMA_HOST environment variable is required"))?;
    // Be tolerant of missing scheme in OLLAMA_HOST (e.g., "127.0.0.1:11434")
    if !(ollama_host.starts_with("http://") || ollama_host.starts_with("https://")) {
        ollama_host = format!("http://{}", ollama_host);
    }
    tracing::info!("Using OLLAMA_HOST: {}", ollama_host);

    // Initialize configuration
    let config = Arc::new(config::Config {
        agent_name: agent_name.to_string(),
        api_url: api_url.to_string(),
        api_token,
        polling_interval: std::time::Duration::from_secs(2),
    });

    // Initialize API client
    let api_client = Arc::new(api::RaworcClient::new(config.clone()));

    // Initialize Ollama client
    let ollama_client = match ollama::OllamaClient::new(&ollama_host) {
        Ok(client) => client,
        Err(e) => {
            tracing::error!("Failed to initialize Ollama client: {}", e);
            return Err(anyhow::anyhow!("Failed to initialize Ollama client: {}", e));
        }
    };
    let ollama_client = Arc::new(ollama_client);

    // Initialize guardrails
    let guardrails = Arc::new(guardrails::Guardrails::new());

    // Initialize agent directories
    let agent_dirs = ["/agent", "/agent/code", "/agent/secrets", "/agent/content"];

    for dir in agent_dirs.iter() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            warn!("Failed to create directory {}: {}", dir, e);
        }
    }

    // No separate content preview server; content is published via raworc-content.

    // Wait for and execute setup script if it becomes available
    let setup_script = std::path::Path::new("/agent/code/setup.sh");

    // Check if a setup script is expected based on environment variable
    let has_setup_script = std::env::var("RAWORC_HAS_SETUP").is_ok();

    if has_setup_script {
        // Setup script is expected, wait up to 2 seconds for it to be written by controller
        info!("Setup script expected, waiting for it to be created...");
        let mut attempts = 0;
        let max_attempts = 4; // 2 seconds with 500ms intervals

        while !setup_script.exists() && attempts < max_attempts {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            attempts += 1;
        }
    } else {
        // No setup script expected, check once and proceed
        info!("No setup script expected, checking once...");
    }

    if setup_script.exists() {
        info!("Executing setup script: /agent/code/setup.sh");
        match std::process::Command::new("bash")
            .arg("/agent/code/setup.sh")
            .current_dir("/agent")
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    info!("Setup script executed successfully");
                    if !output.stdout.is_empty() {
                        info!("Setup stdout: {}", String::from_utf8_lossy(&output.stdout));
                    }
                } else {
                    error!(
                        "Setup script failed with exit code: {:?}",
                        output.status.code()
                    );
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
        info!("No setup script found at /agent/code/setup.sh");
    }

    // Set working directory to agent directory
    if let Err(e) = std::env::set_current_dir("/agent") {
        warn!("Failed to set working directory to /agent: {}", e);
    } else {
        info!("Set working directory to /agent");
    }

    // Initialize message handler
    let message_handler = message_handler::MessageHandler::new(
        api_client.clone(),
        ollama_client.clone(),
        guardrails.clone(),
    );

    // Initialize processed message tracking to prevent reprocessing on restore
    if let Err(e) = message_handler.initialize_processed_tracking().await {
        warn!(
            "Failed to initialize processed tracking: {}, proceeding anyway",
            e
        );
    }

    // Log published content URL hint
    let base_url = std::env::var("RAWORC_HOST_URL")
        .expect("RAWORC_HOST_URL must be set by the start script")
        .trim_end_matches('/')
        .to_string();
    let host_name = std::env::var("RAWORC_HOST_NAME").unwrap_or_else(|_| "Raworc".to_string());
    let operator_url = format!("{}", base_url);
    let api_url = format!("{}/api", base_url);
    let published_url = format!("{}/content/{}/", base_url, agent_name);
    info!("{} environment detected", host_name);
    info!("Operator: {}", operator_url);
    info!("API: {}", api_url);
    info!("Content folder: /agent/content/ - Publish when ready to share");
    info!("Published content (when published): {}", published_url);

    info!("Setting agent to idle to start timeout...");

    // Set agent to idle after initialization to start timeout
    if let Err(e) = api_client.update_agent_to_idle().await {
        warn!("Failed to set agent to idle after initialization: {}", e);
    } else {
        info!("Agent set to idle - timeout started");
    }

    info!("Starting message polling loop...");

    // Main polling loop with comprehensive error handling
    loop {
        match message_handler.poll_and_process().await {
            Ok(count) => {
                if count > 0 {
                    info!("Processed {} messages", count);
                }
            }
            Err(e) => {
                error!("Error processing messages: {}", e);

                // Send error as message to user instead of crashing
                let error_message = format!("Agent encountered an error: {}", e);
                if let Err(send_err) = api_client.send_message(error_message, None).await {
                    error!("Failed to send error message to user: {}", send_err);
                } else {
                    info!("Sent error message to user, continuing operation");
                }

                // Continue polling - agent should never die silently
            }
        }

        // Wait before next poll
        tokio::time::sleep(config.polling_interval).await;
    }
}

    // Content preview server removed.
