use anyhow::Result;
use clap::Parser;

#[path = "../shared/logging.rs"]
mod logging;
#[path = "../sandbox/mod.rs"]
mod sandbox;

#[derive(Parser)]
#[command(name = "tsbx-sandbox")]
#[command(about = "TaskSandbox Sandbox - Computer Use Sandbox inside sandbox containers")]
struct Args {
    /// API server URL
    #[arg(long, env = "TSBX_API_URL")]
    api_url: String,

    /// Sandbox ID (UUID)
    #[arg(long, env = "SANDBOX_ID")]
    sandbox_id: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize service logging (write alongside sandbox data)
    let _ = logging::init_service_logging("/sandbox/logs", "tsbx_sandbox");

    // Run the Sandbox (Computer Use Sandbox) with comprehensive error handling
    loop {
        match sandbox::run(&args.api_url, &args.sandbox_id).await {
            Ok(()) => {
                // Sandbox run completed successfully (should not happen in normal operation)
                tracing::warn!("Sandbox run completed unexpectedly, restarting...");
            }
            Err(e) => {
                tracing::error!("Sandbox crashed with error: {}", e);
                tracing::error!("Attempting to restart sandbox in 5 seconds...");

                // Wait before restart to prevent tight crash loops
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }

        tracing::info!("Restarting sandbox...");
    }
}
