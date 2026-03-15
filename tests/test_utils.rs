//! File filtering and prompt building tests — ported from `utils.test.js`

use bot::review::{build_prompt, should_ignore, ChangedFile};

// ── should_ignore tests ─────────────────────────────────────────

#[test]
fn ignore_by_extension() {
    let patterns = vec![
        "*.md".to_string(),
        "*.json".to_string(),
        "*.test.js".to_string(),
    ];
    assert!(should_ignore("README.md", &patterns));
    assert!(should_ignore("package-lock.json", &patterns));
    assert!(!should_ignore("src/app.js", &patterns));
}

#[test]
fn ignore_by_directory() {
    let patterns = vec!["dist/".to_string(), "node_modules/".to_string()];
    assert!(should_ignore("dist/bundle.js", &patterns));
    assert!(should_ignore("node_modules/package.json", &patterns));
    assert!(!should_ignore("src/main.rs", &patterns));
}

#[test]
fn ignore_no_false_positives_on_substring() {
    let patterns = vec!["node_modules/".to_string()];
    assert!(!should_ignore("src/node_modules_utils.rs", &patterns));
}

#[test]
fn ignore_exact_filename_match() {
    let patterns = vec!["Makefile".to_string()];
    assert!(should_ignore("Makefile", &patterns));
    assert!(!should_ignore("Makefile.bak", &patterns));
}

#[test]
fn ignore_multiple_extensions() {
    let patterns = vec![
        "*.png".to_string(),
        "*.jpg".to_string(),
        "*.svg".to_string(),
        "*.ico".to_string(),
        "*.lock".to_string(),
        "*.txt".to_string(),
    ];
    assert!(should_ignore("image.png", &patterns));
    assert!(should_ignore("photo.jpg", &patterns));
    assert!(should_ignore("logo.svg", &patterns));
    assert!(should_ignore("favicon.ico", &patterns));
    assert!(should_ignore("Cargo.lock", &patterns));
    assert!(should_ignore("notes.txt", &patterns));
    assert!(!should_ignore("src/lib.rs", &patterns));
    assert!(!should_ignore("index.html", &patterns));
}

#[test]
fn ignore_empty_patterns() {
    let patterns: Vec<String> = vec![];
    assert!(!should_ignore("anything.rs", &patterns));
}

// ── build_prompt tests ──────────────────────────────────────────

#[test]
fn build_prompt_includes_file_list() {
    let files = vec![
        ChangedFile {
            filename: "src/main.rs".to_string(),
            patch: "+fn main() {}".to_string(),
        },
        ChangedFile {
            filename: "src/lib.rs".to_string(),
            patch: "+pub mod config;".to_string(),
        },
    ];
    let pr_meta = serde_json::json!({
        "title": "Add feature X",
        "body": "This PR adds feature X",
        "user": { "login": "testuser" },
        "base": { "ref": "main" },
        "head": { "ref": "feature/x" }
    });

    let prompt = build_prompt(&files, &pr_meta);

    assert!(prompt.contains("src/main.rs"));
    assert!(prompt.contains("src/lib.rs"));
    assert!(prompt.contains("Add feature X"));
    assert!(prompt.contains("testuser"));
    assert!(prompt.contains("feature/x"));
}

#[test]
fn build_prompt_handles_missing_pr_fields() {
    let files = vec![ChangedFile {
        filename: "test.rs".to_string(),
        patch: "+code".to_string(),
    }];
    let pr_meta = serde_json::json!({});
    let prompt = build_prompt(&files, &pr_meta);
    assert!(prompt.contains("unknown"));
    assert!(prompt.contains("test.rs"));
}

#[test]
fn build_prompt_includes_diff_content() {
    let files = vec![ChangedFile {
        filename: "app.rs".to_string(),
        patch: "@@ -1,2 +1,2 @@\n-old_line\n+new_line".to_string(),
    }];
    let pr_meta = serde_json::json!({"title": "Fix bug"});
    let prompt = build_prompt(&files, &pr_meta);
    assert!(prompt.contains("+new_line"));
}

#[test]
fn build_prompt_empty_files() {
    let files: Vec<ChangedFile> = vec![];
    let pr_meta = serde_json::json!({"title": "Empty PR"});
    let prompt = build_prompt(&files, &pr_meta);
    assert!(prompt.contains("Review"));
}
