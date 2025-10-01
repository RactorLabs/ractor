use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::time::Duration;

pub struct Config {
    pub twitter_bearer_token: String,
    pub twitter_user_id: String,
    pub twitter_api_base: String,
    pub poll_interval: Duration,
    pub raworc_host_url: String,
    pub raworc_admin_token: String,
    pub initial_since_id: Option<String>,
    pub twitter_api_key: Option<String>,
    pub twitter_api_secret: Option<String>,
    pub twitter_access_token: Option<String>,
    pub twitter_access_token_secret: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let twitter_bearer_token = env::var("RAWORC_APPS_ASKREPO_TWITTER_BEARER_TOKEN")
            .context("RAWORC_APPS_ASKREPO_TWITTER_BEARER_TOKEN is required")?;
        let twitter_user_id = env::var("RAWORC_APPS_ASKREPO_TWITTER_USER_ID")
            .context("RAWORC_APPS_ASKREPO_TWITTER_USER_ID is required")?;
        let twitter_api_base = env::var("RAWORC_APPS_ASKREPO_TWITTER_API_BASE")
            .unwrap_or_else(|_| "https://api.x.com".to_string());
        let poll_interval_secs: u64 = env::var("RAWORC_APPS_ASKREPO_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|val| val.parse().ok())
            .unwrap_or(90);
        let raworc_host_url = env::var("RAWORC_HOST_URL").context("RAWORC_HOST_URL is required")?;
        let raworc_admin_token = env::var("RAWORC_APPS_ASKREPO_ADMIN_TOKEN")
            .context("RAWORC_APPS_ASKREPO_ADMIN_TOKEN is required")?;
        let initial_since_id = env::var("RAWORC_APPS_ASKREPO_TWITTER_SINCE_ID").ok();
        let twitter_api_key =
            env_fallback("RAWORC_APPS_ASKREPO_TWITTER_API_KEY", "TWITTER_API_KEY");
        let twitter_api_secret = env_fallback(
            "RAWORC_APPS_ASKREPO_TWITTER_API_SECRET",
            "TWITTER_API_SECRET",
        );
        let twitter_access_token = env_fallback(
            "RAWORC_APPS_ASKREPO_TWITTER_ACCESS_TOKEN",
            "TWITTER_ACCESS_TOKEN",
        );
        let twitter_access_token_secret = env_fallback(
            "RAWORC_APPS_ASKREPO_TWITTER_ACCESS_TOKEN_SECRET",
            "TWITTER_ACCESS_TOKEN_SECRET",
        );

        Ok(Self {
            twitter_bearer_token,
            twitter_user_id,
            twitter_api_base,
            poll_interval: Duration::from_secs(poll_interval_secs.max(10)),
            raworc_host_url,
            raworc_admin_token,
            initial_since_id,
            twitter_api_key,
            twitter_api_secret,
            twitter_access_token,
            twitter_access_token_secret,
        })
    }

    pub fn agent_secrets(&self) -> HashMap<String, String> {
        let mut secrets = HashMap::new();
        secrets.insert(
            "TWITTER_BEARER_TOKEN".to_string(),
            self.twitter_bearer_token.clone(),
        );
        if let Some(value) = self.twitter_api_key.as_ref() {
            secrets.insert("TWITTER_API_KEY".to_string(), value.clone());
        }
        if let Some(value) = self.twitter_api_secret.as_ref() {
            secrets.insert("TWITTER_API_SECRET".to_string(), value.clone());
        }
        if let Some(value) = self.twitter_access_token.as_ref() {
            secrets.insert("TWITTER_ACCESS_TOKEN".to_string(), value.clone());
        }
        if let Some(value) = self.twitter_access_token_secret.as_ref() {
            secrets.insert("TWITTER_ACCESS_TOKEN_SECRET".to_string(), value.clone());
        }
        secrets
    }
}

fn env_fallback(primary: &str, secondary: &str) -> Option<String> {
    env::var(primary)
        .ok()
        .or_else(|| env::var(secondary).ok())
        .filter(|value| !value.is_empty())
}
