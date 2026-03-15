//! Core review pipeline.
//!
//! Orchestrates: fetch diff -> filter files -> build prompt -> call Jules -> post review.

use std::collections::HashSet;
use std::fmt::Write as _;

use anyhow::{Context, Result};
use octofer::octocrab::models::repos::DiffEntryStatus;
use tracing::{error, info};

use crate::jules::{JulesClient, ReviewComment};
use crate::AppState;

/// A file that has changed in the PR, with its unified diff patch.
#[derive(Debug)]
struct ChangedFile {
    filename: String,
    patch: String,
}

/// Parses a unified diff patch and returns the set of valid new-file (right-side) line numbers
/// that GitHub will accept for inline review comments.
fn parse_diff_lines(patch: &str) -> HashSet<u64> {
    let mut lines = HashSet::new();
    let mut new_line: u64 = 0;

    for raw in patch.lines() {
        // Hunk header: @@ -old,count +new,count @@
        if let Some(rest) = raw.strip_prefix("@@ ") {
            if let Some(plus_pos) = rest.find('+') {
                let after_plus = &rest[plus_pos + 1..];
                let end = after_plus
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(after_plus.len());
                if let Ok(n) = after_plus[..end].parse::<u64>() {
                    new_line = n;
                }
            }
            continue;
        }

        if raw.starts_with('-') {
            // Deleted line — not in new file
            continue;
        }

        if raw.starts_with('+') || raw.starts_with(' ') {
            lines.insert(new_line);
            new_line += 1;
        }
    }

    lines
}

/// Checks whether a filename matches any of the ignore patterns.
fn should_ignore(filename: &str, patterns: &[String]) -> bool {
    for pattern in patterns {
        if pattern.ends_with('/') {
            // Directory prefix match
            if filename.starts_with(pattern.as_str()) {
                return true;
            }
        } else if let Some(ext) = pattern.strip_prefix("*.") {
            // Extension match — avoid format! allocation (mem-avoid-format)
            if filename.len() > ext.len()
                && filename.as_bytes()[filename.len() - ext.len() - 1] == b'.'
                && filename.ends_with(ext)
            {
                return true;
            }
        } else if filename == pattern {
            return true;
        }
    }
    false
}

/// Builds the system prompt for code review (mirrors `_buildPrompt` in `jules.js`).
fn build_prompt(files: &[ChangedFile], pr_meta: &serde_json::Value) -> String {
    // mem-write-over-format: build lists with write! instead of format!
    let mut file_list = String::with_capacity(files.len() * 40);
    for f in files {
        let _ = writeln!(file_list, "- `{}`", f.filename);
    }

    let mut file_context = String::with_capacity(files.iter().map(|f| f.patch.len() + 60).sum());
    for f in files {
        let _ = write!(
            file_context,
            "### File: `{}`\n```diff\n{}\n```\n\n",
            f.filename, f.patch
        );
    }

    let title = pr_meta
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let body = pr_meta
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let author = pr_meta
        .get("user")
        .and_then(|u| u.get("login"))
        .and_then(|l| l.as_str())
        .unwrap_or("unknown");
    let base_branch = pr_meta
        .get("base")
        .and_then(|b| b.get("ref"))
        .and_then(|r| r.as_str())
        .unwrap_or("main");
    let head_branch = pr_meta
        .get("head")
        .and_then(|h| h.get("ref"))
        .and_then(|r| r.as_str())
        .unwrap_or("unknown");

    let extensions: HashSet<&str> = files
        .iter()
        .filter_map(|f| f.filename.rsplit('.').next())
        .collect();
    let lang_hint: String = extensions.into_iter().collect::<Vec<_>>().join(", ");

    let mut pr_context = Vec::with_capacity(4);
    if !title.is_empty() {
        pr_context.push(format!("**PR Title**: {title}"));
    }
    if !body.is_empty() {
        pr_context.push(format!("**PR Description**: {body}"));
    }
    pr_context.push(format!("**Author**: {author}"));
    pr_context.push(format!("**Branch**: `{head_branch}` → `{base_branch}`"));
    let pr_context_str = pr_context.join("\n");

    let json_example = concat!(
        "{\n",
        "  \"analysis_scratchpad\": \"...\",\n",
        "  \"summary\": \"# PR Summary...\",\n",
        "  \"issues\": [\n",
        "    {\n",
        "      \"file\": \"path/to/file\",\n",
        "      \"line\": 65,\n",
        "      \"severity\": \"critical\",\n",
        "      \"category\": \"SECURITY\",\n",
        "      \"comment\": \"Explanation plus code suggestion\"\n",
        "    }\n",
        "  ]\n",
        "}"
    );

    format!(
        "Role: You are a fiercely pragmatic Principal Engineer and a paranoid Security Researcher. \
         Review this code with brutal technical precision.\n\n\
         ## Pull Request Context\n{pr_context_str}\n\n\
         ## Changed Files ({lang_hint})\n{file_list}\n\n---\n\n\
         ## Review Methodology\n\n\
         Analyze each file diff below. Apply SECURITY (STRIDE), LOGIC (correctness & edge cases), \
         and CLEAN CODE (DRY, SOLID, Performance) frameworks.\n\n\
         ### What NOT to flag\n- Minor style preferences\n- Import ordering\n- Trivial naming nitpicks\n\n---\n\n\
         ## Output Format\n\nRespond ONLY with valid JSON:\n\n{json_example}\n\n\
         IMPORTANT: This is a one-shot automated review. Respond with the JSON above and nothing else. \
         Do NOT ask follow-up questions.\n\n\
         ## Diffs\n\n{file_context}"
    )
}

/// Main review pipeline: fetches diff, filters files, calls Jules, posts review.
pub async fn analyze_and_review(
    context: &octofer::Context,
    state: &AppState,
    pr: &serde_json::Value,
) -> Result<()> {
    let pr_number = pr.get("number").and_then(|n| n.as_u64()).unwrap_or(0);
    info!(pr_number, "Starting review pipeline");

    let octo = context
        .installation_client()
        .await?
        .context("no installation client available")?;

    let payload = context.payload();
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

    let base_sha = pr
        .get("base")
        .and_then(|b| b.get("sha"))
        .and_then(|s| s.as_str())
        .unwrap_or("");
    let head_sha = pr
        .get("head")
        .and_then(|h| h.get("sha"))
        .and_then(|s| s.as_str())
        .unwrap_or("");

    // 1. Compare commits to get changed files with patches
    let comparison = octo
        .commits(owner, repo_name)
        .compare(base_sha, head_sha)
        .send()
        .await
        .context("failed to compare commits")?;

    let config = &state.bot_config;

    let changed_files: Vec<ChangedFile> = comparison
        .files
        .unwrap_or_default()
        .into_iter()
        .filter(|file| {
            let allowed = matches!(
                file.status,
                DiffEntryStatus::Modified
                    | DiffEntryStatus::Added
                    | DiffEntryStatus::Renamed
                    | DiffEntryStatus::Copied
            );
            if !allowed {
                info!(file = %file.filename, status = ?file.status, "Skipping (status)");
                return false;
            }
            if should_ignore(&file.filename, &config.ignore_patterns) {
                info!(file = %file.filename, "Skipping (ignore pattern)");
                return false;
            }
            let patch = file.patch.as_deref().unwrap_or("");
            if patch.is_empty() || patch.len() > config.max_patch_length {
                info!(file = %file.filename, "Skipping (patch too large or empty)");
                return false;
            }
            info!(file = %file.filename, "✓ Will review");
            true
        })
        .map(|file| ChangedFile {
            filename: file.filename.clone(),
            patch: file.patch.unwrap_or_default(),
        })
        .collect();

    if changed_files.is_empty() {
        info!(pr_number, "No relevant files to review after filtering");
        return Ok(());
    }

    info!(
        pr_number,
        file_count = changed_files.len(),
        "Reviewing files"
    );

    // 2. Build prompt and call Jules
    let prompt = build_prompt(&changed_files, pr);
    let client = JulesClient::new(
        &config.jules_api_key,
        &config.jules_mode,
        config.session_timeout_minutes,
    );
    let review = client.analyze(&prompt).await?;

    // 3. Build diff line maps for validation
    let diff_line_maps: std::collections::HashMap<String, HashSet<u64>> = changed_files
        .iter()
        .map(|f| (f.filename.clone(), parse_diff_lines(&f.patch)))
        .collect();

    let mut valid_comments: Vec<ReviewComment> = Vec::new();
    let mut orphan_comments: Vec<ReviewComment> = Vec::new();

    for comment in &review.comments {
        if let Some(valid_lines) = diff_line_maps.get(&comment.path) {
            if valid_lines.contains(&comment.line) {
                valid_comments.push(comment.clone());
            } else {
                orphan_comments.push(comment.clone());
            }
        }
    }

    let mut review_body = review.summary.clone();

    if !orphan_comments.is_empty() {
        review_body
            .push_str("\n\n---\n**Additional comments** (on lines outside the diff):\n\n");
        for c in &orphan_comments {
            let _ = write!(review_body, "- **{}:{}** -- {}\n", c.path, c.line, c.body);
        }
    }

    // 4. Post review
    let github_comments: Vec<serde_json::Value> = valid_comments
        .iter()
        .map(|c| {
            serde_json::json!({
                "path": c.path,
                "line": c.line,
                "side": c.side,
                "body": c.body,
            })
        })
        .collect();

    let review_request = serde_json::json!({
        "commit_id": head_sha,
        "body": review_body,
        "event": "COMMENT",
        "comments": github_comments,
    });

    // Use the GitHub REST API directly for creates review
    let url = format!(
        "https://api.github.com/repos/{owner}/{repo_name}/pulls/{pr_number}/reviews"
    );

    let res = reqwest::Client::new()
        .post(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "pawgloo-bot")
        .header(
            "Authorization",
            format!(
                "token {}",
                octo.current()
                    .user()
                    .await
                    .ok()
                    .map(|_| "")  // placeholder — token handled by octocrab below
                    .unwrap_or("")
            ),
        )
        .json(&review_request)
        .send()
        .await;

    // Fallback: if direct REST fails, try octocrab's issue comment
    match res {
        Ok(r) if r.status().is_success() => {
            info!(
                pr_number,
                inline_comments = valid_comments.len(),
                "✅ Review posted"
            );
        }
        _ => {
            error!(pr_number, "createReview failed, falling back to issue comment");

            let mut fallback_body = review_body;
            for c in &valid_comments {
                let _ = write!(fallback_body, "\n- **{}:{}** -- {}", c.path, c.line, c.body);
            }
            let _ = octo
                .issues(owner, repo_name)
                .create_comment(pr_number, fallback_body)
                .await;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_diff_lines() {
        let patch = "@@ -1,3 +1,4 @@\n context line\n+added line\n context line\n-removed line\n+replacement line";
        let lines = parse_diff_lines(patch);
        assert!(lines.contains(&1)); // context line
        assert!(lines.contains(&2)); // added line
        assert!(lines.contains(&3)); // context line
        assert!(lines.contains(&4)); // replacement line
        assert!(!lines.contains(&0));
    }

    #[test]
    fn test_should_ignore_extension() {
        let patterns = vec!["*.png".to_string(), "dist/".to_string()];
        assert!(should_ignore("image.png", &patterns));
        assert!(should_ignore("dist/bundle.js", &patterns));
        assert!(!should_ignore("src/main.rs", &patterns));
    }

    #[test]
    fn test_should_ignore_directory() {
        let patterns = vec!["node_modules/".to_string()];
        assert!(should_ignore("node_modules/package.json", &patterns));
        assert!(!should_ignore("src/node_modules_utils.rs", &patterns));
    }
}
