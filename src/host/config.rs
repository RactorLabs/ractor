use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub session_name: String,  // Changed from session_id in v0.4.0
    pub api_url: String,
    pub api_token: String,
    pub polling_interval: Duration,
}