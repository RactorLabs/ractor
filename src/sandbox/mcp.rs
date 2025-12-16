use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};
use tracing::warn;

const DEFAULT_CACHE_TTL_SECS: u64 = 300;

#[derive(Debug, Clone, Deserialize, Serialize)]
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
    cache_dir: PathBuf,
    trace_dir: PathBuf,
    cache_ttl: Duration,
    trace_enabled: bool,
}

impl McpClient {
    pub fn new(base_url: String, sandbox_id: &str) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to build MCP HTTP client")?;
        let cache_dir = PathBuf::from("/sandbox/mcp_cache");
        let trace_dir = PathBuf::from("/sandbox/mcp_traces").join(sandbox_id);
        fs::create_dir_all(&cache_dir).ok();
        fs::create_dir_all(&trace_dir).ok();

        let cache_ttl = std::env::var("TSBX_MCP_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(DEFAULT_CACHE_TTL_SECS));

        let trace_enabled = std::env::var("TSBX_MCP_TRACE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(true);

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
            cache_dir,
            trace_dir,
            cache_ttl,
            trace_enabled,
        })
    }

    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        if let Some(cached) = self.read_cache("tools_all.json")? {
            return Ok(cached);
        }

        let base = self.base_url.trim_end_matches('/');
        let url = if base.ends_with("/api/v0/mcp") {
            format!("{}/tools?include_examples=true", base)
        } else {
            format!("{}/api/v0/mcp/tools?include_examples=true", base)
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
            if let Some(cached) = self.read_cache("tools_all.json")? {
                return Ok(cached);
            }
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
        let _ = self.write_cache("tools_all.json", &tools);
        let _ = self.write_cache_per_server(&tools);
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
        let started = Utc::now();
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
        payload.insert("arguments".to_string(), arguments.clone());
        payload.insert(
            "sandbox_id".to_string(),
            Value::String(sandbox_id.to_string()),
        );

        let resp = self
            .http
            .post(url)
            .header("Mcp-Session-Id", sandbox_id)
            .json(&payload)
            .send()
            .await
            .context("failed to reach MCP service for invoke")?;

        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();

        // JSON-RPC 2.0 envelope handling
        if let Ok(value) = serde_json::from_str::<Value>(&body_text) {
            if let Some(err) = value.get("error").filter(|v| !v.is_null()) {
                let msg = err
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown MCP error");
                self.write_trace(
                    sandbox_id,
                    server_id,
                    server_name,
                    tool,
                    &arguments,
                    &value.get("result").cloned(),
                    Some(msg),
                    started,
                )
                .ok();
                return Err(anyhow::anyhow!(msg.to_string()));
            }
            if let Some(result) = value.get("result") {
                self.write_trace(
                    sandbox_id,
                    server_id,
                    server_name,
                    tool,
                    &arguments,
                    &Some(result.clone()),
                    None,
                    started,
                )
                .ok();
                return Ok(result.clone());
            }

            // If status is success and we have a JSON value without result/error, treat the whole value as the result.
            if status.is_success() {
                self.write_trace(
                    sandbox_id,
                    server_id,
                    server_name,
                    tool,
                    &arguments,
                    &Some(value.clone()),
                    None,
                    started,
                )
                .ok();
                return Ok(value);
            }
        }

        // Legacy InvocationResponse fallback
        if let Ok(parsed) = serde_json::from_str::<InvocationResponse>(&body_text) {
            if !status.is_success() || parsed.status.to_lowercase() == "failed" {
                let msg = parsed
                    .error
                    .unwrap_or_else(|| format!("invoke failed with status {}", parsed.status));
                self.write_trace(
                    sandbox_id,
                    server_id,
                    server_name,
                    tool,
                    &arguments,
                    &parsed.result,
                    Some(&msg),
                    started,
                )
                .ok();
                return Err(anyhow::anyhow!(msg));
            }

            self.write_trace(
                sandbox_id,
                server_id,
                server_name,
                tool,
                &arguments,
                &parsed.result,
                None,
                started,
            )
            .ok();

            return parsed
                .result
                .ok_or_else(|| anyhow::anyhow!("missing result from MCP invocation"));
        }

        // Fallback: if status success but body is non-JSON, return raw text
        if status.is_success() {
            if !body_text.trim().is_empty() {
                warn!(
                    "MCP invoke returned non-JSON success ({}): {}",
                    status.as_u16(),
                    body_text
                );
            }
            let raw_value = Value::String(body_text.clone());
            self.write_trace(
                sandbox_id,
                server_id,
                server_name,
                tool,
                &arguments,
                &Some(raw_value.clone()),
                None,
                started,
            )
            .ok();
            return Ok(raw_value);
        }

        // If non-success and unparsable, surface error with body text
        warn!("MCP invoke failed ({}): {}", status.as_u16(), body_text);
        let msg = format!(
            "invoke failed: status {} body {}",
            status.as_u16(),
            body_text
        );
        self.write_trace(
            sandbox_id,
            server_id,
            server_name,
            tool,
            &arguments,
            &None,
            Some(&msg),
            started,
        )
        .ok();
        Err(anyhow::anyhow!(msg))
    }

    fn cache_path(&self, name: &str) -> PathBuf {
        self.cache_dir.join(name)
    }

    fn read_cache(&self, name: &str) -> Result<Option<Vec<McpTool>>> {
        let path = self.cache_path(name);
        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };

        if let Ok(modified) = meta.modified() {
            if let Ok(age) = SystemTime::now().duration_since(modified) {
                if age > self.cache_ttl {
                    return Ok(None);
                }
            }
        }

        let bytes = fs::read(&path).context("failed to read MCP cache")?;
        let tools: Vec<McpTool> =
            serde_json::from_slice(&bytes).context("failed to decode MCP cache")?;
        Ok(Some(tools))
    }

    fn write_cache(&self, name: &str, tools: &[McpTool]) -> Result<()> {
        let path = self.cache_path(name);
        let data = serde_json::to_vec_pretty(tools)?;
        fs::write(path, data)?;
        Ok(())
    }

    fn write_cache_per_server(&self, tools: &[McpTool]) -> Result<()> {
        let mut grouped: std::collections::HashMap<String, Vec<McpTool>> =
            std::collections::HashMap::new();
        for t in tools {
            let key = slugify(&t.server_name);
            grouped.entry(key).or_default().push(t.clone());
        }
        for (server, list) in grouped {
            let filename = format!("{}_tools_all.json", server);
            let _ = self.write_cache(&filename, &list);
        }
        Ok(())
    }

    fn write_trace(
        &self,
        sandbox_id: &str,
        server_id: Option<&str>,
        server_name: Option<&str>,
        tool: &str,
        arguments: &Value,
        result: &Option<Value>,
        error: Option<&str>,
        started_at: DateTime<Utc>,
    ) -> Result<()> {
        if !self.trace_enabled {
            return Ok(());
        }
        let timestamp = started_at.format("%Y%m%dT%H%M%SZ").to_string();
        let sid = server_id
            .or(server_name)
            .unwrap_or("unknown")
            .replace(['/', '\\', ':'], "_");
        let fname = format!("{}__{}__{}.json", sid, tool, timestamp);
        let path = self.trace_dir.join(fname);
        let body = serde_json::to_string_pretty(&serde_json::json!({
            "sandbox_id": sandbox_id,
            "server_id": server_id,
            "server_name": server_name,
            "tool": tool,
            "arguments": arguments,
            "result": result,
            "error": error,
            "started_at": started_at.to_rfc3339(),
            "written_at": Utc::now().to_rfc3339(),
        }))?;
        fs::write(path, body)?;
        Ok(())
    }
}

fn slugify(input: &str) -> String {
    let mut out = input
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}
