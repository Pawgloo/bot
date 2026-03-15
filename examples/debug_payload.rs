use octofer::octocrab::models::webhook_events::WebhookEvent;

fn main() {
    let json = r#"{
        "action": "created",
        "issue": {
            "id": 1,
            "url": "https://api.github.com/repos/pawgloo/dummy/issues/1",
            "number": 1,
            "title": "feat: dummy PR for testing",
            "user": { "login": "octocat", "id": 1 },
            "state": "open",
            "pull_request": {
                "url": "https://api.github.com/repos/pawgloo/dummy/pulls/1"
            }
        },
        "comment": {
            "url": "https://api.github.com/repos/pawgloo/dummy/issues/comments/123",
            "id": 123,
            "user": { "login": "octocat", "id": 1 },
            "body": "/pawgloo",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        },
        "repository": {
            "id": 1,
            "name": "dummy",
            "full_name": "pawgloo/dummy",
            "owner": { "login": "pawgloo", "id": 1 }
        },
        "sender": { "login": "octocat", "id": 1 }
    }"#;

    let event = WebhookEvent::try_from_header_and_body("issue_comment", json).unwrap();
    let serialized = serde_json::to_value(&event).unwrap();
    println!("{}", serde_json::to_string_pretty(&serialized).unwrap());

    // Check what keys are at the top level
    if let Some(obj) = serialized.as_object() {
        println!("\n=== Top-level keys ===");
        for key in obj.keys() {
            println!("  {}", key);
        }
    }

    // Try the handler's lookups
    println!("\n=== Handler lookups ===");
    println!("action: {:?}", serialized.get("action"));
    println!("issue: {:?}", serialized.get("issue").is_some());
    println!("comment: {:?}", serialized.get("comment").is_some());
    println!("repository: {:?}", serialized.get("repository").is_some());
}
