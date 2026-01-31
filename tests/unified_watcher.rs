//! Tests for UnifiedAgentWatcher
//!
//! TDD: Tests for the unified watcher that composes Claude and OpenClaw watchers.

use panopticon::agents::merger::merge_sessions;
use panopticon::data::{AgentSession, AgentStatus, AgentType};

mod session_merger {
    use super::*;
    use chrono::Utc;

    fn make_session(id: &str, branch: Option<&str>, agent_type: AgentType) -> AgentSession {
        AgentSession {
            id: id.to_string(),
            agent_type,
            status: AgentStatus::Running,
            working_directory: Some("/project".to_string()),
            git_branch: branch.map(|s| s.to_string()),
            last_output: None,
            started_at: Utc::now(),
            last_activity: Utc::now(),
            window_id: None,
            activity: Default::default(),
        }
    }

    #[test]
    fn merges_empty_lists() {
        let merged = merge_sessions(vec![], vec![]);
        assert!(merged.is_empty());
    }

    #[test]
    fn includes_claude_sessions() {
        let claude = vec![make_session("c1", Some("main"), AgentType::ClaudeCode)];
        let merged = merge_sessions(claude, vec![]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "c1");
    }

    #[test]
    fn includes_openclaw_sessions() {
        let openclaw = vec![make_session("o1", Some("main"), AgentType::OpenClaw)];
        let merged = merge_sessions(vec![], openclaw);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "o1");
    }

    #[test]
    fn claude_takes_precedence_for_same_branch() {
        let claude = vec![make_session("c1", Some("main"), AgentType::ClaudeCode)];
        let openclaw = vec![make_session("o1", Some("main"), AgentType::OpenClaw)];

        let merged = merge_sessions(claude, openclaw);

        // Should only have Claude session, OpenClaw deduplicated
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "c1");
        assert!(matches!(merged[0].agent_type, AgentType::ClaudeCode));
    }

    #[test]
    fn keeps_both_for_different_branches() {
        let claude = vec![make_session("c1", Some("feat-a"), AgentType::ClaudeCode)];
        let openclaw = vec![make_session("o1", Some("feat-b"), AgentType::OpenClaw)];

        let merged = merge_sessions(claude, openclaw);

        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn keeps_sessions_without_branches() {
        let claude = vec![make_session("c1", None, AgentType::ClaudeCode)];
        let openclaw = vec![make_session("o1", None, AgentType::OpenClaw)];

        let merged = merge_sessions(claude, openclaw);

        // Both should be kept since no branch to deduplicate on
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn handles_multiple_sessions_per_source() {
        let claude = vec![
            make_session("c1", Some("main"), AgentType::ClaudeCode),
            make_session("c2", Some("feat-a"), AgentType::ClaudeCode),
        ];
        let openclaw = vec![
            make_session("o1", Some("main"), AgentType::OpenClaw),    // Dedup with c1
            make_session("o2", Some("feat-b"), AgentType::OpenClaw), // Unique
        ];

        let merged = merge_sessions(claude, openclaw);

        // c1 (main), c2 (feat-a), o2 (feat-b) = 3 sessions
        assert_eq!(merged.len(), 3);
        assert!(merged.iter().any(|s| s.id == "c1"));
        assert!(merged.iter().any(|s| s.id == "c2"));
        assert!(merged.iter().any(|s| s.id == "o2"));
        // o1 should be deduplicated
        assert!(!merged.iter().any(|s| s.id == "o1"));
    }
}
