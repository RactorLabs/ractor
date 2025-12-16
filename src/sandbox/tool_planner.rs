use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

use super::error::Result;
use super::inference::{ChatMessage, InferenceClient};
use super::mcp::McpToolDescriptor;

const PLANNER_SYSTEM_PROMPT: &str = "You are a pre-loop MCP planner. Select exactly one MCP tool from the provided list and return only compact JSON in the shape {\"server\":string|null,\"tool\":string|null,\"args\":object|null}. Prefer tools whose required params can be filled from the task text and avoid inventing fields. When forced_server is present, only choose tools from that server. Use recent_successes to bias toward servers that recently worked. When previous_error is present, avoid repeating the failing tool/arguments. If no tool clearly fits, return {\"server\":null,\"tool\":null,\"args\":null}. Never include natural language, markdown, or additional keys.";
const MAX_PLANNER_TOOLS: usize = 50;
const PLANNER_PAYLOAD_BUDGET_BYTES: usize = 60_000;
const PLANNER_PAYLOAD_MIN_BUDGET_BYTES: usize = 40_000;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlannerSuggestion {
    pub server: Option<String>,
    pub tool: Option<String>,
    pub args: Option<Value>,
}

pub fn filter_tools_for_planner(
    task: &str,
    tools: &[McpToolDescriptor],
    forced_server: Option<&str>,
    exclude: Option<(&str, &str)>,
    recent_successes: Option<&HashMap<String, String>>,
) -> Vec<McpToolDescriptor> {
    if tools.is_empty() {
        return Vec::new();
    }

    let task_lower = task.to_lowercase();
    let tokens: Vec<&str> = task_lower
        .split_whitespace()
        .filter(|t| t.len() > 2)
        .collect();

    let mut candidates: Vec<McpToolDescriptor> = tools
        .iter()
        .filter(|t| {
            if let Some(server) = forced_server {
                if !t.server.eq_ignore_ascii_case(server) {
                    return false;
                }
            }
            if let Some((ex_server, ex_tool)) = exclude {
                if t.server.eq_ignore_ascii_case(ex_server) && t.tool.eq_ignore_ascii_case(ex_tool)
                {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect();

    if candidates.is_empty() {
        return candidates;
    }

    if let Some(successes) = recent_successes {
        if !successes.is_empty() {
            info!(
                "Planner boosting tool scores using {} recent successes",
                successes.len()
            );
        }
    }

    candidates.sort_by(|a, b| {
        let score_a = tool_score(&task_lower, &tokens, a, recent_successes);
        let score_b = tool_score(&task_lower, &tokens, b, recent_successes);
        score_b
            .cmp(&score_a)
            .then_with(|| a.server.cmp(&b.server))
            .then_with(|| a.tool.cmp(&b.tool))
    });

    candidates.truncate(MAX_PLANNER_TOOLS.min(candidates.len()));
    candidates
}

fn compact_descriptors(tools: &[McpToolDescriptor]) -> (Vec<McpToolDescriptor>, usize) {
    let mut trimmed_schemas = 0;
    let compacted = tools
        .iter()
        .map(|t| {
            let compact_schema_value = t.input_schema.as_ref().map(|s| compact_schema(s, 0));
            if compact_schema_value != t.input_schema {
                trimmed_schemas += 1;
            }
            McpToolDescriptor {
                server: t.server.clone(),
                tool: t.tool.clone(),
                description: t
                    .description
                    .as_ref()
                    .map(|d| truncate_string(d, 200).to_string()),
                input_schema: compact_schema_value,
            }
        })
        .collect();
    (compacted, trimmed_schemas)
}

pub async fn plan_tool_call(
    inference_client: &InferenceClient,
    task: &str,
    tools: &[McpToolDescriptor],
    forced_server: Option<&str>,
    recent_successes: Option<&HashMap<String, String>>,
    previous_error: Option<&str>,
) -> Result<Option<PlannerSuggestion>> {
    if tools.is_empty() {
        return Ok(None);
    }

    let (mut compact, trimmed_schemas) = compact_descriptors(tools);
    let mut payload = json!({
        "task": task,
        "tools": compact,
    });

    let mut payload_bytes = serde_json::to_vec(&payload)?.len();
    if payload_bytes > PLANNER_PAYLOAD_BUDGET_BYTES {
        let mut max_tools = MAX_PLANNER_TOOLS.min(compact.len());
        while payload_bytes > PLANNER_PAYLOAD_MIN_BUDGET_BYTES && max_tools > 5 {
            max_tools = max_tools.saturating_sub(5);
            compact.truncate(max_tools);
            payload = json!({
                "task": task,
                "tools": compact,
            });
            payload_bytes = serde_json::to_vec(&payload)?.len();
        }
        if payload_bytes > PLANNER_PAYLOAD_BUDGET_BYTES {
            warn!(
                "Planner payload remains large ({} bytes) even after truncation",
                payload_bytes
            );
        } else {
            info!(
                "Planner payload truncated to {} tools ({} bytes, {} schemas trimmed)",
                max_tools, payload_bytes, trimmed_schemas
            );
        }
    } else {
        info!(
            "Planner payload size {} bytes for {} tools ({} schemas trimmed)",
            payload_bytes,
            compact.len(),
            trimmed_schemas
        );
    }

    if let Some(server) = forced_server {
        payload
            .as_object_mut()
            .unwrap()
            .insert("forced_server".to_string(), json!(server));
    }

    if let Some(successes) = recent_successes {
        if !successes.is_empty() {
            payload.as_object_mut().unwrap().insert(
                "recent_successes".to_string(),
                serde_json::to_value(successes)?,
            );
        }
    }

    if let Some(err) = previous_error {
        payload
            .as_object_mut()
            .unwrap()
            .insert("previous_error".to_string(), json!(err));
    }

    let request_body = serde_json::to_string(&payload)?;
    let response = inference_client
        .complete(
            vec![ChatMessage {
                role: "user".to_string(),
                content: request_body,
                name: None,
                tool_call_id: None,
            }],
            Some(PLANNER_SYSTEM_PROMPT.to_string()),
        )
        .await?;

    let raw = response.content.unwrap_or_default();
    let suggestion = parse_suggestion(&raw);
    if suggestion.is_none() {
        warn!(
            "Planner returned no usable MCP suggestion; raw={}",
            truncate_raw(&raw)
        );
    }
    Ok(suggestion.and_then(|s| enforce_forced_server(s, forced_server)))
}

pub fn format_planner_hint(suggestion: &PlannerSuggestion) -> Option<String> {
    let server = suggestion.server.as_deref()?;
    let tool = suggestion.tool.as_deref()?;
    let args_value = suggestion.args.clone().unwrap_or_else(|| json!({}));
    let args_str = serde_json::to_string(&args_value).unwrap_or_else(|_| "{}".to_string());

    Some(format!(
        "Suggested MCP tool: server={}, tool={}, args={}. Call it as <mcp_call server=\"{}\" tool=\"{}\"><![CDATA[{}]]></mcp_call>.",
        server, tool, args_str, server, tool, args_str
    ))
}

pub fn validate_suggestion(
    suggestion: PlannerSuggestion,
    tools: &[McpToolDescriptor],
    forced_server: Option<&str>,
) -> Option<PlannerSuggestion> {
    let server = suggestion.server.as_deref()?;
    let tool = suggestion.tool.as_deref()?;

    if let Some(fs) = forced_server {
        if !server.eq_ignore_ascii_case(fs) {
            warn!(
                "Planner suggestion ignored: server {} does not match forced server {}",
                server, fs
            );
            return None;
        }
    }

    let descriptor = tools
        .iter()
        .find(|t| t.server.eq_ignore_ascii_case(server) && t.tool.eq_ignore_ascii_case(tool));

    let Some(descriptor) = descriptor else {
        warn!(
            "Planner suggestion ignored: tool {} on server {} not in descriptor list",
            tool, server
        );
        return None;
    };

    if let Some(ref args) = suggestion.args {
        if !args_match_schema(args, descriptor.input_schema.as_ref()) {
            warn!(
                "Planner suggestion ignored: args failed schema validation for {}:{}",
                descriptor.server, descriptor.tool
            );
            return None;
        }
    }

    Some(suggestion)
}

fn parse_suggestion(raw: &str) -> Option<PlannerSuggestion> {
    let cleaned = strip_code_fences(raw).trim();
    if cleaned.is_empty() {
        return None;
    }

    if let Ok(suggestion) = serde_json::from_str::<PlannerSuggestion>(cleaned) {
        return normalize_suggestion(suggestion);
    }

    if let Ok(value) = serde_json::from_str::<Value>(cleaned) {
        if let Ok(suggestion) = serde_json::from_value::<PlannerSuggestion>(value) {
            return normalize_suggestion(suggestion);
        }
    }

    None
}

fn normalize_suggestion(mut suggestion: PlannerSuggestion) -> Option<PlannerSuggestion> {
    let server_empty = suggestion
        .server
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    let tool_empty = suggestion
        .tool
        .as_ref()
        .map(|t| t.trim().is_empty())
        .unwrap_or(true);

    if server_empty || tool_empty {
        return None;
    }

    // Trim whitespace to avoid malformed attribute values.
    suggestion.server = suggestion.server.map(|s| s.trim().to_string());
    suggestion.tool = suggestion.tool.map(|t| t.trim().to_string());
    Some(suggestion)
}

fn enforce_forced_server(
    suggestion: PlannerSuggestion,
    forced_server: Option<&str>,
) -> Option<PlannerSuggestion> {
    if let Some(server) = forced_server {
        if let Some(ref suggested) = suggestion.server {
            if !suggested.eq_ignore_ascii_case(server) {
                warn!(
                    "Planner suggestion rejected: expected forced server {}, got {}",
                    server, suggested
                );
                return None;
            }
        } else {
            return None;
        }
    }
    Some(suggestion)
}

fn args_match_schema(args: &Value, schema: Option<&Value>) -> bool {
    if schema.is_none() {
        return true;
    }
    let schema = schema.unwrap();

    if let Some(ty) = schema.get("type").and_then(|v| v.as_str()) {
        match ty {
            "object" => {
                if !args.is_object() {
                    return false;
                }
            }
            "array" => {
                if !args.is_array() {
                    return false;
                }
            }
            "string" | "number" | "integer" | "boolean" | "null" => {
                if !basic_type_matches(args, ty) {
                    return false;
                }
            }
            _ => {}
        }
    } else if schema.get("properties").is_some() && !args.is_object() {
        return false;
    }

    if let (Some(required), Some(args_obj)) = (
        schema.get("required").and_then(|v| v.as_array()),
        args.as_object(),
    ) {
        for key in required.iter().filter_map(|v| v.as_str()) {
            if !args_obj.contains_key(key) {
                return false;
            }
        }
    }

    if let (Some(properties), Some(args_obj)) = (
        schema.get("properties").and_then(|v| v.as_object()),
        args.as_object(),
    ) {
        for (key, val) in args_obj {
            if let Some(prop_schema) = properties.get(key) {
                if !args_match_schema(val, Some(prop_schema)) {
                    return false;
                }
            }
        }
    }

    true
}

fn basic_type_matches(value: &Value, ty: &str) -> bool {
    match ty {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        _ => true,
    }
}

fn compact_schema(value: &Value, depth: usize) -> Value {
    if depth > 2 {
        return Value::Null;
    }

    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            if let Some(t) = map.get("type").and_then(|v| v.as_str()) {
                out.insert("type".to_string(), Value::String(t.to_string()));
            }
            if let Some(items) = map.get("items") {
                out.insert("items".to_string(), compact_schema(items, depth + 1));
            }
            if let Some(required) = map.get("required").and_then(|v| v.as_array()) {
                let filtered: Vec<Value> = required
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| Value::String(s.to_string())))
                    .collect();
                if !filtered.is_empty() {
                    out.insert("required".to_string(), Value::Array(filtered));
                }
            }
            if let Some(props) = map.get("properties").and_then(|v| v.as_object()) {
                let mut compact_props = serde_json::Map::new();
                for (k, v) in props {
                    compact_props.insert(k.clone(), compact_schema(v, depth + 1));
                }
                if !compact_props.is_empty() {
                    out.insert("properties".to_string(), Value::Object(compact_props));
                }
            }
            Value::Object(out)
        }
        Value::Array(arr) => {
            let mut out = serde_json::Map::new();
            out.insert("type".to_string(), Value::String("array".to_string()));
            if let Some(first) = arr.first() {
                out.insert("items".to_string(), compact_schema(first, depth + 1));
            }
            Value::Object(out)
        }
        Value::String(s) => Value::String(truncate_string(s, 120).to_string()),
        _ => Value::Null,
    }
}

fn truncate_string(input: &str, max: usize) -> &str {
    if input.len() <= max {
        input
    } else {
        &input[..max]
    }
}

fn tool_score(
    task_lower: &str,
    tokens: &[&str],
    tool: &McpToolDescriptor,
    recent_successes: Option<&HashMap<String, String>>,
) -> usize {
    let name = tool.tool.to_lowercase();
    let desc = tool.description.as_deref().unwrap_or("").to_lowercase();
    let mut score = 0;

    if task_lower.contains(&name) {
        score += 5;
    }

    for token in tokens {
        if name.contains(token) {
            score += 3;
        } else if desc.contains(token) {
            score += 1;
        }
    }

    if let Some(successes) = recent_successes {
        for (tool_name, server_name) in successes {
            if tool.tool.eq_ignore_ascii_case(tool_name) {
                score += 4;
            }
            if tool.server.eq_ignore_ascii_case(server_name) {
                score += 2;
            }
        }
    }

    score
}

fn truncate_raw(raw: &str) -> String {
    const MAX_LEN: usize = 400;
    if raw.len() <= MAX_LEN {
        raw.to_string()
    } else {
        format!("{}...", &raw[..MAX_LEN])
    }
}

fn strip_code_fences(raw: &str) -> &str {
    let trimmed = raw.trim();
    if !trimmed.starts_with("```") {
        return trimmed;
    }

    let mut body = &trimmed[3..];
    if let Some(idx) = body.find('\n') {
        body = &body[idx + 1..];
    } else {
        body = "";
    }

    if let Some(end) = body.rfind("```") {
        body[..end].trim()
    } else {
        body.trim()
    }
}
