//! Sorting logic for Linear children and related data.
//!
//! This module provides the single source of truth for sorting child issues,
//! eliminating duplication between app.rs and ui.rs.

use super::{LinearChildRef, SortMode};

/// Sort a list of child issue references by the given sort mode.
///
/// Note: Children don't have agent/vercel/PR fields, so those modes
/// fall back to sorting by Linear status.
pub fn sort_children(
    mut children: Vec<&LinearChildRef>,
    sort_mode: SortMode,
) -> Vec<&LinearChildRef> {
    match sort_mode {
        SortMode::ByLinearStatus => {
            children.sort_by_key(|c| c.status.sort_order());
        }
        SortMode::ByPriority => {
            children.sort_by_key(|c| c.priority.sort_order());
        }
        // Children don't have these fields, sort by status as fallback
        SortMode::ByAgentStatus
        | SortMode::ByVercelStatus
        | SortMode::ByPRActivity
        | SortMode::ByLastUpdated => {
            children.sort_by_key(|c| c.status.sort_order());
        }
    }
    children
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{LinearPriority, LinearStatus};

    fn make_child(id: &str, status: LinearStatus, priority: LinearPriority) -> LinearChildRef {
        LinearChildRef {
            id: id.to_string(),
            identifier: format!("TEST-{}", id),
            title: format!("Test issue {}", id),
            url: format!("https://linear.app/test/{}", id),
            status,
            priority,
        }
    }

    #[test]
    fn test_sort_by_status() {
        let c1 = make_child("1", LinearStatus::Done, LinearPriority::NoPriority);
        let c2 = make_child("2", LinearStatus::InProgress, LinearPriority::NoPriority);
        let c3 = make_child("3", LinearStatus::Todo, LinearPriority::NoPriority);

        let children = vec![&c1, &c2, &c3];
        let sorted = sort_children(children, SortMode::ByLinearStatus);

        // InProgress comes before Todo comes before Done
        assert_eq!(sorted[0].id, "2"); // InProgress
        assert_eq!(sorted[1].id, "3"); // Todo
        assert_eq!(sorted[2].id, "1"); // Done
    }

    #[test]
    fn test_sort_by_priority() {
        let c1 = make_child("1", LinearStatus::Todo, LinearPriority::Low);
        let c2 = make_child("2", LinearStatus::Todo, LinearPriority::Urgent);
        let c3 = make_child("3", LinearStatus::Todo, LinearPriority::Medium);

        let children = vec![&c1, &c2, &c3];
        let sorted = sort_children(children, SortMode::ByPriority);

        // Urgent comes before Medium comes before Low
        assert_eq!(sorted[0].id, "2"); // Urgent
        assert_eq!(sorted[1].id, "3"); // Medium
        assert_eq!(sorted[2].id, "1"); // Low
    }

    #[test]
    fn test_fallback_modes_use_status() {
        let c1 = make_child("1", LinearStatus::Done, LinearPriority::Urgent);
        let c2 = make_child("2", LinearStatus::InProgress, LinearPriority::Low);

        let children = vec![&c1, &c2];

        // All these modes should fall back to status sorting
        for mode in [
            SortMode::ByAgentStatus,
            SortMode::ByVercelStatus,
            SortMode::ByPRActivity,
            SortMode::ByLastUpdated,
        ] {
            let sorted = sort_children(children.clone(), mode);
            assert_eq!(sorted[0].id, "2"); // InProgress
            assert_eq!(sorted[1].id, "1"); // Done
        }
    }
}
