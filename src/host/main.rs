use anyhow::Result;
use clap::Parser;

#[path = "../shared/logging.rs"]
mod logging;
#[path = "../host/mod.rs"]
mod host;

#[derive(Parser)]
#[command(name = "raworc-host")]
#[command(about = "Raworc Host - Computer Use Agent inside session containers")]
struct Args {
    /// API server URL
    #[arg(long, env = "RAWORC_API_URL")]
    api_url: String,
    
    /// Session ID
    #[arg(long, env = "RAWORC_SESSION_ID")]
    session_id: String,
    
    /// API Key for authentication
    #[arg(long, env = "RAWORC_API_KEY")]
    api_key: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize service logging
    let _ = logging::init_service_logging("/app/logs", "raworc_host");
    
    // Run the Host (Computer Use Agent)
    host::run(&args.api_url, &args.session_id, &args.api_key).await
}