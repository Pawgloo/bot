//! Config tests — ported from `index.test.js`

use std::env;
use bot::config::{BotConfig, JulesMode};
use serial_test::serial;

#[test]
fn jules_mode_default_is_speed() {
    assert_eq!(JulesMode::default(), JulesMode::Speed);
}

#[test]
fn jules_mode_api_str() {
    assert_eq!(JulesMode::Speed.as_api_str(), "SPEED");
    assert_eq!(JulesMode::Balanced.as_api_str(), "BALANCED");
}

#[test]
fn jules_mode_display() {
    assert_eq!(format!("{}", JulesMode::Speed), "SPEED");
    assert_eq!(format!("{}", JulesMode::Balanced), "BALANCED");
}

#[test]
fn jules_mode_equality() {
    assert_eq!(JulesMode::Speed, JulesMode::Speed);
    assert_ne!(JulesMode::Speed, JulesMode::Balanced);
}

#[test]
fn jules_mode_clone() {
    let mode = JulesMode::Balanced;
    let cloned = mode.clone();
    assert_eq!(mode, cloned);
}

#[test]
#[serial]
fn bot_config_from_env_missing_api_key() {
    unsafe { env::remove_var("JULES_API_KEY") };
    let result = BotConfig::from_env();
    assert!(result.is_err());
    assert!(format!("{}", result.unwrap_err()).contains("JULES_API_KEY"));
}

#[test]
#[serial]
fn bot_config_defaults_with_api_key() {
    unsafe {
        env::set_var("JULES_API_KEY", "test-key-123");
        env::remove_var("JULES_MODE");
        env::remove_var("SESSION_TIMEOUT_MINUTES");
        env::remove_var("MAX_PATCH_LENGTH");
        env::remove_var("IGNORE_PATTERNS");
        env::remove_var("SKIP_DRAFT_PRS");
    }
    let config = BotConfig::from_env().expect("should succeed");
    assert_eq!(config.jules_api_key, "test-key-123");
    assert_eq!(config.jules_mode, JulesMode::Speed);
    assert_eq!(config.session_timeout_minutes, 25);
    assert_eq!(config.max_patch_length, 100_000);
    assert!(config.skip_draft_prs);
    assert!(!config.ignore_patterns.is_empty());
    unsafe { env::remove_var("JULES_API_KEY") };
}

#[test]
#[serial]
fn bot_config_balanced_mode() {
    unsafe {
        env::set_var("JULES_API_KEY", "test-key-balanced");
        env::set_var("JULES_MODE", "BALANCED");
    }
    let config = BotConfig::from_env().expect("should load");
    assert_eq!(config.jules_mode, JulesMode::Balanced);
    unsafe {
        env::remove_var("JULES_API_KEY");
        env::remove_var("JULES_MODE");
    }
}

#[test]
#[serial]
fn bot_config_custom_ignore_patterns() {
    unsafe {
        env::set_var("JULES_API_KEY", "test-key-patterns");
        env::set_var("IGNORE_PATTERNS", "*.rs,src/generated/");
    }
    let config = BotConfig::from_env().expect("should load");
    assert_eq!(config.ignore_patterns, vec!["*.rs", "src/generated/"]);
    unsafe {
        env::remove_var("JULES_API_KEY");
        env::remove_var("IGNORE_PATTERNS");
    }
}

#[test]
#[serial]
fn bot_config_skip_drafts_false() {
    unsafe {
        env::set_var("JULES_API_KEY", "test-key-drafts");
        env::set_var("SKIP_DRAFT_PRS", "false");
    }
    let config = BotConfig::from_env().expect("should load");
    assert!(!config.skip_draft_prs);
    unsafe {
        env::remove_var("JULES_API_KEY");
        env::remove_var("SKIP_DRAFT_PRS");
    }
}
