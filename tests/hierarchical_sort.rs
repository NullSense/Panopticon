//! Tests for hierarchical sorting of issues.
//!
//! Ensures parent-child relationships are maintained during sorting,
//! with children always appearing directly after their parent regardless
//! of the sort criteria (priority, status, etc.).

#![allow(clippy::field_reassign_with_default)]

use chrono::{TimeZone, Utc};
use panopticon::data::{
    AppState, LinearIssue, LinearParentRef, LinearPriority, LinearStatus, SortMode, Workstream,
};

/// Create a minimal workstream for testing
fn make_workstream(
    id: &str,
    identifier: &str,
    priority: LinearPriority,
    parent_id: Option<&str>,
    parent_identifier: Option<&str>,
) -> Workstream {
    Workstream {
        linear_issue: LinearIssue {
            id: id.to_string(),
            identifier: identifier.to_string(),
            title: format!("Issue {}", identifier),
            description: None,
            url: format!("https://linear.app/test/issue/{}", identifier),
            status: LinearStatus::InProgress,
            priority,
            cycle: None,
            labels: Vec::new(),
            project: None,
            team: None,
            assignee_id: None,
            assignee_name: None,
            estimate: None,
            created_at: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            attachments: Vec::new(),
            parent: parent_id.map(|pid| LinearParentRef {
                id: pid.to_string(),
                identifier: parent_identifier.unwrap_or("PARENT").to_string(),
                title: "Parent".to_string(),
                url: "https://linear.app/test".to_string(),
            }),
            children: Vec::new(),
        },
        github_pr: None,
        vercel_deployment: None,
        agent_sessions: vec![],
        agent_session: None,
        stale: false,
    }
}

/// Helper to extract identifiers from sorted workstreams for easy assertion
fn get_identifiers<'a>(workstreams: &[&'a Workstream]) -> Vec<&'a str> {
    workstreams
        .iter()
        .map(|ws| ws.linear_issue.identifier.as_str())
        .collect()
}

// ============================================================================
// Basic Parent-Child Tests
// ============================================================================

#[test]
fn test_child_appears_after_parent_when_sorted_by_priority() {
    // Setup: Parent has Low priority, Child has Urgent priority
    // Without hierarchical sort, Child would appear first (Urgent < Low)
    // With hierarchical sort, Parent must appear first, then Child
    let parent = make_workstream("1", "TEST-1", LinearPriority::Low, None, None);
    let child = make_workstream(
        "2",
        "TEST-2",
        LinearPriority::Urgent,
        Some("1"),
        Some("TEST-1"),
    );

    let mut state = AppState::default();
    state.workstreams = vec![child.clone(), parent.clone()]; // Child added first
    state.sort_mode = SortMode::ByPriority;

    let grouped = state.grouped_workstreams();
    assert_eq!(grouped.len(), 1); // All InProgress

    let (_, workstreams) = &grouped[0];
    let ids = get_identifiers(workstreams);

    // Parent (TEST-1) must come before Child (TEST-2) despite lower priority
    assert_eq!(ids, vec!["TEST-1", "TEST-2"]);
}

#[test]
fn test_multiple_children_sorted_under_parent() {
    // Parent with 3 children, children should be sorted by priority under parent
    let parent = make_workstream("1", "TEST-1", LinearPriority::Medium, None, None);
    let child_low = make_workstream(
        "2",
        "TEST-2",
        LinearPriority::Low,
        Some("1"),
        Some("TEST-1"),
    );
    let child_urgent = make_workstream(
        "3",
        "TEST-3",
        LinearPriority::Urgent,
        Some("1"),
        Some("TEST-1"),
    );
    let child_high = make_workstream(
        "4",
        "TEST-4",
        LinearPriority::High,
        Some("1"),
        Some("TEST-1"),
    );

    let mut state = AppState::default();
    state.workstreams = vec![child_low, parent, child_high, child_urgent]; // Random order
    state.sort_mode = SortMode::ByPriority;

    let grouped = state.grouped_workstreams();
    let (_, workstreams) = &grouped[0];
    let ids = get_identifiers(workstreams);

    // Parent first, then children sorted by priority (Urgent, High, Low)
    assert_eq!(ids, vec!["TEST-1", "TEST-3", "TEST-4", "TEST-2"]);
}

#[test]
fn test_multiple_parent_trees_sorted() {
    // Two parents, each with children
    // Parents should be sorted by priority, children under each parent also sorted
    let parent1 = make_workstream("1", "TEST-1", LinearPriority::Low, None, None);
    let parent2 = make_workstream("2", "TEST-2", LinearPriority::Urgent, None, None);
    let child1a = make_workstream(
        "3",
        "TEST-3",
        LinearPriority::High,
        Some("1"),
        Some("TEST-1"),
    );
    let child1b = make_workstream(
        "4",
        "TEST-4",
        LinearPriority::Medium,
        Some("1"),
        Some("TEST-1"),
    );
    let child2a = make_workstream(
        "5",
        "TEST-5",
        LinearPriority::Low,
        Some("2"),
        Some("TEST-2"),
    );

    let mut state = AppState::default();
    state.workstreams = vec![child1b, parent1, child2a, parent2, child1a];
    state.sort_mode = SortMode::ByPriority;

    let grouped = state.grouped_workstreams();
    let (_, workstreams) = &grouped[0];
    let ids = get_identifiers(workstreams);

    // Parent2 (Urgent) first with its children, then Parent1 (Low) with its children
    // Children sorted by priority within their parent group
    assert_eq!(ids, vec!["TEST-2", "TEST-5", "TEST-1", "TEST-3", "TEST-4"]);
}

// ============================================================================
// Deep Nesting Tests (up to 5 levels)
// ============================================================================

#[test]
fn test_three_level_nesting() {
    // Grandparent -> Parent -> Child
    let grandparent = make_workstream("1", "TEST-1", LinearPriority::Low, None, None);
    let parent = make_workstream(
        "2",
        "TEST-2",
        LinearPriority::Urgent,
        Some("1"),
        Some("TEST-1"),
    );
    let child = make_workstream(
        "3",
        "TEST-3",
        LinearPriority::High,
        Some("2"),
        Some("TEST-2"),
    );

    let mut state = AppState::default();
    state.workstreams = vec![child, grandparent, parent]; // Random order
    state.sort_mode = SortMode::ByPriority;

    let grouped = state.grouped_workstreams();
    let (_, workstreams) = &grouped[0];
    let ids = get_identifiers(workstreams);

    // Must maintain hierarchy: Grandparent -> Parent -> Child
    assert_eq!(ids, vec!["TEST-1", "TEST-2", "TEST-3"]);
}

#[test]
fn test_four_level_nesting() {
    // Level 1 -> Level 2 -> Level 3 -> Level 4
    let l1 = make_workstream("1", "L1", LinearPriority::Low, None, None);
    let l2 = make_workstream("2", "L2", LinearPriority::Urgent, Some("1"), Some("L1"));
    let l3 = make_workstream("3", "L3", LinearPriority::High, Some("2"), Some("L2"));
    let l4 = make_workstream("4", "L4", LinearPriority::Medium, Some("3"), Some("L3"));

    let mut state = AppState::default();
    state.workstreams = vec![l4, l2, l1, l3]; // Random order
    state.sort_mode = SortMode::ByPriority;

    let grouped = state.grouped_workstreams();
    let (_, workstreams) = &grouped[0];
    let ids = get_identifiers(workstreams);

    // Must maintain hierarchy regardless of priority
    assert_eq!(ids, vec!["L1", "L2", "L3", "L4"]);
}

#[test]
fn test_five_level_nesting() {
    // Maximum supported nesting: L1 -> L2 -> L3 -> L4 -> L5
    let l1 = make_workstream("1", "L1", LinearPriority::NoPriority, None, None);
    let l2 = make_workstream("2", "L2", LinearPriority::Urgent, Some("1"), Some("L1"));
    let l3 = make_workstream("3", "L3", LinearPriority::High, Some("2"), Some("L2"));
    let l4 = make_workstream("4", "L4", LinearPriority::Medium, Some("3"), Some("L3"));
    let l5 = make_workstream("5", "L5", LinearPriority::Low, Some("4"), Some("L4"));

    let mut state = AppState::default();
    // Add in completely reversed order to stress test
    state.workstreams = vec![l5, l4, l3, l2, l1];
    state.sort_mode = SortMode::ByPriority;

    let grouped = state.grouped_workstreams();
    let (_, workstreams) = &grouped[0];
    let ids = get_identifiers(workstreams);

    // Must maintain full 5-level hierarchy
    assert_eq!(ids, vec!["L1", "L2", "L3", "L4", "L5"]);
}

#[test]
fn test_five_level_with_siblings_at_each_level() {
    // Complex tree with siblings at multiple levels
    //
    // ROOT-1 (Low)
    // ├── A1 (Urgent)
    // │   ├── B1 (High)
    // │   │   └── C1 (Medium)
    // │   │       └── D1 (Low)
    // │   └── B2 (Low)
    // └── A2 (High)
    //
    // ROOT-2 (Urgent)
    // └── X1 (Medium)

    let root1 = make_workstream("r1", "ROOT-1", LinearPriority::Low, None, None);
    let a1 = make_workstream(
        "a1",
        "A1",
        LinearPriority::Urgent,
        Some("r1"),
        Some("ROOT-1"),
    );
    let a2 = make_workstream("a2", "A2", LinearPriority::High, Some("r1"), Some("ROOT-1"));
    let b1 = make_workstream("b1", "B1", LinearPriority::High, Some("a1"), Some("A1"));
    let b2 = make_workstream("b2", "B2", LinearPriority::Low, Some("a1"), Some("A1"));
    let c1 = make_workstream("c1", "C1", LinearPriority::Medium, Some("b1"), Some("B1"));
    let d1 = make_workstream("d1", "D1", LinearPriority::Low, Some("c1"), Some("C1"));
    let root2 = make_workstream("r2", "ROOT-2", LinearPriority::Urgent, None, None);
    let x1 = make_workstream(
        "x1",
        "X1",
        LinearPriority::Medium,
        Some("r2"),
        Some("ROOT-2"),
    );

    let mut state = AppState::default();
    // Shuffle order completely
    state.workstreams = vec![d1, x1, b2, root1, c1, a2, root2, b1, a1];
    state.sort_mode = SortMode::ByPriority;

    let grouped = state.grouped_workstreams();
    let (_, workstreams) = &grouped[0];
    let ids = get_identifiers(workstreams);

    // ROOT-2 (Urgent) comes first with its subtree
    // Then ROOT-1 (Low) with its subtree
    // Within ROOT-1: A1 (Urgent) before A2 (High)
    // Within A1: B1 (High) before B2 (Low)
    assert_eq!(
        ids,
        vec!["ROOT-2", "X1", "ROOT-1", "A1", "B1", "C1", "D1", "B2", "A2"]
    );
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_orphaned_child_treated_as_root() {
    // Child whose parent is not in the workstreams list (filtered out, etc.)
    // Should be treated as a root
    let orphan = make_workstream(
        "2",
        "TEST-2",
        LinearPriority::Urgent,
        Some("1"),
        Some("TEST-1"),
    );
    let root = make_workstream("3", "TEST-3", LinearPriority::Low, None, None);

    let mut state = AppState::default();
    state.workstreams = vec![root, orphan];
    state.sort_mode = SortMode::ByPriority;

    let grouped = state.grouped_workstreams();
    let (_, workstreams) = &grouped[0];
    let ids = get_identifiers(workstreams);

    // Orphan (Urgent) should be treated as root and sorted first
    assert_eq!(ids, vec!["TEST-2", "TEST-3"]);
}

#[test]
fn test_empty_workstreams() {
    let state = AppState::default();
    let grouped = state.grouped_workstreams();
    assert!(grouped.is_empty());
}

#[test]
fn test_single_workstream() {
    let ws = make_workstream("1", "TEST-1", LinearPriority::Medium, None, None);
    let mut state = AppState::default();
    state.workstreams = vec![ws];

    let grouped = state.grouped_workstreams();
    assert_eq!(grouped.len(), 1);
    let (_, workstreams) = &grouped[0];
    assert_eq!(workstreams.len(), 1);
    assert_eq!(workstreams[0].linear_issue.identifier, "TEST-1");
}

#[test]
fn test_all_roots_no_children() {
    // No parent-child relationships - should sort normally by priority
    let ws1 = make_workstream("1", "TEST-1", LinearPriority::Low, None, None);
    let ws2 = make_workstream("2", "TEST-2", LinearPriority::Urgent, None, None);
    let ws3 = make_workstream("3", "TEST-3", LinearPriority::High, None, None);

    let mut state = AppState::default();
    state.workstreams = vec![ws1, ws2, ws3];
    state.sort_mode = SortMode::ByPriority;

    let grouped = state.grouped_workstreams();
    let (_, workstreams) = &grouped[0];
    let ids = get_identifiers(workstreams);

    // Sorted by priority: Urgent, High, Low
    assert_eq!(ids, vec!["TEST-2", "TEST-3", "TEST-1"]);
}

// ============================================================================
// Sort Mode Tests
// ============================================================================

#[test]
fn test_hierarchical_sort_by_identifier() {
    let parent = make_workstream("1", "ZZZ-1", LinearPriority::Medium, None, None);
    let child1 = make_workstream(
        "2",
        "AAA-2",
        LinearPriority::Medium,
        Some("1"),
        Some("ZZZ-1"),
    );
    let child2 = make_workstream(
        "3",
        "MMM-3",
        LinearPriority::Medium,
        Some("1"),
        Some("ZZZ-1"),
    );

    let mut state = AppState::default();
    state.workstreams = vec![child2, parent, child1];
    state.sort_mode = SortMode::ByLinearStatus; // Uses identifier as tiebreaker

    let grouped = state.grouped_workstreams();
    let (_, workstreams) = &grouped[0];
    let ids = get_identifiers(workstreams);

    // Parent first, children sorted by identifier (AAA, MMM)
    assert_eq!(ids, vec!["ZZZ-1", "AAA-2", "MMM-3"]);
}

// ============================================================================
// Regression Tests
// ============================================================================

#[test]
fn test_dre_372_regression() {
    // Regression test for the bug where DRE-372 appeared as parent to DRE-279
    // This happened because sorting didn't respect parent-child relationships
    //
    // Correct hierarchy: DRE-372 is parent, DRE-373/374/etc are children
    let dre_372 = make_workstream("372", "DRE-372", LinearPriority::High, None, None);
    let dre_373 = make_workstream(
        "373",
        "DRE-373",
        LinearPriority::Urgent,
        Some("372"),
        Some("DRE-372"),
    );
    let dre_374 = make_workstream(
        "374",
        "DRE-374",
        LinearPriority::Medium,
        Some("372"),
        Some("DRE-372"),
    );

    let mut state = AppState::default();
    // Add in wrong order - if sorting is broken, DRE-373 would appear first due to Urgent priority
    state.workstreams = vec![dre_374, dre_373, dre_372];
    state.sort_mode = SortMode::ByPriority;

    let grouped = state.grouped_workstreams();
    let (_, workstreams) = &grouped[0];
    let ids = get_identifiers(workstreams);

    // Parent must come first, then children sorted by priority
    assert_eq!(ids, vec!["DRE-372", "DRE-373", "DRE-374"]);
}
