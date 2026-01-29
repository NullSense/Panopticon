use serde_json::json;

mod test_utils;
use test_utils::*;

#[test]
fn test_parse_issue_with_parent() {
    let mut node = minimal_issue_json();
    node["parent"] = json!({
        "id": "parent-123",
        "identifier": "TEST-100",
        "title": "Parent Issue",
        "url": "https://linear.app/test/issue/TEST-100"
    });

    let result = parse_issue(&node);
    assert!(result.is_some(), "Should parse issue with parent");

    let linked = result.unwrap();
    assert!(linked.issue.parent.is_some());
    let parent = linked.issue.parent.unwrap();
    assert_eq!(parent.id, "parent-123");
    assert_eq!(parent.identifier, "TEST-100");
}

#[test]
fn test_parse_issue_with_children() {
    let mut node = minimal_issue_json();
    node["children"]["nodes"] = json!([
        {
            "id": "child-1",
            "identifier": "TEST-2",
            "title": "Child Issue 1",
            "url": "https://linear.app/test/issue/TEST-2",
            "priority": 1,
            "state": { "name": "Todo", "type": "unstarted" }
        },
        {
            "id": "child-2",
            "identifier": "TEST-3",
            "title": "Child Issue 2",
            "url": "https://linear.app/test/issue/TEST-3",
            "priority": 3,
            "state": { "name": "Done", "type": "completed" }
        }
    ]);

    let result = parse_issue(&node);
    assert!(result.is_some(), "Should parse issue with children");

    let linked = result.unwrap();
    assert_eq!(linked.issue.children.len(), 2);
    assert_eq!(linked.issue.children[0].identifier, "TEST-2");
    assert_eq!(linked.issue.children[1].identifier, "TEST-3");
}

#[test]
fn test_parse_issue_with_incomplete_parent() {
    let mut node = minimal_issue_json();
    node["parent"] = json!({
        "id": "parent-123"
        // missing identifier
    });

    let result = parse_issue(&node);
    assert!(result.is_some(), "Should parse issue even with incomplete parent");

    let linked = result.unwrap();
    assert!(linked.issue.parent.is_none());
}

#[test]
fn test_parse_issue_with_null_parent() {
    let mut node = minimal_issue_json();
    node["parent"] = json!(null);

    let result = parse_issue(&node);
    assert!(result.is_some());

    let linked = result.unwrap();
    assert!(linked.issue.parent.is_none());
}

#[test]
fn test_parse_issue_with_empty_children() {
    let mut node = minimal_issue_json();
    node["children"]["nodes"] = json!([]);

    let result = parse_issue(&node);
    assert!(result.is_some());

    let linked = result.unwrap();
    assert!(linked.issue.children.is_empty());
}

#[test]
fn test_child_with_missing_required_fields() {
    let mut node = minimal_issue_json();
    node["children"]["nodes"] = json!([
        {
            "id": "child-1",
            // missing identifier - should be skipped
            "title": "Child Issue",
            "state": { "type": "started" }
        },
        {
            "id": "child-2",
            "identifier": "TEST-3",
            "title": "Valid Child",
            "url": "https://linear.app/test/issue/TEST-3",
            "priority": 2,
            "state": { "type": "started" }
        }
    ]);

    let result = parse_issue(&node);
    assert!(result.is_some());

    let linked = result.unwrap();
    // Only the valid child should be parsed
    assert_eq!(linked.issue.children.len(), 1);
    assert_eq!(linked.issue.children[0].identifier, "TEST-3");
}
