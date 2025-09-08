use super::error::{HostError, Result};
use super::ollama::ChatMessage;
use openai_harmony::{
    load_harmony_encoding, HarmonyEncodingName,
    chat::{Role, Message, Conversation}
};
use tracing::{info, warn};
use serde_json::Value;

/// Harmony format client for GPT-OSS models
#[derive(Clone)]
pub struct HarmonyClient {
    encoding: openai_harmony::HarmonyEncoding,
}

/// Multi-channel response from harmony format  
#[derive(Debug)]
pub struct HarmonyResponse {
    pub final_content: String,
    pub analysis_thinking: Option<String>,
    pub tool_calls_found: bool,
}

impl HarmonyClient {
    /// Create new harmony client with GPT-OSS encoding
    pub fn new() -> Result<Self> {
        let encoding = load_harmony_encoding(HarmonyEncodingName::HarmonyGptOss)
            .map_err(|e| HostError::Model(format!("Failed to load harmony encoding: {}", e)))?;
            
        Ok(Self {
            encoding,
        })
    }
    
    /// Basic harmony format detection in text responses
    pub fn detect_harmony_channels(&self, content: &str) -> HarmonyResponse {
        let mut final_content = String::new();
        let mut analysis_thinking = None;
        let tool_calls_found = content.contains("<|channel|>commentary");
        
        // Simple channel detection for now
        if content.contains("<|channel|>final") {
            // Extract final channel content
            if let Some(start) = content.find("<|channel|>final<|message|>") {
                let content_start = start + 27; // Length of "<|channel|>final<|message|>"
                if let Some(end) = content[content_start..].find("<|end|>") {
                    final_content = content[content_start..content_start + end].to_string();
                } else {
                    final_content = content[content_start..].to_string();
                }
            }
        }
        
        if content.contains("<|channel|>analysis") {
            // Extract analysis channel content  
            if let Some(start) = content.find("<|channel|>analysis<|message|>") {
                let content_start = start + 30; // Length of "<|channel|>analysis<|message|>"
                if let Some(end) = content[content_start..].find("<|end|>") {
                    analysis_thinking = Some(content[content_start..content_start + end].to_string());
                } else {
                    analysis_thinking = Some(content[content_start..].to_string());
                }
            }
        }
        
        // If no channels found, treat entire content as final
        if final_content.is_empty() && analysis_thinking.is_none() {
            final_content = content.to_string();
        }
        
        HarmonyResponse {
            final_content,
            analysis_thinking,
            tool_calls_found,
        }
    }
    
    /// Convert ChatMessages to Harmony format and render as tokens
    pub fn render_conversation_tokens(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<String>,
        tools: Vec<HarmonyTool>,
    ) -> Result<Vec<u32>> {
        let conversation = self.create_harmony_conversation(messages, system_prompt, tools)?;
        
        self.encoding
            .render_conversation_for_completion(&conversation, Role::Assistant, None)
            .map_err(|e| HostError::Model(format!("Failed to render harmony conversation: {}", e)))
    }
    
    /// Parse token response back to our format
    pub fn parse_token_response(&self, tokens: Vec<u32>) -> Result<HarmonyResponse> {
        let messages = self.encoding
            .parse_messages_from_completion_tokens(tokens, Some(Role::Assistant))
            .map_err(|e| HostError::Model(format!("Failed to parse harmony response tokens: {}", e)))?;
        
        self.extract_channels_from_messages(messages)
    }
    
    /// Create harmony conversation from our messages
    fn create_harmony_conversation(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<String>,
        tools: Vec<HarmonyTool>,
    ) -> Result<Conversation> {
        let mut harmony_messages = Vec::new();
        
        // Add system message
        if let Some(system_content) = system_prompt {
            let system_msg = Message::from_role_and_content(Role::System, system_content);
            harmony_messages.push(system_msg);
        }
        
        // Add tool definitions as developer message if tools exist
        if !tools.is_empty() {
            info!("Adding {} tools to harmony conversation", tools.len());
            let tools_content = self.format_tools_for_developer_message(&tools);
            info!("Developer message content: {}", tools_content);
            let developer_msg = Message::from_role_and_content(Role::Developer, tools_content);
            harmony_messages.push(developer_msg);
        } else {
            info!("No tools provided for harmony conversation");
        }
        
        // Convert our chat messages
        for chat_msg in messages {
            let role = match chat_msg.role.as_str() {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "tool" => Role::Tool,
                _ => Role::User,
            };
            
            let harmony_msg = Message::from_role_and_content(role, chat_msg.content);
            harmony_messages.push(harmony_msg);
        }
        
        let conversation = Conversation::from_messages(harmony_messages);
        Ok(conversation)
    }
    
    /// Format tools for developer message
    fn format_tools_for_developer_message(&self, tools: &[HarmonyTool]) -> String {
        let mut content = String::from("# Tools\n\nnamespace functions {\n\n");
        
        for tool in tools {
            // Ensure tool has valid name
            if tool.name.is_empty() {
                warn!("Skipping tool with empty name: {}", tool.description);
                continue;
            }
            
            content.push_str(&format!(
                "// {}\ntype {} = ({}) => any;\n\n",
                tool.description,
                tool.name,
                self.format_tool_parameters(&tool.parameters)
            ));
        }
        
        content.push_str("} // namespace functions");
        
        info!("Formatted {} tools for harmony developer message", tools.len());
        content
    }
    
    /// Format tool parameters for harmony format
    fn format_tool_parameters(&self, params: &Value) -> String {
        // Basic parameter formatting - could be enhanced
        if let Some(properties) = params.get("properties") {
            if let Some(obj) = properties.as_object() {
                let param_strs: Vec<String> = obj.iter()
                    .map(|(name, schema)| {
                        let type_str = schema.get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("any");
                        format!("{}: {}", name, type_str)
                    })
                    .collect();
                return format!("_: {{ {} }}", param_strs.join(", "));
            }
        }
        "_: any".to_string()
    }
    
    /// Extract channels from harmony messages
    fn extract_channels_from_messages(&self, messages: Vec<Message>) -> Result<HarmonyResponse> {
        let mut final_content = String::new();
        let mut analysis_thinking = None;
        let mut tool_calls_found = false;
        
        for message in messages {
            let content_text = self.extract_message_text(&message);
            
            match message.channel.as_deref() {
                Some("final") => final_content.push_str(&content_text),
                Some("analysis") => {
                    if !content_text.trim().is_empty() {
                        analysis_thinking = Some(content_text);
                    }
                }
                Some("commentary") => {
                    tool_calls_found = true;
                    info!("Commentary channel detected: {}", content_text);
                }
                None => {
                    // No channel specified - treat as final content
                    final_content.push_str(&content_text);
                }
                _ => {
                    warn!("Unknown harmony channel: {:?}", message.channel);
                }
            }
        }
        
        Ok(HarmonyResponse {
            final_content,
            analysis_thinking,
            tool_calls_found,
        })
    }
    
    /// Extract text content from harmony message (simplified)
    fn extract_message_text(&self, message: &Message) -> String {
        // For now, assume message content is text-like
        // This is a simplified implementation - the actual harmony library
        // would have proper content extraction methods
        format!("{:?}", message.content).trim_matches('"').to_string()
    }
}

/// Tool definition for harmony format
#[derive(Debug, Clone)]
pub struct HarmonyTool {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}