//! Pawgloo Bot — AI-powered PR reviewer built on octofer.
//!
//! Triggers:
//!   - `pull_request.opened` / `pull_request.synchronize` → automatic review
//!   - `issue_comment.created` `/pawgloo-review` or `/pawgloo` → manual re-review

// lint-deny-correctness, lint-warn-suspicious, lint-warn-style,
// lint-warn-complexity, lint-warn-perf, lint-missing-docs (AGENTS.md)
#![deny(clippy::correctness)]
#![warn(clippy::suspicious)]
#![warn(clippy::style)]
#![warn(clippy::complexity)]
#![warn(clippy::perf)]
#![warn(missing_docs)]

use std::sync::Arc;
use tracing::info;

pub mod config;
pub mod handlers;
pub mod jules;
pub mod review;

pub use config::BotConfig;
use octofer::Octofer;

/// Shared application state passed to every event handler.
#[derive(Debug)]
pub struct AppState {
    /// Bot-specific configuration.
    pub bot_config: BotConfig,
}



pub async fn setup_app(bot_config: BotConfig, octofer_config: octofer::Config) -> Result<Octofer, anyhow::Error> {
    let state = Arc::new(AppState { bot_config });
    let mut app = Octofer::new(octofer_config).await?;

    // ── Automatic trigger: PR opened / synchronize ───────────────
    app.on_pull_request(handlers::pull_request_handler, state.clone())
        .await;

    // ── Manual trigger: /pawgloo-review or /pawgloo comment ──────
    app.on_issue_comment(handlers::issue_comment_handler, state.clone())
        .await;

    Ok(app)
}

/// Starts the Pawgloo Bot webhook listener.
pub async fn start() -> Result<(), anyhow::Error> {
    // Initialize tracing via octofer
    let octofer_config =
        octofer::Config::from_env().unwrap_or_else(|_| octofer::Config::default());
    octofer_config.init_logging();

    info!("🤖 Pawgloo Bot starting...");

    // Load bot-specific configuration
    let bot_config = BotConfig::from_env()?;
    info!(
        jules_mode = %bot_config.jules_mode,
        ignore_patterns = ?bot_config.ignore_patterns,
        "Configuration loaded"
    );
    let state = Arc::new(AppState { bot_config: bot_config.clone() });

    // Create the Octofer app
    let app = setup_app(bot_config, octofer_config).await?;

    info!("Registered events: pull_request, issue_comment");

    // Start the webhook server
    app.start().await?;

    Ok(())
}
