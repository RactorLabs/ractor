use anyhow::{anyhow, Context, Error as AnyError, Result};
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::builtin_tools;
use super::command::{CommandChild, CommandInvocation};
use super::mcp::{McpClient, McpTool};

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, args: &Value) -> Result<Value>;
}

pub struct ExecutionResult {
    pub args: Value,
    pub output: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpRouting {
    pub server_name: Option<String>,
    pub tool_name: String,
}

pub struct ExecutionError {
    pub args: Value,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntentRouterHint {
    Direct {
        alias: String,
        server_name: String,
        tool_name: String,
    },
    Ambiguous {
        tool_name: String,
        servers: Vec<String>,
    },
}

impl IntentRouterHint {
    pub fn to_prompt(&self) -> String {
        match self {
            IntentRouterHint::Direct {
                alias,
                server_name,
                tool_name,
            } => format!("Router hint: This request matches MCP tool `{alias}` (server `{server_name}`, tool `{tool_name}`). Call that MCP alias directly, or use <mcp_call server=\"{server_name}\" tool=\"{tool_name}\"> with the registry tool name (not the alias)."),
            IntentRouterHint::Ambiguous {
                tool_name,
                servers,
            } => {
                let options = servers.join(", ");
                format!("Router hint: This request maps to MCP tool `{tool_name}` exposed by multiple servers ({options}). Ask which server to use, then call the matching MCP alias.")
            }
        }
    }
}

impl ExecutionError {
    fn from_error(args: Value, err: AnyError) -> Self {
        Self {
            args,
            message: err.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct McpToolAlias {
    pub alias: String,
    pub server_id: String,
    pub server_name: String,
    pub tool_name: String,
    pub description: Option<String>,
}

pub struct ToolCatalog {
    mcp_client: Option<Arc<McpClient>>,
    mcp_tools: HashMap<String, McpToolAlias>,
    server_tool_index: HashMap<String, HashSet<String>>,
    sandbox_id: String,
}

impl ToolCatalog {
    pub fn new(
        sandbox_id: String,
        mcp_client: Option<Arc<McpClient>>,
        mcp_tool_inventory: Vec<McpTool>,
    ) -> Self {
        let mut mcp_tools = HashMap::new();
        let mut server_tool_index: HashMap<String, HashSet<String>> = HashMap::new();
        for tool in mcp_tool_inventory {
            let alias = format!("mcp_{}_{}", slugify(&tool.server_name), slugify(&tool.name));
            mcp_tools.insert(
                alias.clone(),
                McpToolAlias {
                    alias,
                    server_id: tool.server_id.clone(),
                    server_name: tool.server_name.clone(),
                    tool_name: tool.name.clone(),
                    description: tool.description.clone(),
                },
            );
            server_tool_index
                .entry(tool.server_name.clone())
                .or_default()
                .insert(tool.name.clone());
        }
        Self {
            mcp_client,
            mcp_tools,
            server_tool_index,
            sandbox_id,
        }
    }

    pub fn mcp_client(&self) -> Option<Arc<McpClient>> {
        self.mcp_client.clone()
    }

    fn base_tools() -> Vec<&'static str> {
        vec![
            "run_bash",
            "open_file",
            "create_file",
            "str_replace",
            "insert",
            "remove_str",
            "find_filecontent",
            "find_filename",
            "web_fetch",
            "output",
        ]
    }

    pub fn known_tools(&self) -> Vec<String> {
        let mut list: Vec<String> = Self::base_tools()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        if !self.mcp_tools.is_empty() {
            list.push("mcp_call".to_string());
            list.extend(self.mcp_tools.keys().cloned());
        }
        list
    }

    pub fn intent_router_hint(
        &self,
        user_text: &str,
        preferred_servers: Option<&HashMap<String, String>>,
    ) -> Option<IntentRouterHint> {
        if self.mcp_tools.is_empty() {
            return None;
        }

        let trimmed = user_text.trim();
        if trimmed.is_empty() {
            return None;
        }

        let user_tokens = tokenize(trimmed);
        if user_tokens.is_empty() {
            return None;
        }

        if let Some(target_tool) = self.canonical_tool_from_text(trimmed) {
            let mut matches: Vec<&McpToolAlias> = self
                .mcp_tools
                .values()
                .filter(|alias| alias.tool_name.eq_ignore_ascii_case(&target_tool))
                .collect();

            if !matches.is_empty() {
                matches.sort_by(|a, b| a.server_name.cmp(&b.server_name));

                if let Some(pref) = preferred_servers {
                    if let Some(server) = pref.get(&target_tool) {
                        if let Some(alias) = matches
                            .iter()
                            .find(|a| a.server_name.eq_ignore_ascii_case(server))
                        {
                            return Some(IntentRouterHint::Direct {
                                alias: alias.alias.clone(),
                                server_name: alias.server_name.clone(),
                                tool_name: alias.tool_name.clone(),
                            });
                        }
                    }
                }

                if matches.len() == 1 {
                    let alias = matches[0];
                    return Some(IntentRouterHint::Direct {
                        alias: alias.alias.clone(),
                        server_name: alias.server_name.clone(),
                        tool_name: alias.tool_name.clone(),
                    });
                }

                let servers: Vec<String> = matches.iter().map(|a| a.server_name.clone()).collect();
                return Some(IntentRouterHint::Ambiguous {
                    tool_name: target_tool,
                    servers,
                });
            }
        }

        let mut scored: Vec<(usize, &McpToolAlias)> = Vec::new();
        for alias in self.mcp_tools.values() {
            if let Some(score) = intent_match_score(trimmed, &user_tokens, alias) {
                scored.push((score, alias));
            }
        }
        if scored.is_empty() {
            return None;
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        let top_score = scored[0].0;
        if top_score < 2 {
            return None;
        }

        let top: Vec<_> = scored
            .into_iter()
            .filter(|(score, _)| *score == top_score)
            .collect();

        if top.len() == 1 {
            let alias = top[0].1;
            return Some(IntentRouterHint::Direct {
                alias: alias.alias.clone(),
                server_name: alias.server_name.clone(),
                tool_name: alias.tool_name.clone(),
            });
        }

        let first_tool = top[0].1.tool_name.clone();
        if top.iter().all(|(_, alias)| alias.tool_name == first_tool) {
            let mut servers: Vec<String> = top
                .iter()
                .map(|(_, alias)| alias.server_name.clone())
                .collect();
            servers.sort();
            servers.dedup();

            if let Some(pref) = preferred_servers {
                if let Some(server) = pref.get(&first_tool) {
                    if let Some(alias) = self.mcp_tools.values().find(|a| {
                        a.tool_name == first_tool && a.server_name.eq_ignore_ascii_case(server)
                    }) {
                        return Some(IntentRouterHint::Direct {
                            alias: alias.alias.clone(),
                            server_name: alias.server_name.clone(),
                            tool_name: alias.tool_name.clone(),
                        });
                    }
                }
            }

            if servers.len() == 1 {
                if let Some(alias) = self
                    .mcp_tools
                    .values()
                    .find(|a| a.tool_name == first_tool && a.server_name == servers[0])
                {
                    return Some(IntentRouterHint::Direct {
                        alias: alias.alias.clone(),
                        server_name: alias.server_name.clone(),
                        tool_name: alias.tool_name.clone(),
                    });
                }
            }

            return Some(IntentRouterHint::Ambiguous {
                tool_name: first_tool,
                servers,
            });
        }

        None
    }

    pub fn has(&self, name: &str) -> bool {
        if Self::base_tools().iter().any(|n| *n == name) {
            return true;
        }
        if name == "mcp_call" && self.mcp_client.is_some() {
            return true;
        }
        self.mcp_tools.contains_key(name)
    }

    pub fn command_catalog_prompt(&self) -> String {
        let mut guide = String::from("Tool Reference\n\n");
        guide.push_str("Use exactly one tool call per response until the task is complete. Prefer the purpose-built tools below over improvising with shell commands. Key details:\n");
        guide.push_str(
            "- Paths must stay under `/sandbox` unless the user explicitly directs otherwise.\n",
        );
        guide.push_str("- Always validate file or directory existence before operating on them.\n");
        guide.push_str(
            "- Surface tool failures back to the user so the orchestrating model can react.\n",
        );
        guide.push_str(
            "- Only call tools that directly satisfy the current step; once the user’s request is complete, stop and respond with `<output>`.\n\n",
        );
        guide.push_str("Every XML snippet below is a TEMPLATE. Replace every placeholder (e.g. `<COMMENTARY_GOES_HERE>`, `<PATH_UNDER_/sandbox>`, `<REPLACE_WITH_CONTENT_OR_LEAVE_EMPTY>`) with task-specific values and never send the template verbatim.\n\n");

        guide.push_str("### Tool: web_fetch\n");
        guide.push_str(
            "Example template (only when you must read public web content; never copy verbatim):\n",
        );
        guide.push_str(
            r#"  <web_fetch commentary="<COMMENTARY_GOES_HERE>" url="https://example.com/resource" timeout_seconds="15" max_bytes="200000" segment="head" offset_bytes="0"/>"#,
        );
        guide.push_str("\n- Fetch HTTP/HTTPS content without shell access. Use this instead of `run_bash`+`curl` for reads.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str(
            "  - `commentary` (required): Brief reason for the fetch (include the domain or doc you need).\n",
        );
        guide.push_str("  - `url` (required): HTTP or HTTPS URL. Redirects are limited.\n");
        guide.push_str("  - `timeout_seconds` (optional): Defaults to 20s, capped at 60s.\n");
        guide.push_str("  - `max_bytes` (optional): Defaults to 200kB, hard-capped at 1MB; longer responses are truncated.\n");
        guide.push_str(
            "  - `segment` (optional): `head` (default) returns the beginning; `tail` returns the last `max_bytes`.\n",
        );
        guide.push_str(
            "  - `offset_bytes` (optional with `segment=head`): Skip this many bytes before capturing the next `max_bytes`.\n",
        );
        guide.push_str(
            "- The tool returns the response body (UTF-8 when possible, otherwise base64) plus metadata including truncation flags.\n\n",
        );

        guide.push_str("### Tool: run_bash\n");
        guide.push_str("Example template (adapt placeholders; never copy verbatim):\n");
        guide.push_str(
            r#"  <run_bash commentary="<COMMENTARY_GOES_HERE>" exec_dir="/sandbox/<PATH_UNDER_/sandbox>" commands="echo '<COMMENTARY_GOES_HERE>'; <COMMAND_TO_RUN>"/>"#,
        );
        guide.push_str("\n- Execute a single shell command sequence. Echo the planned action first so the log is self-describing.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str(
            "  - `commentary` (required): Short description of the action (no ellipses).\n",
        );
        guide.push_str("  - `exec_dir` (required): Directory to run the command in; must be `/sandbox` or a child directory.\n");
        guide.push_str("  - `commands` (required): The shell command(s). Chain with `&&` only when each step depends on the previous result.\n");
        guide.push_str("- Only call when the user asks for shell work or you must run a command to complete their request.\n");
        guide.push_str("- On failure, capture the exit code and last 20 stderr lines, then suggest a revised plan or retry with corrected parameters.\n\n");

        guide.push_str("### Tool: open_file\n");
        guide.push_str("Example template (adapt placeholders; never copy verbatim):\n");
        guide.push_str(
            r#"  <open_file commentary="<COMMENTARY_GOES_HERE>" path="/sandbox/<PATH_UNDER_/sandbox>" start_line="optional" end_line="optional"/>"#,
        );
        guide.push_str("\n- Read file contents for inspection. Use before editing so you never assume state.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Why you’re reading the file.\n");
        guide.push_str(
            "  - `path` (required): Absolute path to the file (must be under `/sandbox`).\n",
        );
        guide.push_str("  - `start_line` / `end_line` (optional): Limit output to a specific range (1-based, inclusive). These must be integers.\n");
        guide.push_str("- Call this to inspect files when needed for the user’s request; avoid exploratory reads the user did not ask for, and skip it after a successful create/edit unless the user wants proof.\n\n");

        guide.push_str("### Tool: create_file\n");
        guide.push_str("Example template (only when the user explicitly requests a new file; never copy verbatim):\n");
        guide.push_str(
            r#"  <create_file commentary="<COMMENTARY_GOES_HERE>" path="/sandbox/<PATH_UNDER_/sandbox>"><![CDATA[<REPLACE_WITH_CONTENT_OR_LEAVE_EMPTY>]]></create_file>"#,
        );
        guide.push_str("\n- Create a brand-new file with the supplied body. Only use when the user explicitly requests a new file.\n");
        guide.push_str("- Before calling, confirm the parent directory exists; create directories only if the user asked you to.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Reason for creating the file.\n");
        guide.push_str("  - `path` (required): Absolute file path (under `/sandbox`).\n");
        guide.push_str("  - Body (CDATA): The exact file contents.\n\n");

        guide.push_str("### Tool: str_replace\n");
        guide.push_str("Example template (adapt placeholders; never copy verbatim):\n");
        guide.push_str(
            r#"  <str_replace commentary="<COMMENTARY_GOES_HERE>" path="/sandbox/<PATH_UNDER_/sandbox>" many="false">
  <old_str><![CDATA[<TEXT_TO_REPLACE>]]></old_str>
  <new_str><![CDATA[<REPLACEMENT_TEXT>]]></new_str>
</str_replace>"#,
        );
        guide.push_str("\n- Replace existing text with new content.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Why you’re replacing text.\n");
        guide.push_str("  - `path` (required): File to modify (under `/sandbox`).\n");
        guide.push_str("  - `many` (optional, defaults to `false`): Set to `true` to replace every occurrence.\n");
        guide.push_str("  - `<old_str>` / `<new_str>` (required child elements): The original and replacement strings.\n\n");
        guide.push_str("- Use only when the user wants existing text changed and you have already inspected the file to confirm the target.\n\n");

        guide.push_str("### Tool: insert\n");
        guide.push_str("Example template (adapt placeholders; never copy verbatim):\n");
        guide.push_str(
            r#"  <insert commentary="<COMMENTARY_GOES_HERE>" path="/sandbox/<PATH_UNDER_/sandbox>" line="<LINE_NUMBER>"><![CDATA[<TEXT_TO_INSERT>]]></insert>"#,
        );
        guide.push_str("\n- Insert text before the specified 1-based line number.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Why you’re inserting text.\n");
        guide.push_str("  - `path` (required): File path (under `/sandbox`).\n");
        guide.push_str("  - `line` (required): 1-based line number to insert before.\n");
        guide.push_str("  - Body (CDATA): The content to insert.\n");
        guide.push_str("- Call this to insert user-requested snippets at precise locations; avoid speculative edits.\n\n");

        guide.push_str("### Tool: remove_str\n");
        guide.push_str("Example template (adapt placeholders; never copy verbatim):\n");
        guide.push_str(
            r#"  <remove_str commentary="<COMMENTARY_GOES_HERE>" path="/sandbox/<PATH_UNDER_/sandbox>" many="false"><![CDATA[<TEXT_TO_REMOVE>]]></remove_str>"#,
        );
        guide.push_str("\n- Remove matching text snippets.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Why you’re removing text.\n");
        guide.push_str("  - `path` (required): File path (under `/sandbox`).\n");
        guide.push_str("  - `many` (optional, defaults to `false`): Set to `true` to remove all occurrences.\n");
        guide.push_str("  - Body (CDATA): The exact text to remove.\n\n");
        guide.push_str("- Only remove text the user wants gone; verify the snippet exists before attempting removal.\n\n");

        guide.push_str("### Tool: find_filecontent\n");
        guide.push_str("Example template (only run when the user requests a search or you must verify a specific pattern; never copy verbatim):\n");
        guide.push_str(
            r#"  <find_filecontent commentary="<COMMENTARY_GOES_HERE>" path="/sandbox/<PATH_UNDER_/sandbox>" regex="<REGEX_PATTERN>"/>"#,
        );
        guide.push_str("\n- Search inside files for a regex pattern and return matching lines (Rust-style regex syntax).\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Purpose of the search.\n");
        guide.push_str("  - `path` (required): Root directory or file path (under `/sandbox`). Narrow the path when possible.\n");
        guide.push_str("  - `regex` (required): Pattern to match.\n");
        guide.push_str("- Only call this when the user explicitly requests a search or you must verify the presence/absence of specific text.\n\n");

        guide.push_str("### Tool: find_filename\n");
        guide.push_str("Example template (adapt placeholders; never copy verbatim):\n");
        guide.push_str(
            r#"  <find_filename commentary="<COMMENTARY_GOES_HERE>" path="/sandbox/<DIRECTORY>" glob="<GLOB_PATTERN_1>; <GLOB_PATTERN_2>"/>"#,
        );
        guide.push_str("\n- Locate files using glob patterns (semicolon-separated for multiple patterns). Searches recursively from `path`.\n");
        guide.push_str("- Parameters:\n");
        guide.push_str("  - `commentary` (required): Why you’re searching.\n");
        guide.push_str("  - `path` (required): Directory to search (under `/sandbox`).\n");
        guide.push_str("  - `glob` (required): Glob pattern(s) to match file names.\n");
        guide.push_str("- Avoid listing directories unless the user needs the filenames; prefer precise globs and minimize extra calls. Do not run this immediately after creating files just to confirm they exist.\n");
        guide.push_str("- Use to locate files only when the user cannot provide the path and the search directly supports their request.\n\n");

        guide.push_str("### Tool: output\n");
        guide.push_str("Example template (adapt placeholders; never copy verbatim):\n");
        guide.push_str(r#"  <output><![CDATA[<FINAL_MESSAGE_TO_USER>]]></output>"#);
        guide.push_str("\n- Send the final user-facing message once the task is complete.\n");
        guide.push_str(
            "- Body (CDATA) must be JSON with a `commentary` string and a `content` array.\n",
        );
        guide.push_str("- Each entry in `content` must include `type` (`md`, `text`, or `json`) plus a matching `content` payload.\n");
        guide.push_str("- Use `md` for markdown summaries, `text` for short plaintext snippets, and `json` when returning structured data verbatim.\n");

        if !self.mcp_tools.is_empty() {
            guide.push_str("\n### Tool: mcp_call (plus MCP aliases)\n");
            guide.push_str("Dynamic tools are provided by the MCP registry. Prefer these for external APIs/data and CRUD over free-form answers. Use the generic dispatcher when uncertain, or call a specific alias directly.\n");
            guide.push_str("- Generic template:\n");
            guide.push_str(
                r#"  <mcp_call commentary="run MCP tool" server="SERVER_NAME" tool="tool_name"><![CDATA[{"arg":"value"}]]></mcp_call>"#,
            );
            guide.push_str("\n- MCP aliases you can call directly:\n  ");
            let aliases = self
                .mcp_tools
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            guide.push_str(&aliases);
            guide.push('\n');
            guide.push_str("- For aliases, include arguments as attributes or JSON in the body; do not invent tool names beyond this list.\n");
            guide.push_str("- When a router hint or clear match exists, call the hinted MCP tool/alias before trying web_fetch/run_bash for the same data. If an MCP call fails with unknown tool, retry once using the registry tool name (not the alias) before giving up.\n");
            guide.push_str("- Wrap JSON payloads in CDATA inside the element body to avoid malformed JSON errors (e.g., <![CDATA[{\"query\":\"...\"}]]>).\n");
        }
        guide
    }

    pub async fn execute_invocation(
        &self,
        cmd: &CommandInvocation,
    ) -> std::result::Result<ExecutionResult, ExecutionError> {
        if self.is_mcp_tool(cmd) {
            return self.execute_mcp(cmd).await;
        }

        let args = match self.build_args(cmd) {
            Ok(value) => value,
            Err(err) => return Err(ExecutionError::from_error(Value::Null, err)),
        };
        let output = match cmd.name.as_str() {
            "run_bash" => builtin_tools::ShellTool::new().execute(&args).await,
            "open_file" => builtin_tools::OpenFileTool.execute(&args).await,
            "create_file" => builtin_tools::CreateFileTool.execute(&args).await,
            "str_replace" => builtin_tools::StrReplaceTool.execute(&args).await,
            "insert" => builtin_tools::InsertTool.execute(&args).await,
            "remove_str" => builtin_tools::RemoveStrTool.execute(&args).await,
            "find_filecontent" => builtin_tools::FindFilecontentTool.execute(&args).await,
            "find_filename" => builtin_tools::FindFilenameTool.execute(&args).await,
            "web_fetch" => builtin_tools::WebFetchTool::new().execute(&args).await,
            "output" => builtin_tools::OutputTool.execute(&args).await,
            other => Err(anyhow!("unknown tool '{}'", other)),
        };
        match output {
            Ok(output) => Ok(ExecutionResult { args, output }),
            Err(err) => Err(ExecutionError::from_error(args, err)),
        }
    }

    fn is_mcp_tool(&self, cmd: &CommandInvocation) -> bool {
        cmd.name == "mcp_call" || self.mcp_tools.contains_key(&cmd.name)
    }

    pub fn resolve_mcp_metadata(&self, cmd: &CommandInvocation) -> Option<McpRouting> {
        if let Some(alias) = self.mcp_tools.get(&cmd.name) {
            return Some(McpRouting {
                server_name: Some(alias.server_name.clone()),
                tool_name: alias.tool_name.clone(),
            });
        }
        if cmd.name == "mcp_call" {
            let tool_name = cmd.attributes.get("tool")?.to_string();
            let server_name = cmd
                .attributes
                .get("server")
                .cloned()
                .or_else(|| cmd.attributes.get("server_id").cloned());
            return Some(McpRouting {
                server_name,
                tool_name,
            });
        }
        None
    }

    async fn execute_mcp(
        &self,
        cmd: &CommandInvocation,
    ) -> std::result::Result<ExecutionResult, ExecutionError> {
        let client = match &self.mcp_client {
            Some(c) => c.clone(),
            None => {
                return Err(ExecutionError::from_error(
                    Value::Null,
                    anyhow!("MCP is not configured for this sandbox"),
                ))
            }
        };

        let (server_id, server_name, mut tool_name, args) = match self.resolve_mcp_call(cmd) {
            Ok(v) => v,
            Err(err) => return Err(ExecutionError::from_error(Value::Null, err)),
        };

        let mut tried_alt = false;
        let args_for_call = args.clone();

        loop {
            match client
                .invoke(
                    server_id.as_deref(),
                    server_name.as_deref(),
                    &tool_name,
                    args_for_call.clone(),
                    &self.sandbox_id,
                )
                .await
            {
                Ok(output) => {
                    return Ok(ExecutionResult {
                        args: args_for_call,
                        output,
                    })
                }
                Err(err) => {
                    if !tried_alt {
                        if let Some(server) = server_name.clone() {
                            if let Some(alternate) =
                                self.preferred_tool_for_server(&server, &tool_name)
                            {
                                if alternate != tool_name {
                                    tried_alt = true;
                                    tool_name = alternate;
                                    continue;
                                }
                            }
                        }
                    }
                    return Err(ExecutionError::from_error(args.clone(), err));
                }
            }
        }
    }

    fn resolve_mcp_call(
        &self,
        cmd: &CommandInvocation,
    ) -> Result<(Option<String>, Option<String>, String, Value)> {
        if let Some(alias) = self.mcp_tools.get(&cmd.name) {
            let args = build_mcp_args(cmd, &["commentary"])?;
            return Ok((
                Some(alias.server_id.clone()),
                Some(alias.server_name.clone()),
                alias.tool_name.clone(),
                args,
            ));
        }

        if cmd.name == "mcp_call" {
            let server_id = cmd.attributes.get("server_id").cloned();
            let server_name = cmd.attributes.get("server").cloned();
            if server_id.is_none() && server_name.is_none() {
                return Err(anyhow!(
                    "mcp_call requires either 'server_id' or 'server' attribute"
                ));
            }
            let mut tool_name = require_attr(&cmd.attributes, "tool")?;

            if let Some(alias) = self.mcp_tools.get(&tool_name) {
                let args = build_mcp_args(cmd, &["server", "server_id", "tool", "commentary"])?;
                return Ok((
                    Some(alias.server_id.clone()),
                    Some(alias.server_name.clone()),
                    alias.tool_name.clone(),
                    args,
                ));
            }
            let args = build_mcp_args(cmd, &["server", "server_id", "tool", "commentary"])?;
            if let Some(server) = server_name.as_ref().or_else(|| server_id.as_ref()).cloned() {
                if let Some(alternate) = self.preferred_tool_for_server(&server, &tool_name) {
                    tool_name = alternate;
                }
            }
            return Ok((server_id, server_name, tool_name, args));
        }

        Err(anyhow!("unknown MCP tool '{}'", cmd.name))
    }

    fn build_args(&self, cmd: &CommandInvocation) -> Result<Value> {
        let mut map = Map::new();
        let attrs = &cmd.attributes;
        let body = cmd.body.clone().unwrap_or_default();
        let child_map = children_to_map(&cmd.children);

        match cmd.name.as_str() {
            "run_bash" => {
                let commentary = require_attr(attrs, "commentary")?;
                let exec_dir = attrs
                    .get("exec_dir")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "/sandbox".to_string());
                if !exec_dir.starts_with("/sandbox") {
                    return Err(anyhow!("run_bash exec_dir must start with /sandbox"));
                }
                let commands = require_attr(attrs, "commands")?;
                map.insert("commentary".into(), Value::String(commentary));
                map.insert("exec_dir".into(), Value::String(exec_dir));
                map.insert("commands".into(), Value::String(commands));
            }
            "open_file" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                if let Some(start) = attrs.get("start_line") {
                    map.insert("start_line".into(), Value::Number(parse_u64(start)?.into()));
                }
                if let Some(end) = attrs.get("end_line") {
                    map.insert("end_line".into(), Value::Number(parse_u64(end)?.into()));
                }
            }
            "create_file" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                if attrs.contains_key("body") {
                    return Err(anyhow!(
                        "create_file content must be provided in the element body (<![CDATA[...]]>), not a 'body' attribute"
                    ));
                }
                map.insert("content".into(), Value::String(body));
            }
            "str_replace" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                let old_str = require_child(&child_map, "old_str")?;
                let new_str = require_child(&child_map, "new_str")?;
                map.insert("old_str".into(), Value::String(old_str));
                map.insert("new_str".into(), Value::String(new_str));
                if let Some(many) = attrs.get("many") {
                    map.insert("many".into(), Value::Bool(parse_bool(many)?));
                }
            }
            "insert" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                let line = require_attr(attrs, "line")?;
                map.insert(
                    "insert_line".into(),
                    Value::Number(parse_u64(&line)?.into()),
                );
                map.insert("content".into(), Value::String(body));
            }
            "remove_str" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                map.insert("content".into(), Value::String(body));
                if let Some(many) = attrs.get("many") {
                    map.insert("many".into(), Value::Bool(parse_bool(many)?));
                }
            }
            "find_filecontent" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                map.insert("regex".into(), Value::String(require_attr(attrs, "regex")?));
            }
            "find_filename" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                map.insert("path".into(), Value::String(require_attr(attrs, "path")?));
                map.insert("glob".into(), Value::String(require_attr(attrs, "glob")?));
            }
            "output" => {
                map.insert("commentary".into(), Value::String("final response".into()));
                map.insert(
                    "content".into(),
                    Value::Array(vec![json!({
                        "type": "markdown",
                        "title": "Result",
                        "content": body
                    })]),
                );
            }
            "web_fetch" => {
                map.insert(
                    "commentary".into(),
                    Value::String(require_attr(attrs, "commentary")?),
                );
                let url = require_attr(attrs, "url")?;
                map.insert("url".into(), Value::String(url));
                if let Some(timeout) = attrs.get("timeout_seconds") {
                    map.insert(
                        "timeout_seconds".into(),
                        Value::Number(parse_u64(timeout)?.into()),
                    );
                }
                if let Some(max_bytes) = attrs.get("max_bytes") {
                    map.insert(
                        "max_bytes".into(),
                        Value::Number(parse_u64(max_bytes)?.into()),
                    );
                }
                if let Some(segment) = attrs.get("segment") {
                    map.insert("segment".into(), Value::String(segment.to_string()));
                }
                if let Some(offset) = attrs.get("offset_bytes") {
                    map.insert(
                        "offset_bytes".into(),
                        Value::Number(parse_u64(offset)?.into()),
                    );
                }
            }
            other => return Err(anyhow!("unknown tool '{}'", other)),
        }

        Ok(Value::Object(map))
    }
}

fn require_attr(attrs: &HashMap<String, String>, key: &str) -> Result<String> {
    attrs
        .get(key)
        .cloned()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("missing '{}' attribute", key))
}

fn parse_u64(value: &str) -> Result<u64> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| anyhow!("invalid integer '{}'", value))
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        other => Err(anyhow!("invalid boolean '{}'", other)),
    }
}

fn children_to_map(children: &[CommandChild]) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for child in children {
        map.entry(child.name.to_string())
            .or_default()
            .push(child.content.clone());
    }
    map
}

fn require_child(children: &HashMap<String, Vec<String>>, key: &str) -> Result<String> {
    children
        .get(key)
        .and_then(|vals| vals.first().cloned())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("missing <{}>...</{}> block", key, key))
}

fn build_mcp_args(cmd: &CommandInvocation, skip_keys: &[&str]) -> Result<Value> {
    if let Some(body) = cmd.body.as_ref() {
        let trimmed = body.trim();
        if !trimmed.is_empty() {
            let parsed: Value = serde_json::from_str(trimmed)
                .context("invalid JSON body for MCP tool (wrap in CDATA)")?;
            return Ok(parsed);
        }
    }

    let mut map = Map::new();
    let skip: HashSet<&str> = skip_keys.iter().copied().collect();
    for (k, v) in &cmd.attributes {
        if skip.contains(k.as_str()) {
            continue;
        }
        map.insert(k.clone(), Value::String(v.clone()));
    }
    for child in &cmd.children {
        if skip.contains(child.name.as_str()) {
            continue;
        }
        if !child.content.trim().is_empty() {
            map.insert(child.name.clone(), Value::String(child.content.clone()));
        }
    }
    Ok(Value::Object(map))
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

fn intent_match_score(
    raw_text: &str,
    user_tokens: &[String],
    alias: &McpToolAlias,
) -> Option<usize> {
    let mut score: usize = 0;
    let lower_text = raw_text.to_lowercase();
    let alias_phrase = alias.alias.replace('_', " ").to_lowercase();
    let tool_phrase = alias.tool_name.replace('_', " ").to_lowercase();

    if !alias_phrase.is_empty() && lower_text.contains(&alias_phrase) {
        score += 2;
    }
    if !tool_phrase.is_empty() && lower_text.contains(&tool_phrase) {
        score += 2;
    }

    let mut corpus = tokenize(&alias.tool_name);
    corpus.extend(tokenize(&alias.server_name));
    if let Some(desc) = &alias.description {
        corpus.extend(tokenize(desc));
    }

    let filtered_user_tokens: Vec<&String> = user_tokens
        .iter()
        .filter(|t| !STOPWORDS.contains(&t.as_str()))
        .collect();

    let matched_tokens = filtered_user_tokens
        .iter()
        .filter(|token| corpus.iter().any(|c| token_match(token, &c)))
        .count();

    score += matched_tokens;

    if score == 0 {
        None
    } else {
        Some(score)
    }
}

fn tokenize(input: &str) -> Vec<String> {
    input
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter_map(|token| {
            let normalized = normalize_token(token);
            if normalized.is_empty() {
                None
            } else {
                Some(normalized)
            }
        })
        .collect()
}

fn normalize_token(token: &str) -> String {
    let trimmed = token.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    if trimmed.is_empty() {
        return String::new();
    }
    let lower = trimmed.to_lowercase();
    if lower.ends_with("ies") && lower.len() > 3 {
        format!("{}y", &lower[..lower.len() - 3])
    } else if lower.ends_with('s') && lower.len() > 3 {
        lower[..lower.len() - 1].to_string()
    } else {
        lower
    }
}

fn token_match(a: &str, b: &str) -> bool {
    a == b || a.starts_with(b) || b.starts_with(a)
}

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "can", "for", "give", "how", "i", "in", "is", "it", "me", "my", "of",
    "on", "please", "show", "some", "tell", "that", "the", "this", "to", "what", "with", "you",
];

fn lower_contains_all(text: &str, phrases: &[&str]) -> bool {
    let lower = text.to_lowercase();
    phrases.iter().all(|p| lower.contains(p))
}

const MCP_SYNONYMS: &[(&[&str], &str)] = &[
    (&["list repos"], "search_repositories"),
    (&["list repositories"], "search_repositories"),
    (&["my repos"], "search_repositories"),
    (&["github repos"], "search_repositories"),
    (&["github repositories"], "search_repositories"),
    (&["list github repos"], "search_repositories"),
    (&["list github repositories"], "search_repositories"),
    (&["who am i"], "get_me"),
    (&["whoami"], "get_me"),
    (&["my profile"], "get_me"),
    (&["my user"], "get_me"),
    (&["list issues"], "search_issues"),
    (&["search issues"], "search_issues"),
    (&["my prs"], "search_pull_requests"),
    (&["my pull requests"], "search_pull_requests"),
    (&["issue details"], "issue_read"),
    (&["issue number"], "issue_read"),
    (&["get issue"], "issue_read"),
    (&["read issue"], "issue_read"),
];

impl ToolCatalog {
    fn canonical_tool_from_text(&self, text: &str) -> Option<String> {
        for (phrases, tool) in MCP_SYNONYMS {
            if lower_contains_all(text, phrases) && self.any_server_has_tool(tool) {
                return Some(tool.to_string());
            }
        }

        // Token-based fallback for common repo listing asks (e.g., "list my GitHub repos")
        let tokens = tokenize(text);
        if self.any_server_has_tool("search_repositories") {
            let mentions_repo = tokens.iter().any(|t| t.starts_with("repo"));
            let mentions_list = tokens
                .iter()
                .any(|t| matches!(t.as_str(), "list" | "fetch" | "get" | "show"));
            if mentions_repo && mentions_list {
                return Some("search_repositories".to_string());
            }
        }
        None
    }

    fn any_server_has_tool(&self, tool: &str) -> bool {
        self.server_tool_index
            .values()
            .any(|tools| tools.iter().any(|t| t.eq_ignore_ascii_case(tool)))
    }

    fn server_has_tool(&self, server: &str, tool: &str) -> bool {
        self.server_tool_index
            .get(server)
            .map(|tools| tools.iter().any(|t| t.eq_ignore_ascii_case(tool)))
            .unwrap_or(false)
    }

    fn preferred_tool_for_server(&self, server: &str, tool: &str) -> Option<String> {
        // Strip server prefix if present (e.g., github_get_me -> get_me)
        let lower_tool = tool.to_lowercase();
        let server_prefix = format!("{}_", server.to_lowercase());
        let stripped = if lower_tool.starts_with(&server_prefix) {
            lower_tool.trim_start_matches(&server_prefix).to_string()
        } else {
            tool.to_string()
        };

        if self.server_has_tool(server, &stripped) {
            return Some(stripped);
        }
        if self.server_has_tool(server, tool) {
            return Some(tool.to_string());
        }
        let lower = tool.to_lowercase();
        for (phrases, canonical) in MCP_SYNONYMS {
            if canonical.eq_ignore_ascii_case(tool) {
                continue;
            }
            if phrases.iter().any(|p| lower.contains(p)) && self.server_has_tool(server, canonical)
            {
                return Some(canonical.to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::mcp::McpTool;
    use std::collections::HashMap;

    fn sample_tool(server: &str, name: &str, desc: &str) -> McpTool {
        McpTool {
            id: format!("{}_{}", server, name),
            server_id: format!("{}-id", server.to_lowercase()),
            server_name: server.to_string(),
            name: name.to_string(),
            description: Some(desc.to_string()),
            input_schema: None,
            output_schema: None,
            metadata: None,
            version: None,
        }
    }

    #[test]
    fn routes_direct_match() {
        let tools = vec![sample_tool(
            "GitHub",
            "list_repositories",
            "List repositories for the current user",
        )];
        let catalog = ToolCatalog::new("sandbox".into(), None, tools);
        let hint = catalog
            .intent_router_hint("can you list my repos?", None)
            .expect("expected a router hint");

        assert_eq!(
            hint,
            IntentRouterHint::Direct {
                alias: "mcp_github_list_repositories".into(),
                server_name: "GitHub".into(),
                tool_name: "list_repositories".into()
            }
        );
    }

    #[test]
    fn routes_ambiguous_server() {
        let tools = vec![
            sample_tool(
                "GitHub",
                "list_repositories",
                "List repositories for the current user",
            ),
            sample_tool(
                "GitLab",
                "list_repositories",
                "List repositories for the current user",
            ),
        ];
        let catalog = ToolCatalog::new("sandbox".into(), None, tools);
        let hint = catalog
            .intent_router_hint("list my repositories", None)
            .expect("expected an ambiguous router hint");

        assert_eq!(
            hint,
            IntentRouterHint::Ambiguous {
                tool_name: "list_repositories".into(),
                servers: vec!["GitHub".into(), "GitLab".into()]
            }
        );
    }

    #[test]
    fn ignores_irrelevant_requests() {
        let tools = vec![sample_tool(
            "GitHub",
            "list_repositories",
            "List repositories for the current user",
        )];
        let catalog = ToolCatalog::new("sandbox".into(), None, tools);
        assert!(catalog.intent_router_hint("hello there", None).is_none());
    }

    #[tokio::test]
    async fn mcp_call_accepts_alias_in_tool_attribute() {
        let tools = vec![sample_tool(
            "GitHub",
            "search_repositories",
            "Search repositories",
        )];
        let catalog = ToolCatalog::new("sandbox".into(), None, tools);

        let cmd = CommandInvocation {
            name: "mcp_call".into(),
            attributes: HashMap::from([
                ("tool".into(), "mcp_github_search_repositories".into()),
                ("server".into(), "github".into()),
            ]),
            body: None,
            children: vec![],
        };

        let (server_id, server_name, tool_name, args) = catalog
            .resolve_mcp_call(&cmd)
            .expect("expected alias resolution");

        assert_eq!(server_id.unwrap(), "github-id");
        assert_eq!(server_name.unwrap(), "GitHub");
        assert_eq!(tool_name, "search_repositories");
        assert_eq!(args, serde_json::json!({}));
    }
}
