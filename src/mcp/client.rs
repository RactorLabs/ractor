use anyhow::Context;
use base64::{engine::general_purpose, Engine as _};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::Value;
use tracing::{debug, info};
use uuid::Uuid;

use crate::mcp::error::McpError;
use crate::mcp::models::{
    AuthPayload, InvocationResponse, InvokeRequest, McpToolDescriptor, McpToolSyncResponse,
};
use crate::mcp::state::McpState;

fn apply_auth(
    mut builder: reqwest::RequestBuilder,
    auth_type: Option<&str>,
    auth_payload: Option<&AuthPayload>,
) -> reqwest::RequestBuilder {
    if let Some(payload) = auth_payload {
        if let Some(token) = payload.bearer_token.as_ref() {
            builder = builder.header(AUTHORIZATION, format!("Bearer {}", token));
        }
        if let (Some(user), Some(pass)) = (
            payload.basic_username.as_ref(),
            payload.basic_password.as_ref(),
        ) {
            let encoded = general_purpose::STANDARD.encode(format!("{}:{}", user, pass));
            builder = builder.header(AUTHORIZATION, format!("Basic {}", encoded));
        }
        if let Some(headers) = payload.headers.as_ref() {
            for (k, v) in headers {
                builder = builder.header(k, v);
            }
        }
    }

    if let Some(kind) = auth_type {
        if kind.eq_ignore_ascii_case("json") {
            builder = builder.header(CONTENT_TYPE, "application/json");
        }
    }

    builder
}

pub async fn fetch_tools(
    state: &McpState,
    base_url: &str,
    auth_type: Option<&str>,
    auth_payload: Option<&AuthPayload>,
) -> Result<Vec<McpToolDescriptor>, McpError> {
    let session_id = ensure_session(state, base_url, auth_type, auth_payload).await?;
    let url = format!("{}/tools", base_url.trim_end_matches('/'));
    let mut request = state.http.post(&url);
    request = apply_auth(request, auth_type, auth_payload);
    request = request.header("Mcp-Session-Id", session_id.clone());
    request = request.header("Accept", "text/event-stream, application/json");
    request = request.header(CONTENT_TYPE, "application/json");
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });

    debug!(
        "Fetching MCP tools from {} with session {}",
        url, session_id
    );
    let resp = request
        .json(&payload)
        .send()
        .await
        .context("failed to reach MCP server for tool sync")?;
    let status = resp.status();
    if !status.is_success() {
        return Err(McpError::Upstream(format!(
            "tool sync failed: status {}",
            status
        )));
    }
    let content_type = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_lowercase();
    if content_type.contains("text/event-stream") {
        let text = resp
            .text()
            .await
            .context("failed to read MCP tool sync stream")?;
        let parsed = parse_sse_result(&text).ok_or_else(|| {
            McpError::Upstream("failed to parse SSE response for tools".to_string())
        })?;
        let envelope: Value = parsed;
        let payload = envelope.get("result").cloned().unwrap_or(envelope);
        let body: McpToolSyncResponse =
            serde_json::from_value(payload).context("failed to decode MCP tool sync payload")?;
        return Ok(body.tools);
    }
    let envelope: Value = resp
        .json()
        .await
        .context("failed to decode MCP tool sync response")?;
    let payload = envelope.get("result").cloned().unwrap_or(envelope);
    let body: McpToolSyncResponse =
        serde_json::from_value(payload).context("failed to decode MCP tool sync response")?;
    Ok(body.tools)
}

pub async fn invoke_tool(
    state: &McpState,
    base_url: &str,
    auth_type: Option<&str>,
    auth_payload: Option<&AuthPayload>,
    invocation_id: Uuid,
    request: &InvokeRequest,
) -> Result<InvocationResponse, McpError> {
    let session_id = ensure_session(state, base_url, auth_type, auth_payload).await?;
    let url = format!("{}/invoke", base_url.trim_end_matches('/'));
    let mut builder = state.http.post(&url);
    builder = apply_auth(builder, auth_type, auth_payload);
    builder = builder.header("Mcp-Session-Id", session_id.clone());
    builder = builder.header("Accept", "text/event-stream, application/json");
    builder = builder.header(CONTENT_TYPE, "application/json");

    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": request.tool,
            "arguments": request.arguments.clone().unwrap_or(serde_json::json!({}))
        }
    });

    debug!(
        "Invoking MCP tool {} on {} with session {}",
        request.tool, base_url, session_id
    );
    let resp = builder
        .json(&payload)
        .send()
        .await
        .context("failed to reach MCP server for invocation")?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(McpError::Upstream(format!(
            "invoke failed: status {}, body: {}",
            status, text
        )));
    }

    let content_type = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_lowercase();

    let value: serde_json::Value = if content_type.contains("text/event-stream") {
        let text = resp
            .text()
            .await
            .context("failed to read MCP invocation stream")?;
        parse_sse_result(&text)
            .ok_or_else(|| McpError::Upstream("failed to parse invocation SSE".to_string()))?
    } else {
        resp.json()
            .await
            .context("failed to decode MCP invocation response")?
    };

    let result_value = value
        .get("result")
        .cloned()
        .unwrap_or_else(|| value.clone());

    Ok(InvocationResponse {
        id: invocation_id,
        status: "completed".to_string(),
        result: Some(result_value),
        error: None,
    })
}

async fn ensure_session(
    state: &McpState,
    base_url: &str,
    auth_type: Option<&str>,
    auth_payload: Option<&AuthPayload>,
) -> Result<String, McpError> {
    let mut sessions = state.sessions.lock().await;
    if let Some(existing) = sessions.get(base_url) {
        return Ok(existing.clone());
    }

    let url = format!("{}/session", base_url.trim_end_matches('/'));
    let mut builder = state.http.post(&url);
    builder = apply_auth(builder, auth_type, auth_payload);
    builder = builder.header("Accept", "text/event-stream, application/json");
    builder = builder.header(CONTENT_TYPE, "application/json");

    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });

    info!(
        "Initializing MCP session at {} with payload {}",
        url, payload
    );
    let resp = builder
        .json(&payload)
        .send()
        .await
        .context("failed to reach MCP server for session init")?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(McpError::Upstream(format!(
            "session init failed: status {}, body: {}",
            status, text
        )));
    }

    let session_id = resp
        .headers()
        .get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| McpError::Upstream("missing Mcp-Session-Id header".to_string()))?;

    sessions.insert(base_url.to_string(), session_id.clone());
    Ok(session_id)
}

fn parse_sse_result(body: &str) -> Option<serde_json::Value> {
    let mut last = None;
    for line in body.lines() {
        if let Some(stripped) = line.strip_prefix("data:") {
            let json_str = stripped.trim();
            if json_str.is_empty() {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
                last = Some(value);
            }
        }
    }
    last
}
