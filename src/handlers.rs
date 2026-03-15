//! Event handlers for the Pawgloo bot.
//!
//! - `pull_request_handler` — triggers on PR opened / synchronize
//! - `issue_comment_handler` — triggers on `/pawgloo-review` or `/pawgloo`

use std::sync::Arc;

use tracing::{error, info};

use crate::review;
use crate::AppState;

/// Handles pull_request events (opened, synchronize).
pub async fn pull_request_handler(
    context: octofer::Context,
    state: Arc<AppState>,
) -> anyhow::Result<()> {
    let payload = context.payload();

    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Only react to "opened" and "synchronize"
    if action != "opened" && action != "synchronize" {
        return Ok(());
    }

    let pr = match payload.get("pull_request") {
        Some(pr) => pr,
        None => return Ok(()),
    };

    let pr_number = pr.get("number").and_then(|n| n.as_u64()).unwrap_or(0);
    let pr_state = pr
        .get("state")
        .and_then(|s| s.as_str())
        .unwrap_or("open");
    let is_draft = pr.get("draft").and_then(|d| d.as_bool()).unwrap_or(false);
    let author = pr
        .get("user")
        .and_then(|u| u.get("login"))
        .and_then(|l| l.as_str())
        .unwrap_or("unknown");

    info!(
        pr_number,
        action, author, "🔔 Auto-trigger fired for PR"
    );

    // Skip closed or locked PRs
    if pr_state == "closed" {
        info!(pr_number, "Skipping closed PR");
        return Ok(());
    }

    // Skip drafts if configured
    if is_draft && state.bot_config.skip_draft_prs {
        info!(pr_number, "Skipping draft PR");
        return Ok(());
    }

    if let Err(e) = review::analyze_and_review(&context, &state, pr).await {
        error!(pr_number, error = %e, "Review failed");
    }

    Ok(())
}

/// Handles issue_comment events for manual triggers.
pub async fn issue_comment_handler(
    context: octofer::Context,
    state: Arc<AppState>,
) -> anyhow::Result<()> {
    let payload = context.payload();

    // Only react to "created" comments
    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if action != "created" {
        return Ok(());
    }

    // Must be on a PR (has pull_request link)
    let issue = match payload.get("issue") {
        Some(i) => i,
        None => return Ok(()),
    };
    if issue.get("pull_request").is_none() {
        return Ok(());
    }

    let comment = match payload.get("comment") {
        Some(c) => c,
        None => return Ok(()),
    };
    let body = comment
        .get("body")
        .and_then(|b| b.as_str())
        .unwrap_or("");
    let body_lower = body.to_lowercase();

    // Check for trigger commands anywhere in the comment
    if !body_lower.contains("/pawgloo") {
        return Ok(());
    }

    let pr_number = issue.get("number").and_then(|n| n.as_u64()).unwrap_or(0);
    let commenter = comment
        .get("user")
        .and_then(|u| u.get("login"))
        .and_then(|l| l.as_str())
        .unwrap_or("unknown");

    info!(
        pr_number,
        commenter, "Manual trigger received"
    );

    // React with 🚀 to acknowledge
    if let Some(octo) = context.installation_client().await? {
        let repo = payload.get("repository");
        let owner = repo
            .and_then(|r| r.get("owner"))
            .and_then(|o| o.get("login"))
            .and_then(|l| l.as_str())
            .unwrap_or("");
        let repo_name = repo
            .and_then(|r| r.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("");
        let comment_id = comment.get("id").and_then(|id| id.as_u64()).unwrap_or(0);

        // Add rocket reaction via REST API
        let reaction_url = format!(
            "https://api.github.com/repos/{owner}/{repo_name}/issues/comments/{comment_id}/reactions"
        );
        let _ = reqwest::Client::new()
            .post(&reaction_url)
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "pawgloo-bot")
            .json(&serde_json::json!({ "content": "rocket" }))
            .send()
            .await;

        // Fetch full PR object
        let pr_data = octo.pulls(owner, repo_name).get(pr_number).await?;
        let pr_value = serde_json::to_value(&pr_data)?;

        if let Err(e) = review::analyze_and_review(&context, &state, &pr_value).await {
            error!(pr_number, error = %e, "Manual review failed");

            // Post error comment
            let error_body = format!(
                "### Code Review\n\n❌ **Error during review**: {}\n\nPlease check the bot logs or retry with `/pawgloo-review`.",
                e
            );
            let _ = octo
                .issues(owner, repo_name)
                .create_comment(pr_number, error_body)
                .await;
        }
    }

    Ok(())
}
