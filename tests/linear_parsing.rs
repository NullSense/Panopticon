use serde_json::json;

// Re-export test utilities
mod test_utils;
use test_utils::*;

#[test]
fn test_parse_minimal_issue() {
    let node = minimal_issue_json();
    let result = parse_issue(&node);

    assert!(result.is_some(), "Should parse minimal issue successfully");
    let linked = result.unwrap();
    assert_eq!(linked.issue.id, "issue-123");
    assert_eq!(linked.issue.identifier, "TEST-1");
    assert_eq!(linked.issue.title, "Test Issue");
}

#[test]
fn test_parse_issue_with_null_optional_fields() {
    let mut node = minimal_issue_json();
    node["project"] = json!(null);
    node["cycle"] = json!(null);
    node["parent"] = json!(null);
    node["estimate"] = json!(null);
    node["description"] = json!(null);

    let result = parse_issue(&node);
    assert!(result.is_some(), "Should parse issue with null optional fields");

    let linked = result.unwrap();
    assert!(linked.issue.project.is_none());
    assert!(linked.issue.cycle.is_none());
    assert!(linked.issue.parent.is_none());
    assert!(linked.issue.estimate.is_none());
    assert!(linked.issue.description.is_none());
}

#[test]
fn test_parse_issue_missing_required_id() {
    let node = json!({
        "identifier": "TEST-1",
        "title": "Test",
        "state": { "type": "started" }
    });

    let result = parse_issue(&node);
    assert!(result.is_none(), "Should fail to parse issue missing id");
}

#[test]
fn test_parse_issue_missing_state() {
    let node = json!({
        "id": "issue-123",
        "identifier": "TEST-1",
        "title": "Test",
        "url": "https://linear.app/test",
        "createdAt": "2024-01-01T00:00:00Z",
        "updatedAt": "2024-01-01T00:00:00Z"
    });

    let result = parse_issue(&node);
    assert!(result.is_none(), "Should fail to parse issue missing state");
}

#[test]
fn test_all_status_types() {
    let statuses = [
        ("backlog", "Backlog"),
        ("unstarted", "Todo"),
        ("started", "In Progress"),
        ("completed", "Done"),
        ("canceled", "Canceled"),
    ];

    for (state_type, expected_display) in statuses {
        let mut node = minimal_issue_json();
        node["state"]["type"] = json!(state_type);

        let result = parse_issue(&node);
        assert!(result.is_some(), "Should parse issue with state type: {}", state_type);

        let linked = result.unwrap();
        assert_eq!(
            linked.issue.status.display_name(),
            expected_display,
            "Status display name mismatch for type: {}",
            state_type
        );
    }
}

#[test]
fn test_review_status_detection() {
    let mut node = minimal_issue_json();
    node["state"] = json!({
        "name": "In Review",
        "type": "custom"
    });

    let result = parse_issue(&node);
    assert!(result.is_some(), "Should parse issue with review state");

    let linked = result.unwrap();
    assert_eq!(linked.issue.status.display_name(), "In Review");
}

#[test]
fn test_review_status_with_started_type() {
    // This tests the fix for the bug where type "started" with name "In Review"
    // was incorrectly mapped to InProgress instead of InReview
    let mut node = minimal_issue_json();
    node["state"] = json!({
        "name": "In Review",
        "type": "started"  // Common Linear configuration
    });

    let result = parse_issue(&node);
    assert!(result.is_some(), "Should parse issue with started type and review name");

    let linked = result.unwrap();
    assert_eq!(
        linked.issue.status.display_name(),
        "In Review",
        "Type 'started' with name 'In Review' should map to InReview status"
    );
}

#[test]
fn test_code_review_status_with_started_type() {
    // Another common variant - "Code Review" instead of "In Review"
    let mut node = minimal_issue_json();
    node["state"] = json!({
        "name": "Code Review",
        "type": "started"
    });

    let result = parse_issue(&node);
    assert!(result.is_some());

    let linked = result.unwrap();
    assert_eq!(
        linked.issue.status.display_name(),
        "In Review",
        "Type 'started' with name 'Code Review' should map to InReview status"
    );
}
