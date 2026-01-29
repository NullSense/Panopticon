use serde_json::json;

mod test_utils;
use test_utils::*;

#[test]
fn test_parse_issue_with_cycle() {
    let mut node = minimal_issue_json();
    node["cycle"] = json!({
        "id": "cycle-1",
        "name": "Sprint 5",
        "number": 5,
        "startsAt": "2024-01-01T00:00:00Z",
        "endsAt": "2024-01-14T00:00:00Z"
    });

    let result = parse_issue(&node);
    assert!(result.is_some(), "Should parse issue with cycle");

    let linked = result.unwrap();
    assert!(linked.issue.cycle.is_some());
    let cycle = linked.issue.cycle.unwrap();
    assert_eq!(cycle.id, "cycle-1");
    assert_eq!(cycle.name, "Sprint 5");
    assert_eq!(cycle.number, 5);
}

#[test]
fn test_parse_issue_with_incomplete_cycle() {
    let mut node = minimal_issue_json();
    node["cycle"] = json!({
        "name": "Sprint 5",
        "number": 5
        // missing id
    });

    let result = parse_issue(&node);
    assert!(result.is_some(), "Should parse issue even with incomplete cycle");

    let linked = result.unwrap();
    assert!(linked.issue.cycle.is_none());
}

#[test]
fn test_parse_issue_with_project() {
    let mut node = minimal_issue_json();
    node["project"] = json!({
        "name": "Q1 Project"
    });

    let result = parse_issue(&node);
    assert!(result.is_some(), "Should parse issue with project");

    let linked = result.unwrap();
    assert_eq!(linked.issue.project, Some("Q1 Project".to_string()));
}

#[test]
fn test_parse_issue_with_labels() {
    let mut node = minimal_issue_json();
    node["labels"]["nodes"] = json!([
        { "name": "bug", "color": "#ff0000" },
        { "name": "urgent", "color": "#ff6600" }
    ]);

    let result = parse_issue(&node);
    assert!(result.is_some(), "Should parse issue with labels");

    let linked = result.unwrap();
    assert_eq!(linked.issue.labels.len(), 2);
    assert_eq!(linked.issue.labels[0].name, "bug");
    assert_eq!(linked.issue.labels[1].name, "urgent");
}

#[test]
fn test_parse_issue_with_estimate() {
    let mut node = minimal_issue_json();
    node["estimate"] = json!(3.5);

    let result = parse_issue(&node);
    assert!(result.is_some());

    let linked = result.unwrap();
    assert_eq!(linked.issue.estimate, Some(3.5));
}
