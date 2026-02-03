//! Session merging logic for combining multiple agent sources
//!
//! Pure functions for merging Claude and OpenClaw sessions with deduplication.
//! Claude sessions are kept first; duplicates are removed by session ID.

use crate::data::AgentSession;
use std::collections::HashSet;

/// Merge sessions from Claude and OpenClaw sources.
///
/// Claude sessions are listed first, followed by OpenClaw sessions.
/// Duplicate session IDs are removed to avoid accidental repeats.
///
/// # Arguments
/// * `claude_sessions` - Sessions from Claude Code watcher
/// * `openclaw_sessions` - Sessions from OpenClaw watcher
///
/// # Returns
/// Combined list with Claude-first ordering and ID deduplication
pub fn merge_sessions(
    claude_sessions: Vec<AgentSession>,
    openclaw_sessions: Vec<AgentSession>,
) -> Vec<AgentSession> {
    let mut result = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    // Add all Claude sessions first
    for session in claude_sessions {
        if seen_ids.insert(session.id.clone()) {
            result.push(session);
        }
    }

    // Add OpenClaw sessions unless the ID already exists
    for session in openclaw_sessions {
        if seen_ids.insert(session.id.clone()) {
            result.push(session);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{AgentStatus, AgentType};
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
    fn empty_inputs_return_empty() {
        let result = merge_sessions(vec![], vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn claude_only() {
        let claude = vec![make_session("c1", Some("main"), AgentType::ClaudeCode)];
        let result = merge_sessions(claude, vec![]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn openclaw_only() {
        let openclaw = vec![make_session("o1", Some("main"), AgentType::OpenClaw)];
        let result = merge_sessions(vec![], openclaw);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn keeps_both_when_ids_differ() {
        let claude = vec![make_session("c1", Some("main"), AgentType::ClaudeCode)];
        let openclaw = vec![make_session("o1", Some("main"), AgentType::OpenClaw)];

        let result = merge_sessions(claude, openclaw);

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|s| s.id == "c1"));
        assert!(result.iter().any(|s| s.id == "o1"));
    }

    #[test]
    fn preserves_order() {
        let claude = vec![
            make_session("c1", Some("a"), AgentType::ClaudeCode),
            make_session("c2", Some("b"), AgentType::ClaudeCode),
        ];
        let openclaw = vec![make_session("o1", Some("c"), AgentType::OpenClaw)];

        let result = merge_sessions(claude, openclaw);

        assert_eq!(result[0].id, "c1");
        assert_eq!(result[1].id, "c2");
        assert_eq!(result[2].id, "o1");
    }
}
