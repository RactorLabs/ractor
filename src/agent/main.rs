use anyhow::Result;
use clap::Parser;

#[path = "../agent/mod.rs"]
mod agent;
#[path = "../shared/logging.rs"]
mod logging;

#[derive(Parser)]
#[command(name = "raworc-agent")]
#[command(about = "Raworc Agent - Computer Use Agent inside agent containers")]
struct Args {
    /// API server URL
    #[arg(long, env = "RAWORC_API_URL")]
    api_url: String,

    /// Agent Name
    #[arg(long, env = "RAWORC_AGENT_NAME")]
    agent_name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize service logging (write alongside agent data)
    let _ = logging::init_service_logging("/agent/logs", "raworc_agent");

    // Run the Agent (Computer Use Agent) with comprehensive error handling
    loop {
        match agent::run(&args.api_url, &args.agent_name).await {
            Ok(()) => {
                // Agent run completed successfully (should not happen in normal operation)
                tracing::warn!("Agent run completed unexpectedly, restarting...");
            }
            Err(e) => {
                tracing::error!("Agent crashed with error: {}", e);
                tracing::error!("Attempting to restart agent in 5 seconds...");
                
                // Wait before restart to prevent tight crash loops
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
        
        tracing::info!("Restarting agent...");
    }
}
