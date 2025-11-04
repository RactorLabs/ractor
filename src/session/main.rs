use anyhow::Result;
use clap::Parser;

#[path = "../shared/logging.rs"]
mod logging;
#[path = "../session/mod.rs"]
mod session;

#[derive(Parser)]
#[command(name = "tsbx-session")]
#[command(about = "TaskSandbox Session - Computer Use Session inside session containers")]
struct Args {
    /// API server URL
    #[arg(long, env = "TSBX_API_URL")]
    api_url: String,

    /// Session ID (UUID)
    #[arg(long, env = "SESSION_ID")]
    session_id: String,

    /// Session Name
    #[arg(long, env = "TSBX_SESSION_NAME")]
    session_name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize service logging (write alongside session data)
    let _ = logging::init_service_logging("/session/logs", "tsbx_session");

    // Run the Session (Computer Use Session) with comprehensive error handling
    loop {
        match session::run(&args.api_url, &args.session_id, &args.session_name).await {
            Ok(()) => {
                // Session run completed successfully (should not happen in normal operation)
                tracing::warn!("Session run completed unexpectedly, restarting...");
            }
            Err(e) => {
                tracing::error!("Session crashed with error: {}", e);
                tracing::error!("Attempting to restart session in 5 seconds...");

                // Wait before restart to prevent tight crash loops
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }

        tracing::info!("Restarting session...");
    }
}
