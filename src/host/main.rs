use anyhow::Result;
use clap::Parser;

#[path = "../host/mod.rs"]
mod host;
#[path = "../shared/logging.rs"]
mod logging;

#[derive(Parser)]
#[command(name = "raworc-host")]
#[command(about = "Raworc Host - Computer Use Agent inside session containers")]
struct Args {
    /// API server URL
    #[arg(long, env = "RAWORC_API_URL")]
    api_url: String,

    /// Session Name
    #[arg(long, env = "RAWORC_SESSION_NAME")]
    session_name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize service logging
    let _ = logging::init_service_logging("/app/logs", "raworc_host");

    // Run the Host (Computer Use Agent)
    host::run(&args.api_url, &args.session_name).await
}
