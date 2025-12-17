use std::{
    collections::{HashMap, HashSet},
    fs,
};

use once_cell::sync::OnceLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

use super::error::Result;
use super::inference::{ChatMessage, InferenceClient};
use super::mcp::McpToolDescriptor;

const PLANNER_SYSTEM_PROMPT: &str = "You are a pre-loop MCP planner. Select exactly one MCP tool from the provided list and return only compact JSON in the shape {\"server\":string|null,\"tool\":string|null,\"args\":object|null}. Prefer tools whose required params can be filled from the task text and avoid inventing fields. When forced_server is present, only choose tools from that server. Use recent_successes to bias toward servers that recently worked. When previous_error is present, avoid repeating the failing tool/arguments. Use the provided tips to set arguments; do not invent params. If no tool clearly fits, return {\"server\":null,\"tool\":null,\"args\":null}. Never include natural language, markdown, or additional keys.";
const PLANNER_STRUCTURED_PROMPT: &str = "You are a pre-loop MCP planner. Select exactly one MCP tool from the provided list and return only compact JSON in the shape {\"server\":string|null,\"tool\":string|null,\"args\":object|null,\"rationale\":string|null,\"pagination\":boolean|null,\"missing\":boolean|null}. Prefer tools whose required params can be filled from the task text; do not invent fields. When forced_server is present, only choose tools from that server. Use recent_successes to bias toward servers that recently worked. When previous_error is present, avoid repeating the failing tool/arguments. Use the provided tips to set arguments; do not invent params. If no tool clearly fits, set missing=true and leave server/tool null. Set pagination=true only when the task requires iterating pages to get the full result set. Never include natural language, markdown, or additional keys.";
const MAX_PLANNER_TOOLS: usize = 50;
const PLANNER_PAYLOAD_BUDGET_BYTES: usize = 60_000;
const PLANNER_PAYLOAD_MIN_BUDGET_BYTES: usize = 40_000;
const DEFAULT_TIPS_PATH: &str = "/sandbox/tips/planner_tips.json";
const MAX_TIPS: usize = 3;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlannerSuggestion {
    pub server: Option<String>,
    pub tool: Option<String>,
    pub args: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlannerPlan {
    pub server: Option<String>,
    pub tool: Option<String>,
    pub args: Option<Value>,
    pub rationale: Option<String>,
    pub pagination: Option<bool>,
    pub missing: Option<bool>,
}

static PLANNER_TIPS: OnceLock<std::collections::HashMap<String, String>> = OnceLock::new();

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

fn strip_schemas(tools: &[McpToolDescriptor]) -> Vec<McpToolDescriptor> {
    tools
        .iter()
        .map(|t| McpToolDescriptor {
            server: t.server.clone(),
            tool: t.tool.clone(),
            description: t.description.clone(),
            input_schema: None,
        })
        .collect()
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

    let raw = run_planner_request(
        inference_client,
        task,
        tools,
        forced_server,
        recent_successes,
        previous_error,
        PLANNER_SYSTEM_PROMPT,
    )
    .await?;

    let suggestion = parse_suggestion(&raw);
    if suggestion.is_none() {
        warn!(
            "Planner returned no usable MCP suggestion; raw={}",
            truncate_raw(&raw)
        );
    }
    Ok(suggestion.and_then(|s| enforce_forced_server(s, forced_server)))
}

pub async fn plan_tool_call_structured(
    inference_client: &InferenceClient,
    task: &str,
    tools: &[McpToolDescriptor],
    forced_server: Option<&str>,
    recent_successes: Option<&HashMap<String, String>>,
    previous_error: Option<&str>,
) -> Result<Option<PlannerPlan>> {
    if tools.is_empty() {
        return Ok(None);
    }

    let raw = run_planner_request(
        inference_client,
        task,
        tools,
        forced_server,
        recent_successes,
        previous_error,
        PLANNER_STRUCTURED_PROMPT,
    )
    .await?;

    let plan = parse_plan(&raw);
    if plan.is_none() {
        warn!(
            "Structured planner returned no usable MCP plan; raw={}",
            truncate_raw(&raw)
        );
    }
    Ok(plan.and_then(|p| enforce_forced_server_plan(p, forced_server)))
}

async fn run_planner_request(
    inference_client: &InferenceClient,
    task: &str,
    tools: &[McpToolDescriptor],
    forced_server: Option<&str>,
    recent_successes: Option<&HashMap<String, String>>,
    previous_error: Option<&str>,
    system_prompt: &str,
) -> Result<String> {
    let (mut compact, trimmed_schemas) = compact_descriptors(tools);
    info!(
        "Planner candidates: {} (schemas trimmed: {})",
        compact.len(),
        trimmed_schemas
    );
    let mut payload = json!({
        "task": task,
        "tools": compact,
    });

    // Optionally inject planner tips
    let mut injected_tips = 0usize;
    if planner_tips_enabled() {
        let tips_map = load_planner_tips();
        let selected = select_tips(task, &compact, forced_server, &tips_map);
        if !selected.is_empty() {
            injected_tips = selected.len();
            payload
                .as_object_mut()
                .unwrap()
                .insert("tips".to_string(), json!(selected));
        }
    }

    let mut payload_bytes = serde_json::to_vec(&payload)?.len();
    let mut schemas_stripped = false;

    if payload_bytes > PLANNER_PAYLOAD_BUDGET_BYTES {
        let stripped = strip_schemas(&compact);
        let stripped_payload = json!({
            "task": task,
            "tools": stripped,
        });
        let stripped_bytes = serde_json::to_vec(&stripped_payload)?.len();
        if stripped_bytes < payload_bytes {
            info!(
                "Planner payload over budget ({} bytes); dropping schemas reduces to {} bytes",
                payload_bytes, stripped_bytes
            );
            payload = stripped_payload;
            payload_bytes = stripped_bytes;
            compact = stripped;
            schemas_stripped = true;
        }
    }

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
                "Planner payload remains large ({} bytes) after truncation (schemas_stripped={})",
                payload_bytes, schemas_stripped
            );
        } else {
            info!(
                "Planner payload truncated to {} tools ({} bytes, schemas_stripped={}, tips={})",
                compact.len(),
                payload_bytes,
                schemas_stripped,
                injected_tips
            );
        }
    } else {
        info!(
            "Planner payload size {} bytes for {} tools (schemas_stripped={}, tips={})",
            payload_bytes,
            compact.len(),
            schemas_stripped,
            injected_tips
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
            Some(system_prompt.to_string()),
        )
        .await?;

    Ok(response.content.unwrap_or_default())
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

pub fn format_structured_hint(
    plan: &PlannerPlan,
    descriptor: Option<&McpToolDescriptor>,
) -> Option<String> {
    let server = plan.server.as_deref()?;
    let tool = plan.tool.as_deref()?;
    let args_value = plan.args.clone().unwrap_or_else(|| json!({}));
    let args_str = serde_json::to_string(&args_value).unwrap_or_else(|_| "{}".to_string());

    let mut parts = Vec::new();
    parts.push(format!(
        "Suggested MCP tool: server={}, tool={}, args={}. Call it as <mcp_call server=\"{}\" tool=\"{}\"><![CDATA[{}]]></mcp_call>.",
        server, tool, args_str, server, tool, args_str
    ));

    if let Some(required) = descriptor.and_then(required_params) {
        if !required.is_empty() {
            parts.push(format!(
                "Required params for {}: {}",
                tool,
                required.join(", ")
            ));
        }
    }

    if let Some(true) = plan.pagination {
        parts.push(
            "Pagination recommended: iterate pages until no results remain before finalizing."
                .to_string(),
        );
    }

    if let Some(rationale) = plan.rationale.as_ref() {
        if !rationale.trim().is_empty() {
            parts.push(format!("Rationale: {}", rationale.trim()));
        }
    }

    Some(parts.join(" "))
}

fn required_params(descriptor: &McpToolDescriptor) -> Option<Vec<String>> {
    descriptor
        .input_schema
        .as_ref()
        .and_then(|schema| schema.get("required"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty())
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

pub fn validate_plan(
    plan: PlannerPlan,
    tools: &[McpToolDescriptor],
    forced_server: Option<&str>,
) -> Option<PlannerPlan> {
    if plan.missing.unwrap_or(false) {
        return Some(plan);
    }

    let server = plan.server.as_deref()?;
    let tool = plan.tool.as_deref()?;

    if let Some(fs) = forced_server {
        if !server.eq_ignore_ascii_case(fs) {
            warn!(
                "Planner plan ignored: server {} does not match forced server {}",
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
            "Planner plan ignored: tool {} on server {} not in descriptor list",
            tool, server
        );
        return None;
    };

    if let Some(ref args) = plan.args {
        if !args_match_schema(args, descriptor.input_schema.as_ref()) {
            warn!(
                "Planner plan ignored: args failed schema validation for {}:{}",
                descriptor.server, descriptor.tool
            );
            return None;
        }
    }

    Some(plan)
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

fn parse_plan(raw: &str) -> Option<PlannerPlan> {
    let cleaned = strip_code_fences(raw).trim();
    if cleaned.is_empty() {
        return None;
    }

    if let Ok(plan) = serde_json::from_str::<PlannerPlan>(cleaned) {
        return normalize_plan(plan);
    }

    if let Ok(value) = serde_json::from_str::<Value>(cleaned) {
        if let Ok(plan) = serde_json::from_value::<PlannerPlan>(value) {
            return normalize_plan(plan);
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

fn normalize_plan(mut plan: PlannerPlan) -> Option<PlannerPlan> {
    if plan.missing.unwrap_or(false) {
        return Some(plan);
    }

    let server_empty = plan
        .server
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    let tool_empty = plan
        .tool
        .as_ref()
        .map(|t| t.trim().is_empty())
        .unwrap_or(true);

    if server_empty || tool_empty {
        return None;
    }

    plan.server = plan.server.map(|s| s.trim().to_string());
    plan.tool = plan.tool.map(|t| t.trim().to_string());
    Some(plan)
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

fn enforce_forced_server_plan(
    plan: PlannerPlan,
    forced_server: Option<&str>,
) -> Option<PlannerPlan> {
    if plan.missing.unwrap_or(false) {
        return Some(plan);
    }
    if let Some(server) = forced_server {
        if let Some(ref suggested) = plan.server {
            if !suggested.eq_ignore_ascii_case(server) {
                warn!(
                    "Planner plan rejected: expected forced server {}, got {}",
                    server, suggested
                );
                return None;
            }
        } else {
            return None;
        }
    }
    Some(plan)
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

fn planner_tips_enabled() -> bool {
    std::env::var("TSBX_PLANNER_TIPS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn load_planner_tips() -> &'static HashMap<String, String> {
    PLANNER_TIPS.get_or_init(|| {
        let path = std::env::var("TSBX_PLANNER_TIPS_PATH")
            .unwrap_or_else(|_| DEFAULT_TIPS_PATH.to_string());
        match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<HashMap<String, String>>(&contents) {
                Ok(map) => map,
                Err(err) => {
                    warn!(
                        "Failed to parse planner tips at {}: {}; falling back to embedded defaults",
                        path, err
                    );
                    load_embedded_tips()
                }
            },
            Err(err) => {
                info!(
                    "Planner tips file not found at {} ({}); using embedded defaults",
                    path, err
                );
                load_embedded_tips()
            }
        }
    })
}

fn load_embedded_tips() -> HashMap<String, String> {
    serde_json::from_str(include_str!("planner_tips.json")).unwrap_or_default()
}

fn select_tips(
    task: &str,
    tools: &[McpToolDescriptor],
    forced_server: Option<&str>,
    tips_map: &HashMap<String, String>,
) -> Vec<String> {
    if tips_map.is_empty() {
        return Vec::new();
    }

    let mut selected = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    if let Some(default_tip) = tips_map.get("default") {
        if seen.insert("default".to_string()) {
            selected.push(default_tip.clone());
        }
    }

    if let Some(server) = forced_server {
        let key = server.to_lowercase();
        if let Some(tip) = tips_map.get(&key) {
            if seen.insert(key) {
                selected.push(tip.clone());
            }
        }
    }

    for tool in tools.iter().take(5) {
        let server_key = tool.server.to_lowercase();
        let combined_key = format!("{}.{}", server_key, tool.tool.to_lowercase());

        if let Some(tip) = tips_map.get(&combined_key) {
            if seen.insert(combined_key) {
                selected.push(tip.clone());
            }
        } else if let Some(tip) = tips_map.get(&server_key) {
            if seen.insert(server_key.clone()) {
                selected.push(tip.clone());
            }
        }

        if selected.len() >= MAX_TIPS {
            break;
        }
    }

    selected.truncate(MAX_TIPS);
    selected
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
