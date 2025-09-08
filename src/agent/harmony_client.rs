use super::error::{HostError, Result};
use openai_harmony::{load_harmony_encoding, HarmonyEncodingName};
use tracing::warn;

/// Harmony format client for GPT-OSS models
pub struct HarmonyClient {
    _encoding: openai_harmony::HarmonyEncoding,
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
            _encoding: encoding,
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
}