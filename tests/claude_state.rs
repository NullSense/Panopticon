//! Tests for Claude state management
//!
//! Tests the stop event handling to verify sessions transition to "done" status correctly.

use panopticon::integrations::claude::state::{
    sessions_from_state, ClaudeSessionState, ClaudeState,
};
use std::collections::HashMap;

/// Helper to set up test environment with custom state directory
/// Note: These tests use the real state file, so we verify behavior rather than isolation
mod state_behavior {
    use super::*;

    #[test]
    fn test_sessions_from_state_maps_done_status() {
        let mut sessions = HashMap::new();
        sessions.insert(
            "test-session-1".to_string(),
            ClaudeSessionState {
                path: "/home/user/project".to_string(),
                git_branch: None,
                status: "done".to_string(),
                last_active: chrono::Utc::now().timestamp(),
                activity: Default::default(),
            },
        );

        let state = ClaudeState { sessions };
        let agent_sessions = sessions_from_state(&state);

        assert_eq!(agent_sessions.len(), 1);
        assert!(matches!(
            agent_sessions[0].status,
            panopticon::data::AgentStatus::Done
        ));
    }

    #[test]
    fn test_sessions_from_state_maps_running_status() {
        let mut sessions = HashMap::new();
        sessions.insert(
            "test-session-2".to_string(),
            ClaudeSessionState {
                path: "/home/user/project".to_string(),
                git_branch: None,
                status: "running".to_string(),
                last_active: chrono::Utc::now().timestamp(),
                activity: Default::default(),
            },
        );

        let state = ClaudeState { sessions };
        let agent_sessions = sessions_from_state(&state);

        assert_eq!(agent_sessions.len(), 1);
        assert!(matches!(
            agent_sessions[0].status,
            panopticon::data::AgentStatus::Running
        ));
    }

    #[test]
    fn test_sessions_from_state_handles_stop_as_done() {
        // The status "stop" in state file should map to AgentStatus::Done
        // This tests the defensive mapping in sessions_from_state
        let mut sessions = HashMap::new();
        sessions.insert(
            "test-session-3".to_string(),
            ClaudeSessionState {
                path: "/home/user/project".to_string(),
                git_branch: None,
                status: "stop".to_string(), // If "stop" somehow gets stored
                last_active: chrono::Utc::now().timestamp(),
                activity: Default::default(),
            },
        );

        let state = ClaudeState { sessions };
        let agent_sessions = sessions_from_state(&state);

        assert_eq!(agent_sessions.len(), 1);
        // "stop" should map to Done per line 128 in state.rs
        assert!(matches!(
            agent_sessions[0].status,
            panopticon::data::AgentStatus::Done
        ));
    }

    #[test]
    fn test_sessions_from_state_all_status_mappings() {
        let statuses = vec![
            ("running", panopticon::data::AgentStatus::Running),
            ("start", panopticon::data::AgentStatus::Running),
            ("active", panopticon::data::AgentStatus::Running),
            ("idle", panopticon::data::AgentStatus::Idle),
            ("waiting", panopticon::data::AgentStatus::WaitingForInput),
            ("done", panopticon::data::AgentStatus::Done),
            ("stop", panopticon::data::AgentStatus::Done),
            ("unknown", panopticon::data::AgentStatus::Idle), // Default fallback
        ];

        for (status_str, expected_status) in statuses {
            let mut sessions = HashMap::new();
            sessions.insert(
                format!("session-{}", status_str),
                ClaudeSessionState {
                    path: "/project".to_string(),
                    git_branch: None,
                    status: status_str.to_string(),
                    last_active: chrono::Utc::now().timestamp(),
                    activity: Default::default(),
                },
            );

            let state = ClaudeState { sessions };
            let agent_sessions = sessions_from_state(&state);

            assert_eq!(agent_sessions.len(), 1);
            assert_eq!(
                agent_sessions[0].status, expected_status,
                "Status '{}' should map to {:?}",
                status_str, expected_status
            );
        }
    }
}
