use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use reqwest::{header, Client, StatusCode, Url};
use serde::Serialize;
use std::collections::HashMap;
use tracing::{debug, trace};
use urlencoding::encode;

pub struct RaworcClient {
    http: Client,
    base_url: Url,
}

impl RaworcClient {
    pub fn new(config: &Config) -> Result<Self> {
        let http = Client::builder()
            .user_agent("raworc-apps-askrepo/0.1")
            .default_headers(Self::default_headers(&config.raworc_admin_token)?)
            .build()
            .context("failed to build Raworc reqwest client")?;

        let mut base_url =
            Url::parse(&config.raworc_host_url).context("RAWORC_HOST_URL is not a valid URL")?;
        if base_url.path().is_empty() {
            base_url.set_path("/");
        }

        Ok(Self { http, base_url })
    }

    fn default_headers(token: &str) -> Result<header::HeaderMap> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {}", token))
                .context("invalid RAWORC admin token for header")?,
        );
        Ok(headers)
    }

    fn agent_url(&self, name: &str) -> Result<Url> {
        self.base_url
            .join(&format!("/api/v0/agents/{}", encode(name)))
            .context("failed to construct agent URL")
    }

    pub async fn agent_exists(&self, name: &str) -> Result<bool> {
        let url = self.agent_url(name)?;
        trace!(%url, "checking for existing agent");
        let res = self
            .http
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("failed to query Raworc API for agent '{}'", name))?;
        match res.status() {
            StatusCode::OK => Ok(true),
            StatusCode::NOT_FOUND => Ok(false),
            status => {
                let body = res.text().await.unwrap_or_default();
                Err(anyhow!(
                    "Raworc API returned {} while checking agent existence (body: {})",
                    status,
                    body
                ))
            }
        }
    }

    pub async fn create_agent(&self, payload: &NewAgentPayload) -> Result<()> {
        let url = self
            .base_url
            .join("/api/v0/agents")
            .context("failed to build create-agent URL")?;

        trace!(%url, agent = %payload.name, "creating agent");

        let response = self
            .http
            .post(url.clone())
            .json(payload)
            .send()
            .await
            .with_context(|| {
                format!(
                    "failed to submit create-agent request for '{}'",
                    payload.name
                )
            })?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Raworc API returned {} when creating agent (body: {})",
                status,
                body
            ));
        }

        debug!(agent = %payload.name, "created agent via Raworc API");
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct NewAgentPayload {
    pub metadata: serde_json::Value,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub secrets: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_timeout_seconds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub busy_timeout_seconds: Option<i32>,
}

impl NewAgentPayload {
    pub fn new(name: String, metadata: serde_json::Value) -> Self {
        Self {
            metadata,
            name,
            description: None,
            tags: Vec::new(),
            instructions: None,
            prompt: None,
            setup: None,
            secrets: HashMap::new(),
            idle_timeout_seconds: None,
            busy_timeout_seconds: None,
        }
    }

    pub fn with_description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_instructions(mut self, instructions: String) -> Self {
        self.instructions = Some(instructions);
        self
    }

    pub fn with_prompt(mut self, prompt: String) -> Self {
        self.prompt = Some(prompt);
        self
    }

    pub fn with_idle_timeout(mut self, secs: Option<i32>) -> Self {
        self.idle_timeout_seconds = secs;
        self
    }

    pub fn with_busy_timeout(mut self, secs: Option<i32>) -> Self {
        self.busy_timeout_seconds = secs;
        self
    }

    pub fn with_secrets(mut self, secrets: HashMap<String, String>) -> Self {
        self.secrets = secrets;
        self
    }
}
