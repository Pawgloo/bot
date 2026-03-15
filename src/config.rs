//! Configuration module for the bot.
//!
//! Reads environment variables for Jules API, file filtering, and other bot settings.

use anyhow::{Context, Result};
use std::env;

/// Jules mode for API requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JulesMode {
    /// Fast mode — prioritizes speed.
    Speed,
    /// Balanced mode — balances speed and quality.
    Balanced,
}

impl JulesMode {
    /// Returns the string representation for the Jules API.
    #[inline]
    pub fn as_api_str(&self) -> &'static str {
        match self {
            Self::Speed => "SPEED",
            Self::Balanced => "BALANCED",
        }
    }
}

impl Default for JulesMode {
    fn default() -> Self {
        Self::Speed
    }
}

impl std::fmt::Display for JulesMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_api_str())
    }
}

/// Bot-specific configuration loaded from environment variables.
///
/// All fields have sensible defaults except `jules_api_key` which must be provided.
#[derive(Debug, Clone)]
pub struct BotConfig {
    /// Jules API key for authentication.
    pub jules_api_key: String,
    /// Jules mode (Speed or Balanced).
    pub jules_mode: JulesMode,
    /// Session timeout in minutes for Jules polling.
    pub session_timeout_minutes: u64,
    /// Maximum patch length (in characters) to send for review.
    pub max_patch_length: usize,
    /// Glob-like patterns for files to ignore during review.
    pub ignore_patterns: Vec<String>,
    /// Whether to skip draft PRs.
    pub skip_draft_prs: bool,
}

impl BotConfig {
    /// Loads configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if `JULES_API_KEY` is not set, or if numeric env vars contain invalid values.
    pub fn from_env() -> Result<Self> {
        let jules_api_key = env::var("JULES_API_KEY").context("JULES_API_KEY must be set")?;

        let jules_mode = match env::var("JULES_MODE").as_deref() {
            Ok("BALANCED") => JulesMode::Balanced,
            _ => JulesMode::Speed,
        };

        let session_timeout_minutes = env::var("SESSION_TIMEOUT_MINUTES")
            .unwrap_or_else(|_| "25".into())
            .parse::<u64>()
            .context("SESSION_TIMEOUT_MINUTES must be a valid integer")?;

        let max_patch_length = env::var("MAX_PATCH_LENGTH")
            .unwrap_or_else(|_| "100000".into())
            .parse::<usize>()
            .context("MAX_PATCH_LENGTH must be a valid integer")?;

        let ignore_patterns_str = env::var("IGNORE_PATTERNS").unwrap_or_else(|_| {
            "*.txt,*.lock,*.png,*.jpg,*.svg,*.ico,dist/,node_modules/".into()
        });

        let ignore_patterns: Vec<String> = ignore_patterns_str
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();

        let skip_draft_prs = env::var("SKIP_DRAFT_PRS")
            .map(|v| v != "false")
            .unwrap_or(true);

        Ok(Self {
            jules_api_key,
            jules_mode,
            session_timeout_minutes,
            max_patch_length,
            ignore_patterns,
            skip_draft_prs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jules_mode_default_is_speed() {
        assert_eq!(JulesMode::default(), JulesMode::Speed);
    }

    #[test]
    fn jules_mode_api_str() {
        assert_eq!(JulesMode::Speed.as_api_str(), "SPEED");
        assert_eq!(JulesMode::Balanced.as_api_str(), "BALANCED");
    }
}
