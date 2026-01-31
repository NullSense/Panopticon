//! Tests for issue navigation in the link menu modal.
//!
//! These tests verify that:
//! 1. Parent/child selection works correctly
//! 2. Navigation uses sorted/filtered children list (not raw)
//! 3. o/Enter correctly navigates to the selected issue

use chrono::{TimeZone, Utc};
use panopticon::config::{
    CacheConfig, Config, GithubConfig, LinearConfig, NotificationConfig, PollingConfig, Tokens,
    UiConfig, VercelConfig,
};
use panopticon::data::{
    LinearChildRef, LinearIssue, LinearParentRef, LinearPriority, LinearStatus, SortMode,
    Workstream,
};
use panopticon::tui::App;

/// Create a minimal config for testing
fn test_config() -> Config {
    Config {
        tokens: Tokens {
            linear: String::new(),
            github: String::new(),
            vercel: None,
        },
        linear: LinearConfig::default(),
        github: GithubConfig::default(),
        vercel: VercelConfig::default(),
        polling: PollingConfig::default(),
        cache: CacheConfig::default(),
        notifications: NotificationConfig::default(),
        ui: UiConfig::default(),
    }
}

/// Create a workstream with parent and children
fn make_workstream_with_hierarchy(
    id: &str,
    identifier: &str,
    priority: LinearPriority,
    parent: Option<(&str, &str)>,                // (id, identifier)
    children: Vec<(&str, &str, LinearPriority)>, // (id, identifier, priority)
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
            parent: parent.map(|(pid, pident)| LinearParentRef {
                id: pid.to_string(),
                identifier: pident.to_string(),
                title: "Parent".to_string(),
                url: "https://linear.app/test".to_string(),
            }),
            children: children
                .into_iter()
                .map(|(cid, cident, cprio)| LinearChildRef {
                    id: cid.to_string(),
                    identifier: cident.to_string(),
                    title: format!("Child {}", cident),
                    url: format!("https://linear.app/test/issue/{}", cident),
                    status: LinearStatus::InProgress,
                    priority: cprio,
                })
                .collect(),
        },
        github_pr: None,
        vercel_deployment: None,
        agent_sessions: vec![],
        agent_session: None,
        stale: false,
    }
}

/// Simple workstream without hierarchy
fn make_simple_workstream(id: &str, identifier: &str) -> Workstream {
    make_workstream_with_hierarchy(id, identifier, LinearPriority::Medium, None, vec![])
}

// ============================================================================
// Pre-selection Tests
// ============================================================================

#[test]
fn test_preselect_parent_when_opening_link_menu() {
    let config = test_config();
    let mut app = App::new(config);

    // Create parent with children, and add the parent to workstreams
    // Use same priority so insertion order is preserved
    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        None,
        vec![("child-id", "CHILD-1", LinearPriority::Medium)],
    );

    // Create child that references parent
    let child = make_workstream_with_hierarchy(
        "child-id",
        "CHILD-1",
        LinearPriority::Medium,
        Some(("parent-id", "PARENT-1")),
        vec![],
    );

    app.state.workstreams = vec![parent, child];
    app.filtered_indices = vec![0, 1];
    app.rebuild_visual_items();

    // Select the child (index 1 in workstreams, visual index 2 after section header)
    app.visual_selected = 2; // Skip section header

    // Open link menu - should preselect parent since child has a parent
    app.open_link_menu();

    // Verify parent is selected
    assert!(app.parent_selected);
    assert_eq!(app.selected_child_idx, None);
}

#[test]
fn test_preselect_first_child_when_no_parent() {
    let config = test_config();
    let mut app = App::new(config);

    // Create parent with children (no parent of its own)
    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        None,
        vec![
            ("child1-id", "CHILD-1", LinearPriority::High),
            ("child2-id", "CHILD-2", LinearPriority::Low),
        ],
    );

    app.state.workstreams = vec![parent];
    app.filtered_indices = vec![0];
    app.rebuild_visual_items();
    app.visual_selected = 1; // First workstream after section header

    // Open link menu - should preselect first child since no parent
    app.open_link_menu();

    // Verify first child is selected (not parent)
    assert!(!app.parent_selected);
    assert_eq!(app.selected_child_idx, Some(0));
}

// ============================================================================
// Navigation Tests
// ============================================================================

#[test]
fn test_navigate_to_parent_success() {
    let config = test_config();
    let mut app = App::new(config);

    // Create parent and child, both in workstreams
    // Use same priority so insertion order is preserved
    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        None,
        vec![("child-id", "CHILD-1", LinearPriority::Medium)],
    );

    let child = make_workstream_with_hierarchy(
        "child-id",
        "CHILD-1",
        LinearPriority::Medium,
        Some(("parent-id", "PARENT-1")),
        vec![],
    );

    app.state.workstreams = vec![parent, child];
    app.filtered_indices = vec![0, 1];
    app.rebuild_visual_items();
    app.visual_selected = 2; // Select child (index 1 in workstreams, visual index 2)

    // Open link menu on child
    app.open_link_menu();
    assert!(app.parent_selected);

    // Navigate to parent
    let result = app.navigate_to_parent();

    // Should succeed and navigate to parent
    assert!(result);
    assert_eq!(app.modal_issue_id, Some("parent-id".to_string()));
}

#[test]
fn test_navigate_to_parent_not_in_workstreams() {
    let config = test_config();
    let mut app = App::new(config);

    // Create child that references a parent NOT in workstreams
    let child = make_workstream_with_hierarchy(
        "child-id",
        "CHILD-1",
        LinearPriority::High,
        Some(("missing-parent-id", "MISSING-PARENT")),
        vec![],
    );

    app.state.workstreams = vec![child];
    app.filtered_indices = vec![0];
    app.rebuild_visual_items();
    app.visual_selected = 1;

    app.open_link_menu();
    assert!(app.parent_selected);

    // Try to navigate to parent
    let result = app.navigate_to_parent();

    // Should fail (parent not in workstreams)
    assert!(!result);
    assert_eq!(app.modal_issue_id, None);
}

#[test]
fn test_navigate_to_child_success() {
    let config = test_config();
    let mut app = App::new(config);

    // Create parent with children, children also in workstreams
    // Use same priority so insertion order is preserved
    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        None,
        vec![("child-id", "CHILD-1", LinearPriority::Medium)],
    );

    let child = make_workstream_with_hierarchy(
        "child-id",
        "CHILD-1",
        LinearPriority::Medium,
        Some(("parent-id", "PARENT-1")),
        vec![],
    );

    app.state.workstreams = vec![parent, child];
    app.filtered_indices = vec![0, 1];
    app.rebuild_visual_items();
    app.visual_selected = 1; // Select parent (visual index 1 after section header)

    // Open link menu on parent
    app.open_link_menu();

    // Should have first child selected (since parent has no parent)
    assert!(!app.parent_selected);
    assert_eq!(app.selected_child_idx, Some(0));

    // Navigate to selected child
    let result = app.navigate_to_selected_child();

    // Should succeed
    assert!(result);
    assert_eq!(app.modal_issue_id, Some("child-id".to_string()));
}

#[test]
fn test_navigate_to_child_not_in_workstreams() {
    let config = test_config();
    let mut app = App::new(config);

    // Create parent with children NOT in workstreams
    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        None,
        vec![("missing-child-id", "MISSING-CHILD", LinearPriority::High)],
    );

    app.state.workstreams = vec![parent];
    app.filtered_indices = vec![0];
    app.rebuild_visual_items();
    app.visual_selected = 1;

    app.open_link_menu();
    assert_eq!(app.selected_child_idx, Some(0));

    // Try to navigate to child
    let result = app.navigate_to_selected_child();

    // Should fail (child not in workstreams)
    assert!(!result);
}

// ============================================================================
// Sorted/Filtered Children Navigation Tests
// ============================================================================

#[test]
fn test_navigation_with_sorted_children() {
    let config = test_config();
    let mut app = App::new(config);

    // Create parent with children that will be sorted by priority
    // Raw order: [Low, Urgent, Medium]
    // Sorted by priority: [Urgent, Medium, Low]
    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        None,
        vec![
            ("child-low", "CHILD-LOW", LinearPriority::Low),
            ("child-urgent", "CHILD-URGENT", LinearPriority::Urgent),
            ("child-medium", "CHILD-MEDIUM", LinearPriority::Medium),
        ],
    );

    // Add children to workstreams so navigation works
    let child_urgent = make_simple_workstream("child-urgent", "CHILD-URGENT");
    let child_medium = make_simple_workstream("child-medium", "CHILD-MEDIUM");
    let child_low = make_simple_workstream("child-low", "CHILD-LOW");

    app.state.workstreams = vec![parent, child_urgent, child_medium, child_low];
    app.state.sort_mode = SortMode::ByPriority;
    app.filtered_indices = vec![0, 1, 2, 3];
    app.rebuild_visual_items();
    app.visual_selected = 1; // Select parent

    app.open_link_menu();

    // First child should be selected
    assert_eq!(app.selected_child_idx, Some(0));

    // Navigate to selected child (index 0 in sorted list = CHILD-URGENT)
    let result = app.navigate_to_selected_child();

    assert!(result);
    // Should navigate to CHILD-URGENT (first in sorted by priority)
    assert_eq!(app.modal_issue_id, Some("child-urgent".to_string()));
}

#[test]
fn test_next_child_uses_sorted_count() {
    let config = test_config();
    let mut app = App::new(config);

    // Create parent with 3 children
    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        None,
        vec![
            ("child1", "CHILD-1", LinearPriority::High),
            ("child2", "CHILD-2", LinearPriority::Medium),
            ("child3", "CHILD-3", LinearPriority::Low),
        ],
    );

    app.state.workstreams = vec![parent];
    app.filtered_indices = vec![0];
    app.rebuild_visual_items();
    app.visual_selected = 1;

    app.open_link_menu();
    assert_eq!(app.selected_child_idx, Some(0));

    // Navigate through all children
    app.next_child_issue();
    assert_eq!(app.selected_child_idx, Some(1));

    app.next_child_issue();
    assert_eq!(app.selected_child_idx, Some(2));

    // Should not go beyond last child
    app.next_child_issue();
    assert_eq!(app.selected_child_idx, Some(2)); // Still at last
}

#[test]
fn test_prev_child_navigation() {
    let config = test_config();
    let mut app = App::new(config);

    // Create parent with children and a parent reference
    let grandparent = make_simple_workstream("grandparent-id", "GRANDPARENT");

    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        Some(("grandparent-id", "GRANDPARENT")),
        vec![
            ("child1", "CHILD-1", LinearPriority::High),
            ("child2", "CHILD-2", LinearPriority::Medium),
        ],
    );

    app.state.workstreams = vec![grandparent, parent];
    app.filtered_indices = vec![0, 1];
    app.rebuild_visual_items();
    app.visual_selected = 2; // Select parent

    app.open_link_menu();

    // Should start with grandparent selected (has parent)
    assert!(app.parent_selected);
    assert_eq!(app.selected_child_idx, None);

    // Move to first child
    app.next_child_issue();
    assert!(!app.parent_selected);
    assert_eq!(app.selected_child_idx, Some(0));

    // Move to second child
    app.next_child_issue();
    assert_eq!(app.selected_child_idx, Some(1));

    // Move back to first child
    app.prev_child_issue();
    assert_eq!(app.selected_child_idx, Some(0));

    // Move back to parent
    app.prev_child_issue();
    assert!(app.parent_selected);
    assert_eq!(app.selected_child_idx, None);

    // Can't go before parent
    app.prev_child_issue();
    assert!(app.parent_selected);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_no_selection_when_no_parent_and_no_children() {
    let config = test_config();
    let mut app = App::new(config);

    // Create issue with no parent and no children
    let issue = make_simple_workstream("issue-id", "ISSUE-1");

    app.state.workstreams = vec![issue];
    app.filtered_indices = vec![0];
    app.rebuild_visual_items();
    app.visual_selected = 1;

    app.open_link_menu();

    // Nothing should be selected
    assert!(!app.parent_selected);
    assert_eq!(app.selected_child_idx, None);
}

#[test]
fn test_navigation_preserves_stack() {
    let config = test_config();
    let mut app = App::new(config);

    // Create 3-level hierarchy (same priority so insertion order is preserved)
    let grandparent = make_workstream_with_hierarchy(
        "gp-id",
        "GP-1",
        LinearPriority::Medium,
        None,
        vec![("parent-id", "PARENT-1", LinearPriority::Medium)],
    );

    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        Some(("gp-id", "GP-1")),
        vec![("child-id", "CHILD-1", LinearPriority::Medium)],
    );

    let child = make_workstream_with_hierarchy(
        "child-id",
        "CHILD-1",
        LinearPriority::Medium,
        Some(("parent-id", "PARENT-1")),
        vec![],
    );

    app.state.workstreams = vec![grandparent, parent, child];
    app.filtered_indices = vec![0, 1, 2];
    app.rebuild_visual_items();
    app.visual_selected = 1; // Select grandparent

    app.open_link_menu();

    // Navigate to parent (child of grandparent)
    app.navigate_to_selected_child();
    assert_eq!(app.modal_issue_id, Some("parent-id".to_string()));
    assert_eq!(app.issue_navigation_stack.len(), 1);

    // Navigate to child
    app.next_child_issue(); // Move to first child
    app.navigate_to_selected_child();
    assert_eq!(app.modal_issue_id, Some("child-id".to_string()));
    assert_eq!(app.issue_navigation_stack.len(), 2);

    // Navigate back
    app.navigate_back();
    assert_eq!(app.modal_issue_id, Some("parent-id".to_string()));
    assert_eq!(app.issue_navigation_stack.len(), 1);

    app.navigate_back();
    assert_eq!(app.modal_issue_id, Some("gp-id".to_string()));
    assert_eq!(app.issue_navigation_stack.len(), 0);
}

// ============================================================================
// Navigate Back Pre-selection Tests (regression for o/Enter not working)
// ============================================================================

#[test]
fn test_navigate_back_preselects_parent_or_child() {
    let config = test_config();
    let mut app = App::new(config);

    // Create 3-level hierarchy: grandparent -> parent -> child (same priority)
    let grandparent = make_workstream_with_hierarchy(
        "gp-id",
        "GP-1",
        LinearPriority::Medium,
        None,
        vec![("parent-id", "PARENT-1", LinearPriority::Medium)],
    );

    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        Some(("gp-id", "GP-1")),
        vec![("child-id", "CHILD-1", LinearPriority::Medium)],
    );

    let child = make_workstream_with_hierarchy(
        "child-id",
        "CHILD-1",
        LinearPriority::Medium,
        Some(("parent-id", "PARENT-1")),
        vec![],
    );

    app.state.workstreams = vec![grandparent, parent, child];
    app.filtered_indices = vec![0, 1, 2];
    app.rebuild_visual_items();
    app.visual_selected = 1; // Select grandparent

    // Open link menu on grandparent (has children, no parent)
    app.open_link_menu();

    // First child should be pre-selected (grandparent has no parent)
    assert!(!app.parent_selected);
    assert_eq!(app.selected_child_idx, Some(0));

    // Navigate to parent issue
    app.navigate_to_selected_child();
    assert_eq!(app.modal_issue_id, Some("parent-id".to_string()));

    // Parent issue has a parent (grandparent), so parent should be pre-selected
    assert!(app.parent_selected);
    assert_eq!(app.selected_child_idx, None);

    // Navigate back to grandparent
    app.navigate_back();
    assert_eq!(app.modal_issue_id, Some("gp-id".to_string()));

    // CRITICAL: After navigate_back, should still have something selected!
    // Grandparent has no parent, so first child should be selected
    assert!(!app.parent_selected);
    assert_eq!(app.selected_child_idx, Some(0)); // First child selected
}

#[test]
fn test_navigate_back_to_issue_with_parent_preselects_parent() {
    let config = test_config();
    let mut app = App::new(config);

    // Create hierarchy: grandparent -> parent -> child (same priority)
    let grandparent = make_workstream_with_hierarchy(
        "gp-id",
        "GP-1",
        LinearPriority::Medium,
        None,
        vec![("parent-id", "PARENT-1", LinearPriority::Medium)],
    );

    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        Some(("gp-id", "GP-1")),
        vec![("child-id", "CHILD-1", LinearPriority::Medium)],
    );

    let child = make_workstream_with_hierarchy(
        "child-id",
        "CHILD-1",
        LinearPriority::Medium,
        Some(("parent-id", "PARENT-1")),
        vec![],
    );

    app.state.workstreams = vec![grandparent, parent, child];
    app.filtered_indices = vec![0, 1, 2];
    app.rebuild_visual_items();
    app.visual_selected = 2; // Select parent (visual index 2)

    // Open link menu on parent (has both parent and children)
    app.open_link_menu();

    // Parent should be pre-selected (since parent has a parent)
    assert!(app.parent_selected);

    // Navigate down to first child
    app.next_child_issue();
    assert_eq!(app.selected_child_idx, Some(0));

    // Navigate to child issue
    app.navigate_to_selected_child();
    assert_eq!(app.modal_issue_id, Some("child-id".to_string()));

    // Child has parent, so parent should be selected
    assert!(app.parent_selected);

    // Navigate back to parent issue
    app.navigate_back();
    assert_eq!(app.modal_issue_id, Some("parent-id".to_string()));

    // Parent issue has a parent (grandparent), so parent should be pre-selected again
    assert!(app.parent_selected);
    assert_eq!(app.selected_child_idx, None);
}

#[test]
fn test_navigate_back_clears_to_original_issue() {
    let config = test_config();
    let mut app = App::new(config);

    // Parent with one child (same priority)
    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        None,
        vec![("child-id", "CHILD-1", LinearPriority::Medium)],
    );

    let child = make_workstream_with_hierarchy(
        "child-id",
        "CHILD-1",
        LinearPriority::Medium,
        Some(("parent-id", "PARENT-1")),
        vec![],
    );

    app.state.workstreams = vec![parent, child];
    app.filtered_indices = vec![0, 1];
    app.rebuild_visual_items();
    app.visual_selected = 1; // Select parent

    app.open_link_menu();

    // Navigate to child
    app.navigate_to_selected_child();
    assert_eq!(app.modal_issue_id, Some("child-id".to_string()));
    assert_eq!(app.issue_navigation_stack.len(), 1);

    // Navigate back - should return to original (modal_issue_id becomes None)
    app.navigate_back();
    assert_eq!(app.modal_issue_id, Some("parent-id".to_string()));

    // Navigate back again - stack is empty, should clear modal_issue_id
    app.navigate_back();
    assert_eq!(app.modal_issue_id, None);

    // Should still have pre-selection for the original selected workstream (parent)
    // Parent has no parent, so first child should be selected
    assert!(!app.parent_selected);
    assert_eq!(app.selected_child_idx, Some(0));
}

// ============================================================================
// Sort Mode Consistency Tests (App vs UI)
// ============================================================================

/// Create a workstream with children that have different statuses
fn make_workstream_with_status_children(
    id: &str,
    identifier: &str,
    children: Vec<(&str, &str, LinearStatus)>, // (id, identifier, status)
) -> Workstream {
    Workstream {
        linear_issue: LinearIssue {
            id: id.to_string(),
            identifier: identifier.to_string(),
            title: format!("Issue {}", identifier),
            description: None,
            url: format!("https://linear.app/test/issue/{}", identifier),
            status: LinearStatus::InProgress,
            priority: LinearPriority::Medium,
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
            parent: None,
            children: children
                .into_iter()
                .map(|(cid, cident, cstatus)| LinearChildRef {
                    id: cid.to_string(),
                    identifier: cident.to_string(),
                    title: format!("Child {}", cident),
                    url: format!("https://linear.app/test/issue/{}", cident),
                    status: cstatus,
                    priority: LinearPriority::Medium,
                })
                .collect(),
        },
        github_pr: None,
        vercel_deployment: None,
        agent_sessions: vec![],
        agent_session: None,
        stale: false,
    }
}

#[test]
fn test_navigation_with_agent_status_sort_mode() {
    // Bug: When sort mode is ByAgentStatus, UI sorts children by status as fallback,
    // but App's sort_children does nothing (keeps original order).
    // This causes the selected child index to point to a different child than what's visible.
    let config = test_config();
    let mut app = App::new(config);

    // Create parent with children in order: [Done, InProgress, Todo]
    // When sorted by status.sort_order():
    // InProgress=0, Todo=2, Done=5
    // So sorted order is: [InProgress, Todo, Done]
    let parent = make_workstream_with_status_children(
        "parent-id",
        "PARENT-1",
        vec![
            ("child-done", "CHILD-DONE", LinearStatus::Done),
            ("child-progress", "CHILD-PROGRESS", LinearStatus::InProgress),
            ("child-todo", "CHILD-TODO", LinearStatus::Todo),
        ],
    );

    // Add children to workstreams so navigation works
    let child_done = make_simple_workstream("child-done", "CHILD-DONE");
    let child_progress = make_simple_workstream("child-progress", "CHILD-PROGRESS");
    let child_todo = make_simple_workstream("child-todo", "CHILD-TODO");

    app.state.workstreams = vec![parent, child_done, child_progress, child_todo];
    app.state.sort_mode = SortMode::ByAgentStatus; // Uses status fallback in UI
    app.filtered_indices = vec![0, 1, 2, 3];
    app.rebuild_visual_items();
    app.visual_selected = 1; // Select parent

    app.open_link_menu();

    // First child should be selected
    assert_eq!(app.selected_child_idx, Some(0));

    // Navigate to selected child (index 0 should be CHILD-PROGRESS after sorting by status)
    let result = app.navigate_to_selected_child();

    assert!(result);
    // Should navigate to CHILD-PROGRESS (first in sorted by status order)
    // Status sort order: InProgress(0) < Todo(2) < Done(5)
    assert_eq!(
        app.modal_issue_id,
        Some("child-progress".to_string()),
        "ByAgentStatus should sort children by status as fallback"
    );
}

#[test]
fn test_navigation_with_vercel_status_sort_mode() {
    // Same bug as ByAgentStatus - UI sorts by status, App doesn't
    let config = test_config();
    let mut app = App::new(config);

    // Children: Done(5), InProgress(0) -> sorted: InProgress, Done
    let parent = make_workstream_with_status_children(
        "parent-id",
        "PARENT-1",
        vec![
            ("child-done", "CHILD-DONE", LinearStatus::Done),
            ("child-progress", "CHILD-PROGRESS", LinearStatus::InProgress),
        ],
    );

    let child_done = make_simple_workstream("child-done", "CHILD-DONE");
    let child_progress = make_simple_workstream("child-progress", "CHILD-PROGRESS");

    app.state.workstreams = vec![parent, child_done, child_progress];
    app.state.sort_mode = SortMode::ByVercelStatus;
    app.filtered_indices = vec![0, 1, 2];
    app.rebuild_visual_items();
    app.visual_selected = 1;

    app.open_link_menu();
    assert_eq!(app.selected_child_idx, Some(0));

    let result = app.navigate_to_selected_child();

    assert!(result);
    // InProgress(0) comes before Done(5)
    assert_eq!(
        app.modal_issue_id,
        Some("child-progress".to_string()),
        "ByVercelStatus should sort children by status as fallback"
    );
}

#[test]
fn test_navigation_with_pr_activity_sort_mode() {
    let config = test_config();
    let mut app = App::new(config);

    // Children: Done(5), InProgress(0) -> sorted: InProgress, Done
    let parent = make_workstream_with_status_children(
        "parent-id",
        "PARENT-1",
        vec![
            ("child-done", "CHILD-DONE", LinearStatus::Done),
            ("child-progress", "CHILD-PROGRESS", LinearStatus::InProgress),
        ],
    );

    let child_done = make_simple_workstream("child-done", "CHILD-DONE");
    let child_progress = make_simple_workstream("child-progress", "CHILD-PROGRESS");

    app.state.workstreams = vec![parent, child_done, child_progress];
    app.state.sort_mode = SortMode::ByPRActivity;
    app.filtered_indices = vec![0, 1, 2];
    app.rebuild_visual_items();
    app.visual_selected = 1;

    app.open_link_menu();
    let result = app.navigate_to_selected_child();

    assert!(result);
    // InProgress(0) comes before Done(5)
    assert_eq!(
        app.modal_issue_id,
        Some("child-progress".to_string()),
        "ByPRActivity should sort children by status as fallback"
    );
}

#[test]
fn test_navigation_with_last_updated_sort_mode() {
    let config = test_config();
    let mut app = App::new(config);

    // Children: Done(5), InProgress(0) -> sorted: InProgress, Done
    let parent = make_workstream_with_status_children(
        "parent-id",
        "PARENT-1",
        vec![
            ("child-done", "CHILD-DONE", LinearStatus::Done),
            ("child-progress", "CHILD-PROGRESS", LinearStatus::InProgress),
        ],
    );

    let child_done = make_simple_workstream("child-done", "CHILD-DONE");
    let child_progress = make_simple_workstream("child-progress", "CHILD-PROGRESS");

    app.state.workstreams = vec![parent, child_done, child_progress];
    app.state.sort_mode = SortMode::ByLastUpdated;
    app.filtered_indices = vec![0, 1, 2];
    app.rebuild_visual_items();
    app.visual_selected = 1;

    app.open_link_menu();
    let result = app.navigate_to_selected_child();

    assert!(result);
    // InProgress(0) comes before Done(5)
    assert_eq!(
        app.modal_issue_id,
        Some("child-progress".to_string()),
        "ByLastUpdated should sort children by status as fallback"
    );
}

// ============================================================================
// Sub-Issues Scroll Height Tests
// ============================================================================

#[test]
fn test_scroll_uses_dynamic_visible_height_small() {
    // Bug: App used fixed height of 8 for scroll calculations,
    // but UI uses dynamic height (3-10 based on terminal size).
    // This test verifies that small visible height triggers earlier scrolling.
    let config = test_config();
    let mut app = App::new(config);

    // Create parent with 15 children (more than max visible height)
    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        None,
        (0..15)
            .map(|i| {
                (
                    format!("child-{}", i).leak() as &str,
                    format!("CHILD-{}", i).leak() as &str,
                    LinearPriority::Medium,
                )
            })
            .collect(),
    );

    app.state.workstreams = vec![parent];
    app.filtered_indices = vec![0];
    app.rebuild_visual_items();
    app.visual_selected = 1;

    app.open_link_menu();

    // Test with small visible height (3)
    app.set_sub_issues_visible_height(3);

    // Navigate down - scroll logic triggers when idx + 1 >= visible_end
    // With height=3, visible_end = scroll + 3 = 3
    // Scroll triggers when going to idx where idx+1 >= 3, i.e., idx >= 2
    app.next_child_issue(); // idx 0, idx+1=1 < 3, no scroll
    assert_eq!(app.sub_issues_scroll, 0);

    app.next_child_issue(); // idx 1, idx+1=2 < 3, no scroll
    assert_eq!(app.sub_issues_scroll, 0);

    app.next_child_issue(); // idx 2, idx+1=3 >= 3, scroll triggers!
    assert_eq!(
        app.sub_issues_scroll, 1,
        "Should scroll at idx 2 with height 3"
    );
}

#[test]
fn test_scroll_uses_dynamic_visible_height_large() {
    // With a larger visible height, scrolling should trigger later
    let config = test_config();
    let mut app = App::new(config);

    // Create parent with 15 children
    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        None,
        (0..15)
            .map(|i| {
                (
                    format!("child-{}", i).leak() as &str,
                    format!("CHILD-{}", i).leak() as &str,
                    LinearPriority::Medium,
                )
            })
            .collect(),
    );

    app.state.workstreams = vec![parent];
    app.filtered_indices = vec![0];
    app.rebuild_visual_items();
    app.visual_selected = 1;

    app.open_link_menu();

    // Test with larger visible height (10)
    app.set_sub_issues_visible_height(10);

    // Navigate down - scroll triggers when idx+1 >= 10, i.e., idx >= 9
    // First 9 navigations (idx 0-8) should not trigger scroll
    for _ in 0..9 {
        app.next_child_issue();
    }
    // Now at idx 8, idx+1=9 < 10, still no scroll
    assert_eq!(
        app.sub_issues_scroll, 0,
        "Should not scroll at idx 8 with height 10"
    );

    app.next_child_issue(); // idx 9, idx+1=10 >= 10, scroll triggers!
    assert_eq!(
        app.sub_issues_scroll, 1,
        "Should scroll at idx 9 with height 10"
    );
}

#[test]
fn test_different_visible_heights_produce_different_scroll_behavior() {
    // This is the key test: same navigation sequence, different visible heights,
    // should produce different scroll positions.
    let config = test_config();

    // Create parent with 15 children
    let parent = make_workstream_with_hierarchy(
        "parent-id",
        "PARENT-1",
        LinearPriority::Medium,
        None,
        (0..15)
            .map(|i| {
                (
                    format!("child-{}", i).leak() as &str,
                    format!("CHILD-{}", i).leak() as &str,
                    LinearPriority::Medium,
                )
            })
            .collect(),
    );

    // Test with height 3
    let mut app3 = App::new(config.clone());
    app3.state.workstreams = vec![parent.clone()];
    app3.filtered_indices = vec![0];
    app3.rebuild_visual_items();
    app3.visual_selected = 1;
    app3.open_link_menu();
    app3.set_sub_issues_visible_height(3);

    // Navigate 5 times
    for _ in 0..5 {
        app3.next_child_issue();
    }
    let scroll_with_height_3 = app3.sub_issues_scroll;

    // Test with height 10
    let mut app10 = App::new(config);
    app10.state.workstreams = vec![parent];
    app10.filtered_indices = vec![0];
    app10.rebuild_visual_items();
    app10.visual_selected = 1;
    app10.open_link_menu();
    app10.set_sub_issues_visible_height(10);

    // Navigate 5 times
    for _ in 0..5 {
        app10.next_child_issue();
    }
    let scroll_with_height_10 = app10.sub_issues_scroll;

    // With smaller visible height, scroll should have moved more
    assert!(
        scroll_with_height_3 > scroll_with_height_10,
        "Smaller visible height ({}) should result in more scroll ({}) than larger height ({}) scroll ({})",
        3, scroll_with_height_3, 10, scroll_with_height_10
    );
}

#[test]
fn test_default_visible_height_is_eight() {
    let config = test_config();
    let app = App::new(config);

    // Default should be 8 for backward compatibility
    assert_eq!(app.sub_issues_visible_height(), 8);
}

#[test]
fn test_visible_height_clamped_to_valid_range() {
    let config = test_config();
    let mut app = App::new(config);

    // Should clamp to minimum 3
    app.set_sub_issues_visible_height(1);
    assert_eq!(app.sub_issues_visible_height(), 3);

    // Should clamp to maximum 10
    app.set_sub_issues_visible_height(100);
    assert_eq!(app.sub_issues_visible_height(), 10);

    // Values in range should be kept
    app.set_sub_issues_visible_height(5);
    assert_eq!(app.sub_issues_visible_height(), 5);
}
