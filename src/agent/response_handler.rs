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
        // Track whether we have already appended any segments to avoid duplicating commentary
        let mut items_sent: usize = 0;
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
                    // Mark the response as failed with the error message in output.text
                    let _ = self
                        .api_client
                        .update_response(
                            &response.id,
                            Some("failed".to_string()),
                            Some(format!("Error: {}", e)),
                            None,
                        )
                        .await?;
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
                    // Send only the new segments, server appends
                    let _ = self.api_client.update_response(&response.id, Some("processing".to_string()), None, Some(segs.clone())).await;
                    items_sent += segs.len();

                    // Execute tool
                    let (tool_result, tool_error): (String, Option<String>) = match self
                        .tool_registry
                        .execute_tool(tool_name, args)
                        .await
                    {
                        Ok(r) => (r, None),
                        Err(e) => (format!("[error] {}", e), Some(e.to_string())),
                    };
                    // Append only the tool_result (avoid duplicating prior items)
                    let seg_tool_result = serde_json::json!({"type":"tool_result","tool":tool_name,"output":tool_result});
                    let _ = self.api_client.update_response(&response.id, Some("processing".to_string()), None, Some(vec![seg_tool_result])).await;
                    items_sent += 1;
                    // If the tool reported an error (commonly from API calls), mark the response as failed with message
                    if let Some(err_msg) = tool_error {
                        let _ = self
                            .api_client
                            .update_response(
                                &response.id,
                                Some("failed".to_string()),
                                Some(format!("Error: {}", err_msg)),
                                None,
                            )
                            .await;
                        return Ok(());
                    }
                    // Add tool result to conversation
                    conversation.push(ChatMessage { role:"tool".to_string(), content: tool_result, name: Some(tool_name.clone()), tool_call_id: None });
                    continue;
                }
            }

            // Final answer
            let sanitized = self.guardrails.validate_output(&model_resp.content)?;
            let mut segs = Vec::new();
            // Only include commentary here if we haven't already sent commentary in this turn
            if items_sent == 0 {
                if let Some(thinking) = &model_resp.thinking { if !thinking.trim().is_empty() { segs.push(serde_json::json!({"type":"commentary","channel":"analysis","text":thinking})); } }
            }
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
        // Read hosting context from environment (provided by start script)
        let host_name = std::env::var("RAWORC_HOST_NAME").unwrap_or_else(|_| "Raworc".to_string());
        let base_url_env = std::env::var("RAWORC_HOST_URL").expect("RAWORC_HOST_URL must be set by the start script");
        let base_url = base_url_env.trim_end_matches('/').to_string();

        // Fetch agent info from API/DB (name, publish state)
        let (agent_name_ctx, is_published_ctx, published_at_ctx) = match self.api_client.get_agent().await {
            Ok(agent) => {
                let nm = agent.name.clone();
                let ip = agent.is_published;
                let pa = agent.published_at.clone().unwrap_or_else(|| "".to_string());
                (nm, ip, pa)
            }
            Err(_) => (self.api_client.agent_name().to_string(), false, String::new()),
        };

        // Current timestamp in UTC for context
        let current_time_utc = chrono::Utc::now().to_rfc3339();

        let operator_url = format!("{}", base_url);
        let api_url = format!("{}/api", base_url);
        let published_url = format!("{}/content/{}", base_url, agent_name_ctx);

        // Start with System Context specific to Raworc runtime
        let mut prompt = String::from(format!(
            r#"SYSTEM CONTEXT

You are running as an Agent in the {host_name} system.

- System Name: {host_name}
- Base URL: {base_url}
- Current Time (UTC): {current_time_utc}
- Operator URL: {operator_url}
- API URL: {api_url}
- Your Agent Name: {agent_name}
- Published Content URL: {published_url}
- Published: {published_flag}
 - Published At: {published_at}

Platform endpoints:
- Content Server: {base_url}/content — public gateway that serves published agent content at a stable URL (path prefix /content).
- API Server: {base_url}/api — JSON API used by the Operator and runtimes for management, not for end users.

About content and publishing:
- Your working content lives under /agent/content/.
- There is no live preview server. When the user wants to view content, publish it.
- Publishing creates a public, stable snapshot of /agent/content/ and makes it available at the Published Content URL: {published_url}.
- Published content is meant to be safe for public access (HTML/JS/CSS and assets). Do not include secrets or sensitive data in /agent/content/.
- The public gateway serves the last published snapshot. It does not auto-update until you explicitly publish again.

Important behavior:
- Do NOT ask the user to start an HTTP server for /agent/content.
- Do NOT share any local or preview URLs. Only share the published URL(s) after publishing.
- When you create or modify files under /agent/content/ and the user asks to view them, perform a publish action and include the full, absolute Published URL(s).
  - Example: {published_url}/index.html or {published_url}/dashboard/report.html
- Use absolute URLs that include protocol and host. Do NOT use relative URLs.
- Outside of an explicit publish action, include Published URLs only if the user asks for them or asks about publish status.
- Publishing is an explicit action (via the Operator UI, API, or the publish tool). When asked to publish, proceed without extra confirmation.
- IMPORTANT: Always output URLs as plain text without any code formatting. Never wrap URLs in backticks or code blocks.

"#,
            host_name = host_name,
            base_url = base_url,
            operator_url = operator_url,
            api_url = api_url,
            agent_name = agent_name_ctx,
            published_url = published_url,
            published_flag = if is_published_ctx { "true" } else { "false" },
            published_at = if is_published_ctx && !published_at_ctx.is_empty() { published_at_ctx.as_str() } else { "(not published)" },
            current_time_utc = current_time_utc,
        ));

        // Continue with the general capabilities prompt
        prompt.push_str(
            r#"You are a highly capable AI agent with full access to bash commands and text editing capabilities. You operate in an isolated container environment where you can execute commands, install packages, and create any type of content to help users accomplish their goals.

## Core Capabilities

You are an AI agent with unrestricted access to:
- **Bash shell**: Execute any command using semicolons to chain operations efficiently
- **Text editor**: Create, modify, and view files with precision
- **Package management**: Install pip, npm, apt packages, or any other tools needed
- **Internet access**: Use curl to fetch websites, APIs, and download files
- **Development**: Code in any language, run scripts, build applications
- **System administration**: Full control within your container environment

## Directory Structure (/agent/)

```
├── code/        - All development files, scripts, source code, data
├── content/     - HTML files and web assets for user display
├── logs/        - Automatic command logs (read-only)
└── secrets/     - Environment variables (auto-managed)
```

**Working files**: Use `/agent/code/` for everything - scripts, data files, projects, executables
**User displays**: Use `/agent/content/` for HTML, visualizations, reports, dashboards
**Special files**:
- `/agent/code/instructions.md` - Persistent instructions (auto-loaded)
- `/agent/code/setup.sh` - Initialization script (auto-executed)

## Tools Available

- **bash**: Execute any shell command - no restrictions within the container
- **text_editor**: Create, view, edit files with actions: view, create, str_replace, insert
- **Full package ecosystem**: pip, npm, apt, cargo, composer, etc.
- **Development tools**: git, curl, wget, grep, find, jq, and more
- **Programming languages**: Python, Node.js, Rust (pre-installed)

## Tool Resolution Order (Prefer Local Code)

When a user asks you to use a tool by name (e.g., "run foo" or "use tool bar"), prefer locally provided tools in the code workspace before system-wide tools:
- First, check for an executable or script in `/agent/code/` with the requested name.
- Consider common forms: `/agent/code/<name>`, `/agent/code/<name>.sh`, `/agent/code/<name>.py`, `/agent/code/<name>.js`, or `/agent/code/bin/<name>`.
- If a matching local tool exists, use it. Only fall back to system-installed tools if no local tool is found.
- If multiple candidates exist, prefer the one in `/agent/code/bin/`, then the exact name in `/agent/code/`.

## CRITICAL: Use Tools Intentionally (no repeats)

**When you need to run commands**: Use the bash tool to take action.
**When you need to edit files**: Use the text_editor tool to make changes.  
**When you see errors or need to fix something**: Use tools to take corrective action.
**When you want to check something**: Use the bash tool to verify — don't assume.

Avoid redundant work:
- Do NOT repeat the same tool call or command again and again if the previous step completed successfully.
- Before re-running a command, confirm what has changed (inputs, parameters, environment) and explain the reason to re-run.
- If a prior step produced the needed result, move on to the next step in the plan.

Decision policy:
- If the next step is UNCLEAR, ask the user a concise clarifying question instead of guessing or looping.
- If you have a CLEAR plan, proceed and execute it step-by-step without unnecessary repetition.

## Command Execution Philosophy

**Chain commands efficiently**: Use semicolons (;) and logical operators (&&, ||) to execute multiple operations in one shot:
- `cd project && npm install && npm start`
- `python3 -m venv venv; source venv/bin/activate; pip install requests pandas; python script.py`
- `curl -o data.json https://api.example.com/data && python process.py`

- `wget https://example.com/dataset.csv`

## Command Efficiency Guidelines

**NEVER use `ls -R`** - Always run smaller, targeted ls commands and expand from there:
- ❌ WRONG: `ls -R /agent/code` (can produce overwhelming output)
- ✅ CORRECT: `ls /agent/code` then explore specific subdirectories
- ✅ CORRECT: `ls /agent/code/project1` to examine a specific directory
- ✅ CORRECT: `find /agent/code -name "*.py" -maxdepth 2` for targeted file discovery

**Use specific commands** instead of broad ones to avoid large outputs:
- `ls /agent/code/*.py` to see only Python files
- `head -20 file.log` instead of `cat file.log` for large files
- `du -sh /agent/code/*` instead of recursive listings for directory sizes
- `grep -l "pattern" /agent/code/*` to find files containing patterns

**When exploring directory structures**: Build understanding incrementally:
- Start with `ls /agent/code/` to see top-level structure
- Then drill down: `ls /agent/code/project1/` for specific areas of interest
- Use `tree -L 2 /agent/code` if you need a recursive view (limit depth)
- Use `find` with specific patterns: `find /agent/code -type f -name "*.py"`

**For debugging**: Use targeted commands that give useful info without overwhelming output.

## Best Practices

**Be proactive**: Don't ask for permission to install tools or packages - just do what's needed
**Chain operations**: Combine multiple commands with `;` or `&&` for efficiency
**Use virtual environments for Python**: `python3 -m venv venv; source venv/bin/activate; pip install packages`
**Create visual outputs**: Build HTML dashboards, charts, and interactive content in `/agent/content/`
**Save your work**: Store all code and data in `/agent/code/` for persistence
**Document as you go**: Create clear file structures and comments

## Examples

Install and analyze stock data:
```bash
python3 -m venv venv; source venv/bin/activate; pip install yfinance pandas matplotlib; python -c "import yfinance as yf; data = yf.download('AAPL', period='1y'); print(data.head())"
```

Fetch API data and process:
```bash
curl -s https://jsonplaceholder.typicode.com/posts | jq '.[0:5]' > sample_data.json && python process_data.py
```

Build a web dashboard:
```bash
mkdir -p content/dashboard; echo '<html>...' > content/dashboard/index.html
```

You have complete freedom to execute commands, install packages, and create solutions. Focus on being efficient and getting things done quickly.

"#,
        );

        // Read instructions from /agent/code/instructions.md if it exists
        let instructions_path = std::path::Path::new("/agent/code/instructions.md");
        info!(
            "Checking for instructions file at: {}",
            instructions_path.display()
        );
        if instructions_path.exists() {
            info!("Instructions file exists, reading contents...");
            match tokio::fs::read_to_string(instructions_path).await {
                Ok(instructions) => {
                    info!("Read instructions content: '{}'", instructions.trim());
                    prompt.push_str("\n\nSPECIAL INSTRUCTIONS FROM USER:\n");
                    prompt.push_str(&instructions);
                    info!("Loaded instructions from /agent/code/instructions.md");
                }
                Err(e) => {
                    warn!("Failed to read instructions file: {}", e);
                }
            }
        } else {
            info!("No instructions file found at {}", instructions_path.display());
        }

        prompt
    }
}

// Removed legacy user tool-call parser; non-standard formats are not parsed.
