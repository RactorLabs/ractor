use super::error::{HostError, Result};
use tracing::{debug, warn};

pub struct Guardrails {
    max_message_length: usize,
}

impl Guardrails {
    pub fn new() -> Self {
        Self {
            max_message_length: 100_000,
        }
    }

    /// Check if content is within size limits
    pub fn check_message_size(&self, content: &str) -> Result<()> {
        if content.len() > self.max_message_length {
            return Err(HostError::Guardrail(format!(
                "Message exceeds maximum length of {} characters",
                self.max_message_length
            )));
        }
        Ok(())
    }

    /// Sanitize content before sending
    pub fn sanitize_output(&self, content: &str) -> String {
        let mut sanitized = content.to_string();

        // Ensure reasonable length
        if sanitized.len() > self.max_message_length {
            sanitized.truncate(self.max_message_length);
            sanitized.push_str("\n[Message truncated due to length]");
        }

        sanitized
    }

    /// Check if the input is asking for system prompt or internal information
    pub fn check_system_prompt_exposure(&self, _content: &str) -> Result<()> {
        // System prompt exposure checks disabled for more open agent behavior
        Ok(())
    }

    /// Check for critical system-level destructive commands only
    pub fn check_system_safety(&self, content: &str) -> Result<()> {
        let harmful_patterns = [
            "rm -rf /",
            "format c:",
            ":(){:|:&};:", // Fork bomb
            "dd if=/dev/zero of=/dev/sda",
            "mkfs /dev/sda",
        ];

        let lower_content = content.to_lowercase();

        for pattern in harmful_patterns {
            if lower_content.contains(pattern) {
                warn!("Critical system destructive command detected: {}", pattern);
                return Err(HostError::Guardrail(
                    "Request contains system-destructive commands".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Validate all guardrails for input
    pub fn validate_input(&self, content: &str) -> Result<()> {
        debug!("Validating input with minimal guardrails");

        self.check_message_size(content)?;
        self.check_system_prompt_exposure(content)?;
        self.check_system_safety(content)?;

        Ok(())
    }

    /// Check if output contains critical system information that should be filtered
    pub fn check_system_info_leakage(&self, _content: &str) -> Result<()> {
        // System info leakage checks disabled for more open agent behavior
        Ok(())
    }

    /// Validate all guardrails for output
    pub fn validate_output(&self, content: &str) -> Result<String> {
        debug!("Validating output with minimal guardrails");

        self.check_message_size(content)?;
        self.check_system_info_leakage(content)?;

        let sanitized = self.sanitize_output(content);
        Ok(sanitized)
    }
}
