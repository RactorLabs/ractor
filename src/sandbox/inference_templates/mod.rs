pub mod openai;
pub mod positron;

use super::error::Result;
use super::inference::{ChatMessage, ModelResponse};
use async_trait::async_trait;

#[async_trait]
pub trait InferenceTemplate: Send + Sync {
    /// Build the request payload for the inference API
    async fn build_request(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<String>,
        model_name: &str,
    ) -> Result<serde_json::Value>;

    /// Parse the response from the inference API
    async fn parse_response(
        &self,
        response_text: &str,
        estimated_context_length: i64,
    ) -> Result<ModelResponse>;

    /// Get the format hint message for retry attempts
    fn format_hint(&self) -> &str;

    /// Guidance snippet for system prompt about how the model should format tool calls
    fn system_prompt_guidance(&self) -> String;
}

pub fn get_template(template_name: &str) -> Result<Box<dyn InferenceTemplate>> {
    match template_name {
        "positron" => Ok(Box::new(positron::PositronTemplate::new())),
        "openai" => Ok(Box::new(openai::OpenAiTemplate::new())),
        "" => Ok(Box::new(openai::OpenAiTemplate::new())),
        _ => Err(super::error::HostError::Model(format!(
            "Unknown inference template: {}. Supported: 'openai', 'positron'",
            template_name
        ))),
    }
}
