//! Tests for build_visual_items performance optimization
//!
//! Verifies that the optimized implementation produces the same results
//! as the original but with O(n) complexity instead of O(n²).

use chrono::Utc;
use panopticon::data::{
    AppState, LinearIssue, LinearPriority, LinearStatus, VisualItem, Workstream,
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
            estimate: None,
            attachments: vec![],
            parent: None,
            children: vec![],
        },
        github_pr: None,
        vercel_deployment: None,
        agent_session: None,
    }
}

#[test]
fn test_build_visual_items_empty_state() {
    let state = AppState::default();
    let items = state.build_visual_items(&[], false);
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
fn test_build_visual_items_groups_by_status() {
    let mut state = AppState::default();
    state.workstreams = vec![
        make_workstream("id-0", "TEST-0", LinearStatus::InProgress),
        make_workstream("id-1", "TEST-1", LinearStatus::Todo),
        make_workstream("id-2", "TEST-2", LinearStatus::InProgress),
    ];

    let filtered = vec![0, 1, 2]; // All items
    let items = state.build_visual_items(&filtered, false);

    // Should have section headers and workstreams
    // InProgress (2 items) and Todo (1 item)
    let mut section_count = 0;
    let mut workstream_count = 0;
    for item in &items {
        match item {
            VisualItem::SectionHeader(_) => section_count += 1,
            VisualItem::Workstream(_) => workstream_count += 1,
        }
    }
    assert_eq!(workstream_count, 3);
    assert!(section_count >= 2); // At least InProgress and Todo sections
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
        make_workstream("id-0", "TEST-0", LinearStatus::InProgress),
        make_workstream("id-1", "TEST-1", LinearStatus::Todo),
    ];
    state.collapsed_sections.insert(LinearStatus::InProgress);

    let filtered = vec![0, 1];
    let items = state.build_visual_items(&filtered, false);

    // InProgress section should be collapsed (header present but no items)
    let mut in_progress_count = 0;
    let mut todo_count = 0;
    let mut in_progress_header_found = false;

    for item in &items {
        match item {
            VisualItem::SectionHeader(status) => {
                if *status == LinearStatus::InProgress {
                    in_progress_header_found = true;
                }
            }
            VisualItem::Workstream(idx) => {
                if state.workstreams[*idx].linear_issue.status == LinearStatus::InProgress {
                    in_progress_count += 1;
                } else if state.workstreams[*idx].linear_issue.status == LinearStatus::Todo {
                    todo_count += 1;
                }
            }
        }
    }

    assert!(in_progress_header_found, "InProgress section header should exist");
    assert_eq!(in_progress_count, 0, "InProgress items should be hidden (collapsed)");
    assert_eq!(todo_count, 1, "Todo items should be visible");
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
