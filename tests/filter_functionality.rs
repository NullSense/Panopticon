//! Tests for filter functionality including project and assignee filters.
//!
//! These tests verify:
//! 1. Project filter toggle adds/removes project IDs
//! 2. Assignee filter toggle handles special indices (me, unassigned, team members)
//! 3. Filter application correctly filters workstreams by project
//! 4. clear_all_filters clears all filter types
//! 5. has_active_filters includes new filter types

use chrono::{TimeZone, Utc};
use panopticon::config::{
    CacheConfig, Config, GithubConfig, LinearConfig, NotificationConfig, PollingConfig, Tokens,
    UiConfig, VercelConfig,
};
use panopticon::data::{LinearIssue, LinearPriority, LinearStatus, Workstream};
use panopticon::integrations::linear::{ProjectInfo, TeamMemberInfo};
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

/// Create a workstream with optional project
fn make_workstream_with_project(id: &str, identifier: &str, project: Option<&str>) -> Workstream {
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
            project: project.map(|p| p.to_string()),
            team: None,
            estimate: None,
            created_at: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            attachments: Vec::new(),
            parent: None,
            children: Vec::new(),
        },
        github_pr: None,
        vercel_deployment: None,
        agent_session: None,
        stale: false,
    }
}

/// Create a simple workstream without project
fn make_simple_workstream(id: &str, identifier: &str) -> Workstream {
    make_workstream_with_project(id, identifier, None)
}

// ============================================================================
// Project Filter Tests
// ============================================================================

#[test]
fn test_toggle_project_filter_adds_project() {
    let config = test_config();
    let mut app = App::new(config);

    // Setup available projects
    app.available_projects = vec![
        ProjectInfo {
            id: "proj-1".to_string(),
            name: "Project Alpha".to_string(),
        },
        ProjectInfo {
            id: "proj-2".to_string(),
            name: "Project Beta".to_string(),
        },
    ];

    // Initially no project filters
    assert!(app.filter_projects.is_empty());

    // Toggle first project filter
    app.toggle_project_filter(0);
    assert!(app.filter_projects.contains("proj-1"));
    assert_eq!(app.filter_projects.len(), 1);
}

#[test]
fn test_toggle_project_filter_removes_project() {
    let config = test_config();
    let mut app = App::new(config);

    app.available_projects = vec![ProjectInfo {
        id: "proj-1".to_string(),
        name: "Project Alpha".to_string(),
    }];

    // Add filter
    app.toggle_project_filter(0);
    assert!(app.filter_projects.contains("proj-1"));

    // Toggle again to remove
    app.toggle_project_filter(0);
    assert!(!app.filter_projects.contains("proj-1"));
    assert!(app.filter_projects.is_empty());
}

#[test]
fn test_toggle_project_filter_invalid_index_ignored() {
    let config = test_config();
    let mut app = App::new(config);

    app.available_projects = vec![ProjectInfo {
        id: "proj-1".to_string(),
        name: "Project Alpha".to_string(),
    }];

    // Invalid index should be ignored
    app.toggle_project_filter(99);
    assert!(app.filter_projects.is_empty());
}

// ============================================================================
// Assignee Filter Tests
// ============================================================================

#[test]
fn test_toggle_assignee_filter_me() {
    let config = test_config();
    let mut app = App::new(config);

    // Index 0 is "me"
    app.toggle_assignee_filter(0);
    assert!(app.filter_assignees.contains("me"));
    assert_eq!(app.filter_assignees.len(), 1);

    // Toggle again to remove
    app.toggle_assignee_filter(0);
    assert!(!app.filter_assignees.contains("me"));
}

#[test]
fn test_toggle_assignee_filter_unassigned() {
    let config = test_config();
    let mut app = App::new(config);

    // Index 1 is "unassigned"
    app.toggle_assignee_filter(1);
    assert!(app.filter_assignees.contains("unassigned"));
    assert_eq!(app.filter_assignees.len(), 1);
}

#[test]
fn test_toggle_assignee_filter_team_member() {
    let config = test_config();
    let mut app = App::new(config);

    app.available_team_members = vec![
        TeamMemberInfo {
            id: "user-1".to_string(),
            name: "Alice".to_string(),
            display_name: Some("Alice A.".to_string()),
            email: Some("alice@example.com".to_string()),
        },
        TeamMemberInfo {
            id: "user-2".to_string(),
            name: "Bob".to_string(),
            display_name: None,
            email: None,
        },
    ];

    // Index 2 maps to first team member (idx - 2 = 0)
    app.toggle_assignee_filter(2);
    assert!(app.filter_assignees.contains("user-1"));

    // Index 3 maps to second team member (idx - 2 = 1)
    app.toggle_assignee_filter(3);
    assert!(app.filter_assignees.contains("user-2"));
    assert_eq!(app.filter_assignees.len(), 2);
}

#[test]
fn test_toggle_assignee_filter_invalid_team_member_index() {
    let config = test_config();
    let mut app = App::new(config);

    app.available_team_members = vec![TeamMemberInfo {
        id: "user-1".to_string(),
        name: "Alice".to_string(),
        display_name: None,
        email: None,
    }];

    // Index 99 is out of bounds, should be ignored
    app.toggle_assignee_filter(99);
    assert!(app.filter_assignees.is_empty());
}

// ============================================================================
// Filter Application Tests
// ============================================================================

#[test]
fn test_apply_filters_by_project() {
    let config = test_config();
    let mut app = App::new(config);

    // Setup workstreams with different projects
    app.state.workstreams = vec![
        make_workstream_with_project("1", "TEST-1", Some("Project Alpha")),
        make_workstream_with_project("2", "TEST-2", Some("Project Beta")),
        make_workstream_with_project("3", "TEST-3", Some("Project Alpha")),
        make_workstream_with_project("4", "TEST-4", None), // No project
    ];

    app.available_projects = vec![
        ProjectInfo {
            id: "proj-alpha".to_string(),
            name: "Project Alpha".to_string(),
        },
        ProjectInfo {
            id: "proj-beta".to_string(),
            name: "Project Beta".to_string(),
        },
    ];

    // No filter = show all
    app.apply_filters();
    assert_eq!(app.filtered_indices.len(), 4);

    // Filter by Project Alpha only
    app.filter_projects.insert("proj-alpha".to_string());
    app.apply_filters();

    // Should only show issues with Project Alpha (indices 0 and 2)
    assert_eq!(app.filtered_indices.len(), 2);
    assert!(app.filtered_indices.contains(&0));
    assert!(app.filtered_indices.contains(&2));
    assert!(!app.filtered_indices.contains(&1)); // Project Beta
    assert!(!app.filtered_indices.contains(&3)); // No project
}

#[test]
fn test_apply_filters_multiple_projects() {
    let config = test_config();
    let mut app = App::new(config);

    app.state.workstreams = vec![
        make_workstream_with_project("1", "TEST-1", Some("Project Alpha")),
        make_workstream_with_project("2", "TEST-2", Some("Project Beta")),
        make_workstream_with_project("3", "TEST-3", Some("Project Gamma")),
    ];

    app.available_projects = vec![
        ProjectInfo {
            id: "proj-alpha".to_string(),
            name: "Project Alpha".to_string(),
        },
        ProjectInfo {
            id: "proj-beta".to_string(),
            name: "Project Beta".to_string(),
        },
        ProjectInfo {
            id: "proj-gamma".to_string(),
            name: "Project Gamma".to_string(),
        },
    ];

    // Filter by Alpha and Beta
    app.filter_projects.insert("proj-alpha".to_string());
    app.filter_projects.insert("proj-beta".to_string());
    app.apply_filters();

    // Should show Alpha and Beta (indices 0 and 1)
    assert_eq!(app.filtered_indices.len(), 2);
    assert!(app.filtered_indices.contains(&0));
    assert!(app.filtered_indices.contains(&1));
    assert!(!app.filtered_indices.contains(&2)); // Gamma not selected
}

// ============================================================================
// Clear All Filters Tests
// ============================================================================

#[test]
fn test_clear_all_filters_clears_projects_and_assignees() {
    let config = test_config();
    let mut app = App::new(config);

    // Add some workstreams so rebuild doesn't panic
    app.state.workstreams = vec![make_simple_workstream("1", "TEST-1")];

    // Setup some filters
    app.filter_projects.insert("proj-1".to_string());
    app.filter_assignees.insert("me".to_string());
    app.filter_assignees.insert("user-1".to_string());
    app.filter_priorities.insert(LinearPriority::High);

    // Verify filters are set
    assert!(!app.filter_projects.is_empty());
    assert!(!app.filter_assignees.is_empty());
    assert!(!app.filter_priorities.is_empty());

    // Clear all
    app.clear_all_filters();

    // All should be empty
    assert!(app.filter_projects.is_empty());
    assert!(app.filter_assignees.is_empty());
    assert!(app.filter_priorities.is_empty());
    assert!(app.filter_cycles.is_empty());
}

// ============================================================================
// Has Active Filters Tests
// ============================================================================

#[test]
fn test_has_active_filters_detects_project_filter() {
    let config = test_config();
    let mut app = App::new(config);

    assert!(!app.has_active_filters());

    app.filter_projects.insert("proj-1".to_string());
    assert!(app.has_active_filters());
}

#[test]
fn test_has_active_filters_detects_assignee_filter() {
    let config = test_config();
    let mut app = App::new(config);

    assert!(!app.has_active_filters());

    app.filter_assignees.insert("me".to_string());
    assert!(app.has_active_filters());
}

#[test]
fn test_has_active_filters_all_types() {
    let config = test_config();
    let mut app = App::new(config);

    assert!(!app.has_active_filters());

    // Add each filter type
    app.filter_cycles.insert("cycle-1".to_string());
    assert!(app.has_active_filters());

    app.filter_cycles.clear();
    app.filter_priorities.insert(LinearPriority::High);
    assert!(app.has_active_filters());

    app.filter_priorities.clear();
    app.filter_projects.insert("proj-1".to_string());
    assert!(app.has_active_filters());

    app.filter_projects.clear();
    app.filter_assignees.insert("user-1".to_string());
    assert!(app.has_active_filters());

    app.filter_assignees.clear();
    assert!(!app.has_active_filters());
}
