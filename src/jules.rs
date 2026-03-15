//! Jules AI API client.
//!
//! Mirrors the flow from the original `lib/jules.js`:
//!   1. Create a session with a code-review prompt
//!   2. Poll for session completion (exponential backoff)
//!   3. List activities to get the agent's response
//!   4. Parse JSON response into structured review comments

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::config::JulesMode;

const JULES_BASE: &str = "https://jules.googleapis.com/v1alpha";
const POLL_INITIAL_MS: u64 = 3_000;
const POLL_MAX_MS: u64 = 15_000;

// ── Request / response types ────────────────────────────────────

#[derive(Debug, Serialize)]
struct CreateSessionRequest {
    prompt: String,
    mode: String,
    title: String,
}

#[derive(Debug, Deserialize)]
struct Session {
    name: String,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    outputs: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct ActivitiesResponse {
    #[serde(default)]
    activities: Vec<serde_json::Value>,
}

// ── Public types ────────────────────────────────────────────────

/// A single inline review comment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewComment {
    /// File path relative to the repo root.
    pub path: String,
    /// Line number in the new file.
    pub line: u64,
    /// Diff side (`RIGHT` for additions).
    pub side: String,
    /// Comment body with category/severity prefix.
    pub body: String,
}

/// The structured output of a Jules review.
#[derive(Debug, Clone)]
pub struct ReviewResult {
    /// A high-level summary of the review findings.
    pub summary: String,
    /// Detailed inline comments found during the review.
    pub comments: Vec<ReviewComment>,
}

// ── Client ──────────────────────────────────────────────────────

/// Jules API reviewer client.
pub struct JulesClient {
    api_key: String,
    mode: JulesMode,
    timeout_ms: u64,
    http: reqwest::Client,
}

impl JulesClient {
    /// Creates a new client.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Jules API authentication key
    /// * `mode` - Review mode (Speed or Balanced)
    /// * `timeout_minutes` - Maximum polling duration
    pub fn new(api_key: &str, mode: &JulesMode, timeout_minutes: u64) -> Self {
        Self {
            api_key: api_key.to_owned(),
            mode: mode.clone(),
            timeout_ms: timeout_minutes * 60 * 1000,
            http: reqwest::Client::new(),
        }
    }

    /// Sends code for review and returns structured comments.
    pub async fn analyze(&self, prompt: &str) -> Result<ReviewResult> {
        // 1. Create session
        let session = self.create_session(prompt).await?;
        let session_id = &session.name;
        info!(session_id, "Jules session created");

        // 2. Poll until done
        let completed = self.poll_session(session_id).await?;
        info!(session_id, state = ?completed.state, "Jules session completed");

        // 3. Fetch activities
        let activities = self.list_activities(session_id).await?;

        // 4. Parse
        self.parse_response(&activities, &completed)
    }

    // ── Private helpers ─────────────────────────────────────────

    async fn create_session(&self, prompt: &str) -> Result<Session> {
        let body = CreateSessionRequest {
            prompt: prompt.to_owned(),
            mode: self.mode.as_api_str().to_owned(),
            title: "Automated Code Review".to_owned(),
        };

        let res = self
            .http
            .post(format!("{JULES_BASE}/sessions"))
            .header("Content-Type", "application/json")
            .header("x-goog-api-key", &self.api_key)
            .json(&body)
            .send()
            .await
            .context("failed to send session request")?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            anyhow::bail!("Jules session creation failed: {status} {text}");
        }

        res.json::<Session>()
            .await
            .context("failed to parse session response")
    }

    async fn poll_session(&self, session_id: &str) -> Result<Session> {
        let start = std::time::Instant::now();
        let mut delay = POLL_INITIAL_MS;

        loop {
            if start.elapsed().as_millis() as u64 > self.timeout_ms {
                anyhow::bail!(
                    "Jules session timed out after {} minutes",
                    self.timeout_ms / 60_000
                );
            }

            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;

            let res = self
                .http
                .get(format!("{JULES_BASE}/{session_id}"))
                .header("x-goog-api-key", &self.api_key)
                .send()
                .await
                .context("failed to poll session")?;

            if !res.status().is_success() {
                let status = res.status();
                let text = res.text().await.unwrap_or_default();
                anyhow::bail!("Jules poll failed: {status} {text}");
            }

            let session: Session = res.json().await.context("failed to parse poll response")?;

            match session.state.as_deref() {
                Some("COMPLETED") => return Ok(session),
                Some("FAILED") | Some("CANCELLED") => {
                    anyhow::bail!(
                        "Jules session ended with state: {}",
                        session.state.as_deref().unwrap_or("UNKNOWN")
                    );
                }
                _ => {
                    // Still running — exponential backoff
                    delay = (delay * 3 / 2).min(POLL_MAX_MS);
                }
            }
        }
    }

    async fn list_activities(&self, session_id: &str) -> Result<ActivitiesResponse> {
        let res = self
            .http
            .get(format!("{JULES_BASE}/{session_id}/activities?pageSize=30"))
            .header("x-goog-api-key", &self.api_key)
            .send()
            .await
            .context("failed to list activities")?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            anyhow::bail!("Jules list activities failed: {status} {text}");
        }

        res.json::<ActivitiesResponse>()
            .await
            .context("failed to parse activities response")
    }

    fn parse_response(
        &self,
        activities: &ActivitiesResponse,
        completed_session: &Session,
    ) -> Result<ReviewResult> {
        let mut raw_text = String::new();

        // Try extracting agent message from activities
        for activity in &activities.activities {
            // Jules API uses "agentMessaged.agentMessage" for the text
            if let Some(msg) = activity
                .get("agentMessaged")
                .and_then(|am| am.get("agentMessage"))
                .and_then(|m| m.as_str())
            {
                raw_text = msg.to_string();
                break;
            }

            // Fallback: check other possible field names
            for key in &[
                "agentMessage",
                "message",
                "response",
                "output",
                "text",
                "content",
            ] {
                if let Some(text) = activity.get(*key).and_then(|v| v.as_str()) {
                    raw_text = text.to_string();
                    break;
                }
            }

            if !raw_text.is_empty() {
                break;
            }
        }

        // Fallback: check session outputs
        if raw_text.is_empty() {
            if let Some(outputs) = &completed_session.outputs {
                for output in outputs {
                    for key in &["text", "content", "message", "response"] {
                        if let Some(text) = output.get(*key).and_then(|v| v.as_str()) {
                            raw_text = text.to_string();
                            break;
                        }
                    }
                    if !raw_text.is_empty() {
                        break;
                    }
                }
            }
        }

        if raw_text.is_empty() {
            return Ok(ReviewResult {
                summary: "### Code Review\n\nNo response received from Jules.".to_string(),
                comments: vec![],
            });
        }

        // Strip markdown code fences
        let json_str = raw_text
            .replace("```json\n", "")
            .replace("```json", "")
            .replace("```\n", "")
            .replace("```", "");
        let json_str = json_str.trim();

        // Try to parse as JSON
        match serde_json::from_str::<serde_json::Value>(json_str) {
            Ok(parsed) => {
                let summary = parsed
                    .get("summary")
                    .and_then(|s| s.as_str())
                    .unwrap_or("No issues found. Code looks good.")
                    .to_string();

                let mut comments = Vec::new();
                if let Some(issues) = parsed.get("issues").and_then(|i| i.as_array()) {
                    comments.reserve_exact(issues.len());
                    for issue in issues {
                        let file = issue.get("file").and_then(|f| f.as_str()).unwrap_or("");
                        let line = issue.get("line").and_then(|l| l.as_u64()).unwrap_or(0);
                        let comment_text =
                            issue.get("comment").and_then(|c| c.as_str()).unwrap_or("");
                        let category = issue
                            .get("category")
                            .and_then(|c| c.as_str())
                            .unwrap_or("NOTE");
                        let severity = issue
                            .get("severity")
                            .and_then(|s| s.as_str())
                            .unwrap_or("note");

                        if !file.is_empty() && line > 0 && !comment_text.is_empty() {
                            comments.push(ReviewComment {
                                path: file.to_string(),
                                line,
                                side: "RIGHT".to_string(),
                                body: format!(
                                    "**[{category}]** ({severity}): {comment_text}"
                                ),
                            });
                        }
                    }
                }

                Ok(ReviewResult {
                    summary: format!("### Code Review\n\n{summary}"),
                    comments,
                })
            }
            Err(_) => {
                // Couldn't parse JSON — post raw text as review summary
                Ok(ReviewResult {
                    summary: format!("### Code Review\n\n{raw_text}"),
                    comments: vec![],
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a JulesClient for testing (no network calls made).
    fn test_client() -> JulesClient {
        JulesClient::new("test-key-for-unit-tests", &JulesMode::Speed, 25)
    }

    fn empty_session() -> Session {
        Session {
            name: "sessions/test-123".to_string(),
            state: Some("COMPLETED".to_string()),
            outputs: None,
        }
    }

    // ── parse_response: valid JSON (mirrors codeblocks.test.js logic) ──

    #[test]
    fn parse_valid_json_with_issues() {
        let client = test_client();
        let json_response = r#"{
            "summary": "Found 2 issues",
            "issues": [
                {
                    "file": "src/main.rs",
                    "line": 10,
                    "severity": "critical",
                    "category": "SECURITY",
                    "comment": "Hardcoded API key"
                },
                {
                    "file": "src/lib.rs",
                    "line": 25,
                    "severity": "warning",
                    "category": "LOGIC",
                    "comment": "Off-by-one error"
                }
            ]
        }"#;

        let activities = ActivitiesResponse {
            activities: vec![serde_json::json!({
                "agentMessaged": { "agentMessage": json_response }
            })],
        };

        let result = client.parse_response(&activities, &empty_session()).unwrap();
        assert!(result.summary.contains("Found 2 issues"));
        assert_eq!(result.comments.len(), 2);
        assert_eq!(result.comments[0].path, "src/main.rs");
        assert_eq!(result.comments[0].line, 10);
        assert!(result.comments[0].body.contains("SECURITY"));
        assert!(result.comments[0].body.contains("Hardcoded API key"));
        assert_eq!(result.comments[1].path, "src/lib.rs");
        assert_eq!(result.comments[1].line, 25);
    }

    #[test]
    fn parse_json_wrapped_in_code_fences() {
        // Mirrors: auto-closes unclosed fenced code block / generic fence handling
        let client = test_client();
        let fenced = "```json\n{\"summary\": \"All good\", \"issues\": []}\n```";

        let activities = ActivitiesResponse {
            activities: vec![serde_json::json!({
                "agentMessaged": { "agentMessage": fenced }
            })],
        };

        let result = client.parse_response(&activities, &empty_session()).unwrap();
        assert!(result.summary.contains("All good"));
        assert!(result.comments.is_empty());
    }

    #[test]
    fn parse_json_with_no_issues_array() {
        let client = test_client();
        let json_response = r#"{"summary": "No problems found"}"#;

        let activities = ActivitiesResponse {
            activities: vec![serde_json::json!({
                "agentMessaged": { "agentMessage": json_response }
            })],
        };

        let result = client.parse_response(&activities, &empty_session()).unwrap();
        assert!(result.summary.contains("No problems found"));
        assert!(result.comments.is_empty());
    }

    // ── parse_response: malformed / non-JSON fallback ───────────────

    #[test]
    fn parse_non_json_falls_back_to_raw_text() {
        // Mirrors: "does not modify already valid fenced code blocks" — when
        // the AI just returns prose instead of JSON, we gracefully fall back.
        let client = test_client();
        let raw_prose = "This code looks great! No issues found.\n\nKeep up the good work.";

        let activities = ActivitiesResponse {
            activities: vec![serde_json::json!({
                "agentMessaged": { "agentMessage": raw_prose }
            })],
        };

        let result = client.parse_response(&activities, &empty_session()).unwrap();
        assert!(result.summary.contains("This code looks great"));
        assert!(result.comments.is_empty());
    }

    // ── parse_response: empty / no activities ───────────────────────

    #[test]
    fn parse_empty_activities_returns_no_response() {
        // Mirrors: "handles null/undefined/non-string gracefully"
        let client = test_client();
        let activities = ActivitiesResponse {
            activities: vec![],
        };

        let result = client.parse_response(&activities, &empty_session()).unwrap();
        assert!(result.summary.contains("No response received"));
        assert!(result.comments.is_empty());
    }

    #[test]
    fn parse_activities_with_empty_message() {
        let client = test_client();
        let activities = ActivitiesResponse {
            activities: vec![serde_json::json!({
                "agentMessaged": { "agentMessage": "" }
            })],
        };

        // Empty string → fallback to session outputs → "No response"
        let result = client.parse_response(&activities, &empty_session()).unwrap();
        assert!(result.summary.contains("No response received"));
    }

    // ── parse_response: fallback to session outputs ─────────────────

    #[test]
    fn parse_falls_back_to_session_outputs() {
        let client = test_client();
        let activities = ActivitiesResponse {
            activities: vec![],
        };
        let session = Session {
            name: "sessions/fallback-1".to_string(),
            state: Some("COMPLETED".to_string()),
            outputs: Some(vec![serde_json::json!({
                "text": "{\"summary\": \"From outputs\", \"issues\": []}"
            })]),
        };

        let result = client.parse_response(&activities, &session).unwrap();
        assert!(result.summary.contains("From outputs"));
    }

    // ── parse_response: filters out invalid comments ────────────────

    #[test]
    fn parse_skips_comments_with_missing_fields() {
        let client = test_client();
        let json_response = r#"{
            "summary": "Review done",
            "issues": [
                { "file": "", "line": 10, "comment": "missing file" },
                { "file": "src/main.rs", "line": 0, "comment": "line is 0" },
                { "file": "src/main.rs", "line": 5, "comment": "" },
                { "file": "src/main.rs", "line": 42, "comment": "This is valid" }
            ]
        }"#;

        let activities = ActivitiesResponse {
            activities: vec![serde_json::json!({
                "agentMessaged": { "agentMessage": json_response }
            })],
        };

        let result = client.parse_response(&activities, &empty_session()).unwrap();
        assert_eq!(result.comments.len(), 1, "only the valid comment should survive");
        assert_eq!(result.comments[0].line, 42);
    }

    // ── parse_response: alternative activity field names ─────────────

    #[test]
    fn parse_extracts_from_fallback_field_names() {
        let client = test_client();
        let activities = ActivitiesResponse {
            activities: vec![serde_json::json!({
                "message": "{\"summary\": \"Via message field\", \"issues\": []}"
            })],
        };

        let result = client.parse_response(&activities, &empty_session()).unwrap();
        assert!(result.summary.contains("Via message field"));
    }

    // ── ReviewComment equality ──────────────────────────────────────

    #[test]
    fn review_comment_equality() {
        let a = ReviewComment {
            path: "src/main.rs".to_string(),
            line: 10,
            side: "RIGHT".to_string(),
            body: "fix this".to_string(),
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    // ── ReviewComment serialization roundtrip ────────────────────────

    #[test]
    fn review_comment_serde_roundtrip() {
        let comment = ReviewComment {
            path: "src/lib.rs".to_string(),
            line: 42,
            side: "RIGHT".to_string(),
            body: "**[SECURITY]** (critical): SQL injection".to_string(),
        };

        let json = serde_json::to_string(&comment).unwrap();
        let deserialized: ReviewComment = serde_json::from_str(&json).unwrap();

        assert_eq!(comment, deserialized);
    }
}
