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

    // Initialize service logging
    let _ = logging::init_service_logging("/app/logs", "raworc_agent");

    // Run the Agent (Computer Use Agent)
    agent::run(&args.api_url, &args.agent_name).await
}
