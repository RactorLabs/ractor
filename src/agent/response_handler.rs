use super::api::{RaworcClient, ResponseView};
use super::builtin_tools::{BashTool, TextEditorTool};
use super::error::Result;
use super::guardrails::Guardrails;
use super::ollama::{ChatMessage, ModelResponse, OllamaClient};
use super::tool_registry::{ContainerExecMapper, ToolRegistry};
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub struct ResponseHandler {
    api_client: Arc<RaworcClient>,
    ollama_client: Arc<OllamaClient>,
    guardrails: Arc<Guardrails>,
    processed_response_ids: Arc<Mutex<HashSet<String>>>,
    task_created_at: DateTime<Utc>,
    tool_registry: Arc<ToolRegistry>,
}

impl ResponseHandler {
    pub fn new(
        api_client: Arc<RaworcClient>,
        ollama_client: Arc<OllamaClient>,
        guardrails: Arc<Guardrails>,
    ) -> Self {
        Self::new_with_registry(api_client, ollama_client, guardrails, None)
    }

    pub fn new_with_registry(
        api_client: Arc<RaworcClient>,
        ollama_client: Arc<OllamaClient>,
        guardrails: Arc<Guardrails>,
        tool_registry: Option<Arc<ToolRegistry>>,
    ) -> Self {
        let task_created_at = std::env::var("RAWORC_TASK_CREATED_AT")
            .ok()
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|| {
                warn!("RAWORC_TASK_CREATED_AT not found, using current time");
                Utc::now()
            });

        // Initialize tool registry
        let tool_registry = if let Some(registry) = tool_registry {
            registry
        } else {
            let registry = Arc::new(ToolRegistry::new());
            tokio::spawn({
                let registry = registry.clone();
                let api_client_clone = api_client.clone();
                async move {
                    registry.register_tool(Box::new(BashTool)).await;
                    registry.register_tool(Box::new(TextEditorTool)).await;
                    let publish_tool = Box::new(super::builtin_tools::PublishTool::new(api_client_clone.clone()));
                    let sleep_tool = Box::new(super::builtin_tools::SleepTool::new(api_client_clone.clone()));
                    registry.register_tool(publish_tool).await;
                    registry.register_tool(sleep_tool).await;
                    registry
                        .register_alias("container.exec", "bash", Some(Box::new(ContainerExecMapper)))
                        .await;
                    info!("Registered built-in tools and aliases");
                }
            });
            registry
        };

        Self {
            api_client,
            ollama_client,
            guardrails,
            processed_response_ids: Arc::new(Mutex::new(HashSet::new())),
            task_created_at,
            tool_registry,
        }
    }

    pub async fn initialize_processed_tracking(&self) -> Result<()> {
        info!("Initializing response tracking; task created at {}", self.task_created_at);
        let total = self.api_client.get_response_count().await.unwrap_or(0);
        let limit: u32 = 500;
        let offset = if total > limit as u64 { (total - limit as u64) as u32 } else { 0 };
        let all = self.api_client.get_responses(Some(limit), Some(offset)).await?;
        let mut pre = HashSet::new();
        for r in &all {
            if let Ok(t) = DateTime::parse_from_rfc3339(&r.created_at) {
                if t.with_timezone(&Utc) < self.task_created_at {
                    pre.insert(r.id.clone());
                }
            }
        }
        let mut processed = self.processed_response_ids.lock().await;
        *processed = pre;
        Ok(())
    }

    pub async fn poll_and_process(&self) -> Result<usize> {
        let total = self.api_client.get_response_count().await.unwrap_or(0);
        let window: u32 = 50;
        let offset = if total > window as u64 { (total - window as u64) as u32 } else { 0 };
        let recent = self.api_client.get_responses(Some(window), Some(offset)).await?;
        if recent.is_empty() { return Ok(0); }

        let mut pending: Vec<ResponseView> = Vec::new();
        for r in &recent {
            if let Ok(t) = DateTime::parse_from_rfc3339(&r.created_at) {
                if t.with_timezone(&Utc) >= self.task_created_at && r.status.to_lowercase() == "pending" {
                    let processed = self.processed_response_ids.lock().await;
                    if !processed.contains(&r.id) { pending.push(r.clone()); }
                }
            }
        }
        if pending.is_empty() { return Ok(0); }
        pending.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        if let Err(e) = self.api_client.update_agent_to_busy().await { warn!("Failed to set busy: {}", e); }
        for r in &pending {
            if let Err(e) = self.process_response(r).await {
                error!("Failed to process response {}: {}", r.id, e);
                let _ = self.api_client.update_response(&r.id, Some("failed".to_string()), Some(format!("Error: {}", e)), None).await;
            }
            let mut processed = self.processed_response_ids.lock().await;
            processed.insert(r.id.clone());
        }
        if let Err(e) = self.api_client.update_agent_to_idle().await { warn!("Failed to set idle: {}", e); }
        Ok(pending.len())
    }

    async fn process_response(&self, response: &ResponseView) -> Result<()> {
        let input_text = response.input.get("text").and_then(|v| v.as_str()).unwrap_or("");
        self.guardrails.validate_input(input_text)?;

        // Build conversation from prior responses
        let all = self.api_client.get_responses(None, None).await?;
        let convo = self.prepare_conversation_from_responses(&all, response);

        // Build system prompt
        let system_prompt = self.build_system_prompt().await;

        let mut conversation = convo;
        loop {
            // Call model (with simple retry/backoff inside ollama client)
            let model_resp: ModelResponse = match self
                .ollama_client
                .complete_with_registry(conversation.clone(), Some(system_prompt.clone()), Some(&*self.tool_registry))
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    warn!("Ollama API failed: {}", e);
                    let _ = self.api_client.update_response(&response.id, Some("failed".to_string()), Some("I'm experiencing technical difficulties.".to_string()), None).await?;
                    return Ok(());
                }
            };

            if let Some(tool_calls) = &model_resp.tool_calls {
                if let Some(tc) = tool_calls.first() {
                    let tool_name = &tc.function.name;
                    let args = &tc.function.arguments;
                    // Append thinking + tool_call
                    let mut segs = Vec::new();
                    if let Some(thinking) = &model_resp.thinking { if !thinking.trim().is_empty() { segs.push(serde_json::json!({"type":"commentary","channel":"analysis","text":thinking})); } }
                    segs.push(serde_json::json!({"type":"tool_call","tool":tool_name,"args":args}));
                    let _ = self.api_client.update_response(&response.id, Some("processing".to_string()), None, Some(segs.clone())).await;

                    // Execute tool
                    let tool_result = match self.tool_registry.execute_tool(tool_name, args).await { Ok(r)=>r, Err(e)=>format!("[error] {}", e)};
                    // Append tool_result
                    let mut segs2 = segs;
                    segs2.push(serde_json::json!({"type":"tool_result","tool":tool_name,"output":tool_result}));
                    let _ = self.api_client.update_response(&response.id, Some("processing".to_string()), None, Some(segs2.clone())).await;
                    // Add tool result to conversation
                    conversation.push(ChatMessage { role:"tool".to_string(), content: tool_result, name: Some(tool_name.clone()), tool_call_id: None });
                    continue;
                }
            }

            // Final answer
            let sanitized = self.guardrails.validate_output(&model_resp.content)?;
            let mut segs = Vec::new();
            if let Some(thinking) = &model_resp.thinking { if !thinking.trim().is_empty() { segs.push(serde_json::json!({"type":"commentary","channel":"analysis","text":thinking})); } }
            segs.push(serde_json::json!({"type":"final","channel":"final","text":sanitized}));
            let _ = self.api_client.update_response(&response.id, Some("completed".to_string()), Some(sanitized), Some(segs)).await?;
            return Ok(());
        }
    }

    fn prepare_conversation_from_responses(&self, responses: &[ResponseView], current: &ResponseView) -> Vec<ChatMessage> {
        let mut convo = Vec::new();
        for r in responses.iter() {
            if r.id == current.id { continue; }
            if let Some(text) = r.input.get("text").and_then(|v| v.as_str()) { if !text.is_empty() { convo.push(ChatMessage { role:"user".to_string(), content:text.to_string(), name:None, tool_call_id:None }); } }
            if let Some(items) = r.output.get("items").and_then(|v| v.as_array()) {
                for it in items {
                    if it.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                        let name = it.get("tool").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let content = it.get("output").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if !content.is_empty() { convo.push(ChatMessage { role:"tool".to_string(), content, name, tool_call_id: None }); }
                    }
                }
            }
            if let Some(out) = r.output.get("text").and_then(|v| v.as_str()) { if !out.trim().is_empty() { convo.push(ChatMessage { role:"assistant".to_string(), content:out.to_string(), name:None, tool_call_id:None }); } }
        }
        if let Some(text) = current.input.get("text").and_then(|v| v.as_str()) { convo.push(ChatMessage { role:"user".to_string(), content:text.to_string(), name:None, tool_call_id:None }); }
        convo
    }

    async fn build_system_prompt(&self) -> String {
        // Minimal prompt; keep prior branding bits
        let base_url_env = std::env::var("RAWORC_HOST_URL").unwrap_or_default();
        let base_url = base_url_env.trim_end_matches('/').to_string();
        let operator_url = format!("{}", base_url);
        let api_url = format!("{}/api", base_url);
        let current_time_utc = chrono::Utc::now().to_rfc3339();
        let mut prompt = String::new();
        prompt.push_str("You are a coding agent. Follow user instructions.\n");
        prompt.push_str(&format!("Operator: {}\nAPI: {}\nTime(UTC): {}\n", operator_url, api_url, current_time_utc));
        prompt
    }
}

fn parse_user_tool_call(s: &str) -> Option<(String, serde_json::Value)> {
    let trimmed = s.trim();
    if !(trimmed.starts_with('{') && trimmed.ends_with('}')) { return None; }
    let v: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let tool = v.get("tool")?.as_str()?.to_string();
    if tool != "bash" && tool != "text_editor" { return None; }
    let input = v.get("input")?.clone();
    Some((tool, input))
}

