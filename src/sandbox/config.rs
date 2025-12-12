use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub sandbox_id: String,
    pub api_url: String,
    pub api_token: String,
    pub polling_interval: Duration,
    pub mcp_url: Option<String>,
}
