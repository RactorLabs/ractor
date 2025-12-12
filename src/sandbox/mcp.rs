use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct McpTool {
    pub id: String,
    pub server_id: String,
    pub server_name: String,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub metadata: Option<Value>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct InvocationResponse {
    pub id: String,
    pub status: String,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct McpClient {
    base_url: String,
    http: Client,
}

impl McpClient {
    pub fn new(base_url: String) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to build MCP HTTP client")?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
        })
    }

    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        let base = self.base_url.trim_end_matches('/');
        let url = if base.ends_with("/api/v0/mcp") {
            format!("{}/tools", base)
        } else {
            format!("{}/api/v0/mcp/tools", base)
        };
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .context("failed to reach MCP service for tool list")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "MCP tool list failed: status {} body {}",
                status,
                body
            ));
        }

        let tools: Vec<McpTool> = resp
            .json()
            .await
            .context("failed to decode MCP tool list response")?;
        Ok(tools)
    }

    pub async fn invoke(
        &self,
        server_id: Option<&str>,
        server_name: Option<&str>,
        tool: &str,
        arguments: Value,
        sandbox_id: &str,
    ) -> Result<Value> {
        let base = self.base_url.trim_end_matches('/');
        let url = if base.ends_with("/api/v0/mcp") {
            format!("{}/invoke", base)
        } else {
            format!("{}/api/v0/mcp/invoke", base)
        };
        let mut payload = serde_json::Map::new();
        if let Some(id) = server_id {
            payload.insert("server_id".to_string(), Value::String(id.to_string()));
        }
        if server_id.is_none() {
            if let Some(name) = server_name {
                payload.insert("server".to_string(), Value::String(name.to_string()));
            }
        }
        payload.insert("tool".to_string(), Value::String(tool.to_string()));
        payload.insert("arguments".to_string(), arguments);
        payload.insert(
            "sandbox_id".to_string(),
            Value::String(sandbox_id.to_string()),
        );

        let resp = self
            .http
            .post(url)
            .json(&payload)
            .send()
            .await
            .context("failed to reach MCP service for invoke")?;

        let status = resp.status();
        let parsed: InvocationResponse = resp
            .json()
            .await
            .context("failed to decode MCP invocation response")?;

        if !status.is_success() || parsed.status.to_lowercase() == "failed" {
            let msg = parsed
                .error
                .unwrap_or_else(|| format!("invoke failed with status {}", parsed.status));
            return Err(anyhow::anyhow!(msg));
        }

        parsed
            .result
            .ok_or_else(|| anyhow::anyhow!("missing result from MCP invocation"))
    }
}
