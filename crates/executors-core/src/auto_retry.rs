use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[schemars(
    title = "Auto Retry",
    description = "Configure automatic retry behavior for recoverable errors."
)]
pub struct AutoRetryConfig {
    #[serde(default)]
    #[schemars(
        title = "Recoverable Error Patterns",
        description = "Regex patterns matched against error output to trigger auto-retry."
    )]
    pub error_patterns: Vec<String>,
    #[serde(default)]
    #[schemars(
        title = "Retry Delay (seconds)",
        description = "Seconds to wait before retrying after a matched error."
    )]
    pub delay_seconds: u32,
    #[serde(default)]
    #[schemars(
        title = "Max Retry Attempts",
        description = "Maximum number of auto-retry attempts for a failed run."
    )]
    pub max_attempts: u32,
}

impl Default for AutoRetryConfig {
    fn default() -> Self {
        Self {
            error_patterns: Vec::new(),
            delay_seconds: 15,
            max_attempts: 3,
        }
    }
}

impl AutoRetryConfig {
    pub fn is_enabled(&self) -> bool {
        !self.error_patterns.is_empty() && self.max_attempts > 0
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.is_enabled() {
            return Ok(());
        }

        if self.delay_seconds == 0 {
            return Err("auto_retry.delay_seconds must be >= 1 when enabled".to_string());
        }

        for pattern in &self.error_patterns {
            Regex::new(pattern)
                .map_err(|e| format!("Invalid auto_retry.error_patterns regex '{pattern}': {e}"))?;
        }

        Ok(())
    }

    pub fn matches_error(&self, text: &str) -> bool {
        if !self.is_enabled() || text.trim().is_empty() {
            return false;
        }

        self.error_patterns.iter().any(|pattern| {
            Regex::new(pattern)
                .map(|re| re.is_match(text))
                .unwrap_or(false)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::AutoRetryConfig;

    #[test]
    fn auto_retry_disabled_has_no_match() {
        let cfg = AutoRetryConfig::default();
        assert!(!cfg.matches_error("Error: something bad happened"));
    }

    #[test]
    fn auto_retry_matches_pattern() {
        let cfg = AutoRetryConfig {
            error_patterns: vec![r"InternalServerError".to_string()],
            delay_seconds: 10,
            max_attempts: 2,
        };
        assert!(cfg.matches_error("Error: InternalServerError"));
    }

    #[test]
    fn auto_retry_validate_rejects_bad_regex() {
        let cfg = AutoRetryConfig {
            error_patterns: vec!["(".to_string()],
            delay_seconds: 10,
            max_attempts: 1,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn auto_retry_validate_rejects_zero_delay() {
        let cfg = AutoRetryConfig {
            error_patterns: vec![r"Error".to_string()],
            delay_seconds: 0,
            max_attempts: 1,
        };
        assert!(cfg.validate().is_err());
    }
}
