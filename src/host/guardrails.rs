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

        // Only redact critical sensitive information
        let sensitive_keywords = vec![
            "anthropic_api_key",
            "api_key",
            "raworc_token",
            "jwt",
            "bearer",
            "mysql://",
        ];

        for keyword in sensitive_keywords {
            if sanitized.to_lowercase().contains(keyword) {
                // Find and replace the pattern
                let lower = sanitized.to_lowercase();
                if let Some(idx) = lower.find(keyword) {
                    let end_idx = idx + keyword.len();
                    // Find the value part (after : or =)
                    let mut value_start = end_idx;
                    let chars: Vec<char> = sanitized[end_idx..].chars().collect();
                    for (i, c) in chars.iter().enumerate() {
                        if !c.is_whitespace() && *c != ':' && *c != '=' {
                            value_start = end_idx + i;
                            break;
                        }
                    }

                    // Find end of value
                    let mut value_end = value_start;
                    let value_chars: Vec<char> = sanitized[value_start..].chars().collect();
                    for (i, c) in value_chars.iter().enumerate() {
                        if c.is_whitespace() || *c == ',' || *c == ';' || *c == '}' {
                            value_end = value_start + i;
                            break;
                        }
                        if i > 50 {
                            value_end = value_start + 50;
                            break;
                        }
                    }

                    if value_end > value_start {
                        let before = &sanitized[..value_start];
                        let after = &sanitized[value_end..];
                        sanitized = format!("{}[REDACTED]{}", before, after);
                    }
                }
            }
        }

        // Ensure reasonable length
        if sanitized.len() > self.max_message_length {
            sanitized.truncate(self.max_message_length);
            sanitized.push_str("\n[Message truncated due to length]");
        }

        sanitized
    }

    /// Check if the input is asking for system prompt or internal information
    pub fn check_system_prompt_exposure(&self, content: &str) -> Result<()> {
        let lower_content = content.to_lowercase();

        let prompt_exposure_patterns = [
            "system prompt",
            "system message",
            "your instructions",
            "your prompt",
            "repeat your instructions",
            "show me your prompt",
            "anthropic_api_key",
            "api_key",
            "raworc_token",
        ];

        for pattern in prompt_exposure_patterns {
            if lower_content.contains(pattern) {
                warn!("System prompt exposure attempt detected: {}", pattern);
                return Err(HostError::Guardrail(
                    "Cannot provide system configuration or internal details".to_string(),
                ));
            }
        }

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
    pub fn check_system_info_leakage(&self, content: &str) -> Result<()> {
        let lower_content = content.to_lowercase();

        let system_info_patterns = [
            "anthropic_api_key",
            "api_key",
            "raworc_token",
            "jwt secret",
            "bearer token",
            "mysql://",
        ];

        for pattern in system_info_patterns {
            if lower_content.contains(pattern) {
                warn!("Critical system information leakage detected: {}", pattern);
                return Err(HostError::Guardrail(
                    "Response contains sensitive system information".to_string(),
                ));
            }
        }

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
