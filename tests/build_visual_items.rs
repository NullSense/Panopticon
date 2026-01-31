//! Tests for build_visual_items and agent-first section grouping
//!
//! Verifies that workstreams are correctly grouped into Agent Sessions and Issues sections,
//! with proper sorting within each section.

#![allow(clippy::field_reassign_with_default)]

use chrono::Utc;
use panopticon::data::{
    AgentSession, AgentStatus, AgentType, AppState, LinearIssue, LinearPriority, LinearStatus,
    SectionType, VisualItem, Workstream,
};

fn make_workstream(id: &str, identifier: &str, status: LinearStatus) -> Workstream {
    Workstream {
        linear_issue: LinearIssue {
            id: id.to_string(),
            identifier: identifier.to_string(),
            title: format!("Test issue {}", identifier),
            description: None,
            status,
            priority: LinearPriority::Medium,
            url: format!("https://linear.app/test/{}", identifier),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            cycle: None,
            labels: vec![],
            project: None,
            team: Some("Test".to_string()),
            assignee_id: None,
            assignee_name: None,
            estimate: None,
            attachments: vec![],
            parent: None,
            children: vec![],
        },
        github_pr: None,
        vercel_deployment: None,
        agent_session: None,
        stale: false,
    }
}

fn make_workstream_with_agent(
    id: &str,
    identifier: &str,
    status: LinearStatus,
    priority: LinearPriority,
    agent_status: AgentStatus,
) -> Workstream {
    let mut ws = make_workstream(id, identifier, status);
    ws.linear_issue.priority = priority;
    ws.agent_session = Some(AgentSession {
        id: format!("session-{}", id),
        agent_type: AgentType::ClaudeCode,
        status: agent_status,
        working_directory: None,
        git_branch: None,
        last_output: None,
        started_at: Utc::now(),
        last_activity: Utc::now(),
        window_id: None,
        activity: Default::default(),
    });
    ws
}

fn make_workstream_with_priority(
    id: &str,
    identifier: &str,
    status: LinearStatus,
    priority: LinearPriority,
) -> Workstream {
    let mut ws = make_workstream(id, identifier, status);
    ws.linear_issue.priority = priority;
    ws
}

#[test]
fn test_build_visual_items_empty_state() {
    let state = AppState::default();
    let items = state.build_visual_items(&[], false);
    // Empty sections are skipped
    assert!(items.is_empty());
}

#[test]
fn test_build_visual_items_preserves_search_order() {
    let mut state = AppState::default();
    state.workstreams = vec![
        make_workstream("id-0", "TEST-0", LinearStatus::InProgress),
        make_workstream("id-1", "TEST-1", LinearStatus::Todo),
        make_workstream("id-2", "TEST-2", LinearStatus::Done),
    ];

    // In search mode (preserve_order=true), items should be in given order
    let filtered = vec![2, 0, 1]; // Score order: Done, InProgress, Todo
    let items = state.build_visual_items(&filtered, true);

    assert_eq!(items.len(), 3);
    // Should be Workstream items in the exact order given
    if let VisualItem::Workstream(idx) = items[0] {
        assert_eq!(idx, 2);
    } else {
        panic!("Expected Workstream");
    }
    if let VisualItem::Workstream(idx) = items[1] {
        assert_eq!(idx, 0);
    } else {
        panic!("Expected Workstream");
    }
    if let VisualItem::Workstream(idx) = items[2] {
        assert_eq!(idx, 1);
    } else {
        panic!("Expected Workstream");
    }
}

#[test]
fn test_build_visual_items_groups_by_section() {
    let mut state = AppState::default();
    state.workstreams = vec![
        make_workstream_with_agent(
            "id-0",
            "TEST-0",
            LinearStatus::InProgress,
            LinearPriority::High,
            AgentStatus::Running,
        ),
        make_workstream("id-1", "TEST-1", LinearStatus::Todo),
        make_workstream_with_agent(
            "id-2",
            "TEST-2",
            LinearStatus::InProgress,
            LinearPriority::Medium,
            AgentStatus::Idle,
        ),
    ];

    let filtered = vec![0, 1, 2]; // All items
    let items = state.build_visual_items(&filtered, false);

    // Should have 2 section headers (AgentSessions and Issues)
    let mut agent_sessions_header_found = false;
    let mut issues_header_found = false;
    let mut workstream_count = 0;

    for item in &items {
        match item {
            VisualItem::SectionHeader(section) => match section {
                SectionType::AgentSessions => agent_sessions_header_found = true,
                SectionType::Issues => issues_header_found = true,
            },
            VisualItem::Workstream(_) => workstream_count += 1,
        }
    }

    assert!(
        agent_sessions_header_found,
        "AgentSessions section header should exist"
    );
    assert!(issues_header_found, "Issues section header should exist");
    assert_eq!(workstream_count, 3);
}

#[test]
fn test_build_visual_items_filters_correctly() {
    let mut state = AppState::default();
    state.workstreams = vec![
        make_workstream("id-0", "TEST-0", LinearStatus::InProgress),
        make_workstream("id-1", "TEST-1", LinearStatus::Todo),
        make_workstream("id-2", "TEST-2", LinearStatus::Done),
        make_workstream("id-3", "TEST-3", LinearStatus::InProgress),
    ];

    // Only include indices 0 and 2
    let filtered = vec![0, 2];
    let items = state.build_visual_items(&filtered, false);

    // Count only workstream items
    let workstream_indices: Vec<usize> = items
        .iter()
        .filter_map(|item| {
            if let VisualItem::Workstream(idx) = item {
                Some(*idx)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(workstream_indices.len(), 2);
    assert!(workstream_indices.contains(&0));
    assert!(workstream_indices.contains(&2));
    assert!(!workstream_indices.contains(&1));
    assert!(!workstream_indices.contains(&3));
}

#[test]
fn test_build_visual_items_collapsed_sections() {
    let mut state = AppState::default();
    state.workstreams = vec![
        make_workstream_with_agent(
            "id-0",
            "TEST-0",
            LinearStatus::InProgress,
            LinearPriority::High,
            AgentStatus::Running,
        ),
        make_workstream("id-1", "TEST-1", LinearStatus::Todo),
    ];
    state.collapsed_sections.insert(SectionType::AgentSessions);

    let filtered = vec![0, 1];
    let items = state.build_visual_items(&filtered, false);

    // AgentSessions section should be collapsed (header present but no items)
    let mut agent_sessions_count = 0;
    let mut issues_count = 0;
    let mut agent_sessions_header_found = false;

    for item in &items {
        match item {
            VisualItem::SectionHeader(section) => {
                if *section == SectionType::AgentSessions {
                    agent_sessions_header_found = true;
                }
            }
            VisualItem::Workstream(idx) => {
                if state.workstreams[*idx].agent_session.is_some() {
                    agent_sessions_count += 1;
                } else {
                    issues_count += 1;
                }
            }
        }
    }

    assert!(
        agent_sessions_header_found,
        "AgentSessions section header should exist"
    );
    assert_eq!(
        agent_sessions_count, 0,
        "AgentSessions items should be hidden (collapsed)"
    );
    assert_eq!(issues_count, 1, "Issues items should be visible");
}

#[test]
fn test_build_visual_items_large_dataset_correctness() {
    // Test with larger dataset to verify correctness at scale
    let mut state = AppState::default();
    let statuses = [
        LinearStatus::InProgress,
        LinearStatus::Todo,
        LinearStatus::Done,
        LinearStatus::Backlog,
    ];

    state.workstreams = (0..100)
        .map(|i| {
            make_workstream(
                &format!("id-{}", i),
                &format!("TEST-{}", i),
                statuses[i % 4],
            )
        })
        .collect();

    // Filter to only even indices
    let filtered: Vec<usize> = (0..100).filter(|i| i % 2 == 0).collect();
    let items = state.build_visual_items(&filtered, false);

    // Count workstream items
    let workstream_count = items
        .iter()
        .filter(|item| matches!(item, VisualItem::Workstream(_)))
        .count();

    assert_eq!(workstream_count, 50, "Should have 50 filtered workstreams");
}

#[test]
fn test_build_visual_items_maintains_id_to_index_mapping() {
    // Verify that workstream indices in output correctly map to original vec
    let mut state = AppState::default();
    state.workstreams = vec![
        make_workstream("unique-id-A", "TEST-A", LinearStatus::InProgress),
        make_workstream("unique-id-B", "TEST-B", LinearStatus::Todo),
        make_workstream("unique-id-C", "TEST-C", LinearStatus::Done),
    ];

    let filtered = vec![0, 1, 2];
    let items = state.build_visual_items(&filtered, false);

    for item in items {
        if let VisualItem::Workstream(idx) = item {
            // Verify the index is valid
            assert!(idx < state.workstreams.len(), "Index {} out of bounds", idx);
            // Verify we can access the workstream
            let ws = &state.workstreams[idx];
            assert!(
                ws.linear_issue.id.starts_with("unique-id-"),
                "Workstream at index {} has unexpected id",
                idx
            );
        }
    }
}

#[test]
fn test_agent_sessions_sorted_by_status_then_priority() {
    let mut state = AppState::default();
    state.workstreams = vec![
        // Agent with Running status, Medium priority
        make_workstream_with_agent(
            "id-0",
            "TEST-0",
            LinearStatus::InProgress,
            LinearPriority::Medium,
            AgentStatus::Running,
        ),
        // Agent with WaitingForInput status (highest priority), Low priority
        make_workstream_with_agent(
            "id-1",
            "TEST-1",
            LinearStatus::InProgress,
            LinearPriority::Low,
            AgentStatus::WaitingForInput,
        ),
        // Agent with Running status, High priority
        make_workstream_with_agent(
            "id-2",
            "TEST-2",
            LinearStatus::InProgress,
            LinearPriority::High,
            AgentStatus::Running,
        ),
    ];

    let filtered = vec![0, 1, 2];
    let items = state.build_visual_items(&filtered, false);

    // Extract workstream indices in order (skip section headers)
    let workstream_order: Vec<usize> = items
        .iter()
        .filter_map(|item| {
            if let VisualItem::Workstream(idx) = item {
                Some(*idx)
            } else {
                None
            }
        })
        .collect();

    // Expected order:
    // 1. id-1 (WaitingForInput - most urgent agent status)
    // 2. id-2 (Running, High priority)
    // 3. id-0 (Running, Medium priority)
    assert_eq!(
        workstream_order,
        vec![1, 2, 0],
        "Should be sorted by agent status then priority"
    );
}

#[test]
fn test_issues_sorted_by_priority_then_status() {
    let mut state = AppState::default();
    state.workstreams = vec![
        // Medium priority, InProgress
        make_workstream_with_priority(
            "id-0",
            "TEST-0",
            LinearStatus::InProgress,
            LinearPriority::Medium,
        ),
        // Urgent priority, Backlog
        make_workstream_with_priority(
            "id-1",
            "TEST-1",
            LinearStatus::Backlog,
            LinearPriority::Urgent,
        ),
        // Medium priority, Todo (lower status than InProgress)
        make_workstream_with_priority("id-2", "TEST-2", LinearStatus::Todo, LinearPriority::Medium),
    ];

    let filtered = vec![0, 1, 2];
    let items = state.build_visual_items(&filtered, false);

    // Extract workstream indices in order
    let workstream_order: Vec<usize> = items
        .iter()
        .filter_map(|item| {
            if let VisualItem::Workstream(idx) = item {
                Some(*idx)
            } else {
                None
            }
        })
        .collect();

    // Expected order (Issues section, sorted by priority → status):
    // 1. id-1 (Urgent priority - highest)
    // 2. id-0 (Medium priority, InProgress - better status)
    // 3. id-2 (Medium priority, Todo - lower status)
    assert_eq!(
        workstream_order,
        vec![1, 0, 2],
        "Should be sorted by priority then status"
    );
}

#[test]
fn test_agent_sessions_appear_before_issues() {
    let mut state = AppState::default();
    state.workstreams = vec![
        // Issue without agent (should be in Issues section)
        make_workstream("id-0", "TEST-0", LinearStatus::InProgress),
        // Issue with agent (should be in AgentSessions section)
        make_workstream_with_agent(
            "id-1",
            "TEST-1",
            LinearStatus::Todo,
            LinearPriority::Low,
            AgentStatus::Idle,
        ),
        // Another issue without agent
        make_workstream("id-2", "TEST-2", LinearStatus::Backlog),
    ];

    let filtered = vec![0, 1, 2];
    let items = state.build_visual_items(&filtered, false);

    // Find where agent sessions end and issues begin
    let mut in_agent_section = false;
    let mut found_issue_after_agent = false;
    let mut agent_workstream_indices: Vec<usize> = vec![];
    let mut issue_workstream_indices: Vec<usize> = vec![];

    for item in &items {
        match item {
            VisualItem::SectionHeader(SectionType::AgentSessions) => {
                in_agent_section = true;
            }
            VisualItem::SectionHeader(SectionType::Issues) => {
                in_agent_section = false;
            }
            VisualItem::Workstream(idx) => {
                if in_agent_section {
                    agent_workstream_indices.push(*idx);
                    if found_issue_after_agent {
                        panic!("Agent session appeared after Issues section");
                    }
                } else {
                    issue_workstream_indices.push(*idx);
                    if !agent_workstream_indices.is_empty() {
                        found_issue_after_agent = true;
                    }
                }
            }
        }
    }

    // Agent sessions section should contain only id-1
    assert_eq!(
        agent_workstream_indices,
        vec![1],
        "Only agent workstream should be in AgentSessions"
    );
    // Issues section should contain id-0 and id-2
    assert!(
        issue_workstream_indices.contains(&0),
        "Issue 0 should be in Issues section"
    );
    assert!(
        issue_workstream_indices.contains(&2),
        "Issue 2 should be in Issues section"
    );
}
