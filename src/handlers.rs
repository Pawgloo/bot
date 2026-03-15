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
    let event = match &context.event {
        Some(e) => e,
        None => return Ok(()),
    };

    let pr_payload = match &event.specific {
        octofer::octocrab::models::webhook_events::WebhookEventPayload::PullRequest(p) => p,
        _ => return Ok(()),
    };

    let action = match pr_payload.action {
        octofer::octocrab::models::webhook_events::payload::PullRequestWebhookEventAction::Opened => "opened",
        octofer::octocrab::models::webhook_events::payload::PullRequestWebhookEventAction::Synchronize => "synchronize",
        _ => return Ok(()),
    };

    let pr = &pr_payload.pull_request;
    let pr_number = pr.number;
    let is_draft = pr.draft.unwrap_or(false);
    
    // octocrab's PullRequest state is an enum (Open, Closed)
    let pr_state = match pr.state {
        Some(octofer::octocrab::models::IssueState::Closed) => "closed",
        _ => "open",
    };

    let author = pr.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown");

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

    // Convert PullRequest to serde_json::Value for analyze_and_review
    let pr_value = serde_json::to_value(pr)?;

    if let Err(e) = review::analyze_and_review(&context, &state, &pr_value).await {
        error!(pr_number, error = %e, "Review failed");
    }

    Ok(())
}

/// Handles issue_comment events for manual triggers.
pub async fn issue_comment_handler(
    context: octofer::Context,
    state: Arc<AppState>,
) -> anyhow::Result<()> {
    let event = match &context.event {
        Some(e) => e,
        None => return Ok(()),
    };

    let comment_payload = match &event.specific {
        octofer::octocrab::models::webhook_events::WebhookEventPayload::IssueComment(c) => c,
        _ => return Ok(()),
    };

    let _action = match comment_payload.action {
        octofer::octocrab::models::webhook_events::payload::IssueCommentWebhookEventAction::Created => "created",
        _ => return Ok(()),
    };

    let issue = &comment_payload.issue;
    
    // Must be on a PR (has pull_request link)
    if issue.pull_request.is_none() {
        return Ok(());
    }

    let comment = &comment_payload.comment;
    let body_lower = comment.body.as_deref().unwrap_or("").to_lowercase();

    // Check for trigger commands anywhere in the comment
    if !body_lower.contains("/pawgloo") {
        return Ok(());
    }

    let pr_number = issue.number;
    let commenter = comment.user.login.as_str();

    info!(
        pr_number,
        commenter, "Manual trigger received"
    );

    // React with 🚀 to acknowledge
    if let Some(octo) = context.installation_client().await? {
        let repo = match &event.repository {
            Some(r) => r,
            None => {
                error!("Webhook event missing repository info");
                return Ok(());
            }
        };
        
        let owner = repo.owner.as_ref().map(|u| u.login.as_str()).unwrap_or("");
        let repo_name = repo.name.as_str();
        let comment_id = comment.id.0;

        // Add rocket reaction via authenticated octocrab client
        let reaction_path = format!(
            "/repos/{owner}/{repo_name}/issues/comments/{comment_id}/reactions"
        );
        match octo
            .post::<_, serde_json::Value>(
                reaction_path,
                Some(&serde_json::json!({ "content": "rocket" })),
            )
            .await
        {
            Ok(_) => info!(pr_number, "🚀 Reaction added"),
            Err(e) => error!(pr_number, error = %e, "Failed to add reaction"),
        }

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
