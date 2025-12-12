use anyhow::Result;

#[path = "mod.rs"]
mod mcp;
#[path = "../shared/mod.rs"]
mod shared;

#[tokio::main]
async fn main() -> Result<()> {
    mcp::run_server().await
}
