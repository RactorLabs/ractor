use super::super::error::{HostError, Result};
use super::super::inference::{ChatMessage, ModelResponse};
use super::InferenceTemplate;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Serialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    tools: Vec<Value>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
    tool_calls: Option<Vec<Value>>,
}

#[derive(Debug, Deserialize, Default)]
struct Usage {
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
}

pub struct DefaultTemplate {}

impl DefaultTemplate {
    pub fn new() -> Self {
        Self {}
    }

    fn build_tools_json() -> Vec<Value> {
        vec![
            json!({
                "type": "function",
                "function": {
                    "name": "run_bash",
                    "description": "Execute shell command(s) in the sandbox",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "commentary": {
                                "type": "string",
                                "description": "Short description of the action being performed"
                            },
                            "exec_dir": {
                                "type": "string",
                                "description": "Directory to run command in (must be /sandbox or subdirectory)"
                            },
                            "commands": {
                                "type": "string",
                                "description": "Shell command(s) to execute"
                            }
                        },
                        "required": ["commentary", "exec_dir", "commands"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "open_file",
                    "description": "Read file contents",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "commentary": {
                                "type": "string",
                                "description": "Why you're reading the file"
                            },
                            "path": {
                                "type": "string",
                                "description": "Absolute path to file (must be under /sandbox)"
                            },
                            "start_line": {
                                "type": "integer",
                                "description": "Optional starting line number"
                            },
                            "end_line": {
                                "type": "integer",
                                "description": "Optional ending line number"
                            }
                        },
                        "required": ["commentary", "path"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "create_file",
                    "description": "Create a new file with content",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "commentary": {
                                "type": "string",
                                "description": "Why you're creating the file"
                            },
                            "path": {
                                "type": "string",
                                "description": "Absolute path for new file (must be under /sandbox)"
                            },
                            "content": {
                                "type": "string",
                                "description": "File content"
                            }
                        },
                        "required": ["commentary", "path", "content"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "str_replace",
                    "description": "Replace text in a file",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "commentary": {
                                "type": "string",
                                "description": "What you're replacing"
                            },
                            "path": {
                                "type": "string",
                                "description": "Path to file"
                            },
                            "old_str": {
                                "type": "string",
                                "description": "Text to find (must match exactly)"
                            },
                            "new_str": {
                                "type": "string",
                                "description": "Replacement text"
                            }
                        },
                        "required": ["commentary", "path", "old_str", "new_str"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "insert",
                    "description": "Insert text at a line in a file",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "commentary": {
                                "type": "string",
                                "description": "What you're inserting"
                            },
                            "path": {
                                "type": "string",
                                "description": "Path to file"
                            },
                            "insert_line": {
                                "type": "integer",
                                "description": "Line number to insert at"
                            },
                            "new_str": {
                                "type": "string",
                                "description": "Text to insert"
                            }
                        },
                        "required": ["commentary", "path", "insert_line", "new_str"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "find_filecontent",
                    "description": "Search file contents for text pattern",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "commentary": {
                                "type": "string",
                                "description": "What you're searching for"
                            },
                            "path": {
                                "type": "string",
                                "description": "Directory to search in"
                            },
                            "pattern": {
                                "type": "string",
                                "description": "Text pattern to search for"
                            },
                            "glob_filter": {
                                "type": "string",
                                "description": "Optional file glob filter (e.g. '*.rs')"
                            }
                        },
                        "required": ["commentary", "path", "pattern"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "find_filename",
                    "description": "Search for files by name pattern",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "commentary": {
                                "type": "string",
                                "description": "What you're looking for"
                            },
                            "path": {
                                "type": "string",
                                "description": "Directory to search in"
                            },
                            "pattern": {
                                "type": "string",
                                "description": "Filename pattern (supports wildcards)"
                            }
                        },
                        "required": ["commentary", "path", "pattern"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "output",
                    "description": "Return final result to the user",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "content": {
                                "type": "string",
                                "description": "The result or message to return"
                            }
                        },
                        "required": ["content"]
                    }
                }
            }),
        ]
    }
}

#[async_trait]
impl InferenceTemplate for DefaultTemplate {
    async fn build_request(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<String>,
        model_name: &str,
    ) -> Result<serde_json::Value> {
        let mut request_messages: Vec<Message> = Vec::new();

        // Simplified system prompt for GPT-OSS models
        let simple_system_prompt = system_prompt.unwrap_or_else(|| {
            "You are TaskSandbox, a helpful AI assistant that executes tasks using the provided tools. \
             Always respond by calling one of the available functions. All file operations must be under /sandbox directory. \
             Use the 'output' function to return final results to the user.".to_string()
        });

        request_messages.push(Message {
            role: "system".to_string(),
            content: simple_system_prompt,
        });

        for msg in messages.iter() {
            let trimmed = msg.content.trim();
            if trimmed.is_empty() {
                continue;
            }
            request_messages.push(Message {
                role: msg.role.clone(),
                content: trimmed.to_string(),
            });
        }

        if request_messages.len() <= 1 {
            // Only system message
            return Err(HostError::Model("No user messages provided".to_string()));
        }

        let req = ChatRequest {
            model: model_name.to_string(),
            messages: request_messages,
            tools: Self::build_tools_json(),
            stream: false,
        };

        serde_json::to_value(&req)
            .map_err(|e| HostError::Model(format!("Failed to serialize request: {}", e)))
    }

    async fn parse_response(
        &self,
        response_text: &str,
        estimated_context_length: i64,
    ) -> Result<ModelResponse> {
        let parsed: ChatResponse = serde_json::from_str(response_text)
            .map_err(|e| HostError::Model(format!("Failed to parse response: {}", e)))?;

        let choice = parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| HostError::Model("Inference response missing choices".into()))?;

        // For default template, we need to convert tool calls to XML format
        let content = if let Some(tool_calls) = &choice.message.tool_calls {
            if let Some(tool_call) = tool_calls.first() {
                self.convert_tool_call_to_xml(tool_call)?
            } else {
                choice.message.content.unwrap_or_default()
            }
        } else {
            choice.message.content.unwrap_or_default()
        };

        let usage = parsed.usage.unwrap_or_default();
        let context_length = usage
            .prompt_tokens
            .or(usage.total_tokens)
            .unwrap_or(estimated_context_length);

        Ok(ModelResponse {
            content: content.trim().to_string(),
            total_tokens: usage.total_tokens,
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            context_length: Some(context_length.max(0)),
        })
    }

    fn format_hint(&self) -> &str {
        "Please use one of the available function calls to respond."
    }
}

impl DefaultTemplate {
    fn convert_tool_call_to_xml(&self, tool_call: &Value) -> Result<String> {
        let function = tool_call
            .get("function")
            .ok_or_else(|| HostError::Model("Tool call missing function".to_string()))?;

        let name = function
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::Model("Tool call missing function name".to_string()))?;

        let arguments_str = function
            .get("arguments")
            .and_then(|v| v.as_str())
            .unwrap_or("{}");

        let args: Value = serde_json::from_str(arguments_str)
            .map_err(|e| HostError::Model(format!("Failed to parse tool arguments: {}", e)))?;

        // Convert function call to XML format expected by the sandbox
        match name {
            "output" => {
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Ok(format!("<output>{}</output>", Self::escape_xml(content)))
            }
            _ => {
                // For other tools, create XML with attributes
                let commentary = args
                    .get("commentary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let mut xml = format!("<{} commentary=\"{}\"", name, Self::escape_xml_attr(commentary));

                // Add other attributes
                if let Some(obj) = args.as_object() {
                    for (key, value) in obj {
                        if key == "commentary" || key == "content" {
                            continue;
                        }
                        if let Some(s) = value.as_str() {
                            xml.push_str(&format!(" {}=\"{}\"", key, Self::escape_xml_attr(s)));
                        } else if let Some(n) = value.as_i64() {
                            xml.push_str(&format!(" {}=\"{}\"", key, n));
                        }
                    }
                }

                // Check if there's content that should go in the body
                if let Some(content) = args.get("content").and_then(|v| v.as_str()) {
                    xml.push('>');
                    xml.push_str(&Self::escape_xml(content));
                    xml.push_str(&format!("</{}>", name));
                } else {
                    xml.push_str("/>");
                }

                Ok(xml)
            }
        }
    }

    fn escape_xml(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    fn escape_xml_attr(s: &str) -> String {
        Self::escape_xml(s).replace('"', "&quot;")
    }
}
