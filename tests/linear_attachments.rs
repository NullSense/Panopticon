use serde_json::json;

mod test_utils;
use test_utils::*;

#[test]
fn test_parse_issue_with_attachments() {
    let mut node = minimal_issue_json();
    node["attachments"]["nodes"] = json!([
        {
            "id": "attach-1",
            "url": "https://example.com/doc1",
            "title": "Design Doc",
            "subtitle": "Version 1",
            "sourceType": "notion"
        },
        {
            "id": "attach-2",
            "url": "https://github.com/org/repo/pull/123",
            "title": "PR #123",
            "subtitle": null,
            "sourceType": "github"
        }
    ]);

    let result = parse_issue(&node);
    assert!(result.is_some(), "Should parse issue with attachments");

    let linked = result.unwrap();
    assert_eq!(linked.linked_pr_url, Some("https://github.com/org/repo/pull/123".to_string()));
    assert_eq!(linked.issue.attachments.len(), 1);
    assert_eq!(linked.issue.attachments[0].title, "Design Doc");
}

#[test]
fn test_parse_issue_with_empty_attachments() {
    let mut node = minimal_issue_json();
    node["attachments"]["nodes"] = json!([]);

    let result = parse_issue(&node);
    assert!(result.is_some());

    let linked = result.unwrap();
    assert!(linked.linked_pr_url.is_none());
    assert!(linked.issue.attachments.is_empty());
}

#[test]
fn test_parse_issue_with_null_attachments() {
    let mut node = minimal_issue_json();
    node["attachments"] = json!(null);

    let result = parse_issue(&node);
    assert!(result.is_some());

    let linked = result.unwrap();
    assert!(linked.issue.attachments.is_empty());
}

#[test]
fn test_attachment_with_missing_fields() {
    let mut node = minimal_issue_json();
    node["attachments"]["nodes"] = json!([
        {
            "url": "https://example.com/doc1"
            // missing id, title, etc
        }
    ]);

    let result = parse_issue(&node);
    assert!(result.is_some(), "Should still parse issue");

    let linked = result.unwrap();
    assert_eq!(linked.issue.attachments.len(), 1);
    assert_eq!(linked.issue.attachments[0].title, "Untitled");
}
