//! Diff parsing tests — ported from `difflines.test.js`

use bot::review::parse_diff_lines;

#[test]
fn parse_diff_simple_patch() {
    let patch = "@@ -1,3 +1,4 @@\n line1\n-line2\n+line2_modified\n+line3_added\n line4";
    let lines = parse_diff_lines(patch);
    assert!(lines.contains(&1));
    assert!(lines.contains(&2));
    assert!(lines.contains(&3));
    assert!(lines.contains(&4));
    assert_eq!(lines.len(), 4);
}

#[test]
fn parse_diff_multiple_hunks() {
    let patch = "@@ -1,2 +1,2 @@\n line1\n line2\n@@ -10,2 +10,2 @@\n line10\n line11";
    let lines = parse_diff_lines(patch);
    assert!(lines.contains(&1));
    assert!(lines.contains(&2));
    assert!(lines.contains(&10));
    assert!(lines.contains(&11));
    assert_eq!(lines.len(), 4);
}

#[test]
fn parse_diff_empty_patch() {
    let lines = parse_diff_lines("");
    assert!(lines.is_empty());
}

#[test]
fn parse_diff_only_deletions() {
    let patch = "@@ -1,3 +1,1 @@\n-deleted1\n-deleted2\n context";
    let lines = parse_diff_lines(patch);
    assert!(lines.contains(&1));
    assert!(!lines.contains(&2));
    assert_eq!(lines.len(), 1);
}

#[test]
fn parse_diff_only_additions() {
    let patch = "@@ -1,0 +1,3 @@\n+new1\n+new2\n+new3";
    let lines = parse_diff_lines(patch);
    assert!(lines.contains(&1));
    assert!(lines.contains(&2));
    assert!(lines.contains(&3));
    assert_eq!(lines.len(), 3);
}

#[test]
fn parse_diff_no_hunk_header() {
    let patch = "+added without hunk";
    let lines = parse_diff_lines(patch);
    assert!(lines.contains(&0));
}

#[test]
fn parse_diff_large_line_numbers() {
    let patch = "@@ -500,3 +1000,4 @@\n context\n+added\n context\n+added2";
    let lines = parse_diff_lines(patch);
    assert!(lines.contains(&1000));
    assert!(lines.contains(&1001));
    assert!(lines.contains(&1002));
    assert!(lines.contains(&1003));
}
