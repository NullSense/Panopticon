//! Tests for agent session caching
//!
//! The agent cache pre-loads all Claude and OpenClaw sessions once per refresh cycle
//! and provides O(1) lookup by working directory.

use panopticon::data::{AgentSession, AgentStatus, AgentType};
use panopticon::integrations::agent_cache::AgentSessionCache;

mod helpers {
    use super::*;
    use chrono::Utc;

    pub fn make_claude_session(id: &str, dir: &str) -> AgentSession {
        AgentSession {
            id: id.to_string(),
            agent_type: AgentType::ClaudeCode,
            status: AgentStatus::Running,
            working_directory: Some(dir.to_string()),
            git_branch: None,
            last_output: None,
            started_at: Utc::now(),
            last_activity: Utc::now(),
            window_id: None,
            activity: Default::default(),
        }
    }

    pub fn make_openclaw_session(id: &str, dir: &str) -> AgentSession {
        AgentSession {
            id: id.to_string(),
            agent_type: AgentType::OpenClaw,
            status: AgentStatus::Idle,
            working_directory: Some(dir.to_string()),
            git_branch: None,
            last_output: None,
            started_at: Utc::now(),
            last_activity: Utc::now(),
            window_id: None,
            activity: Default::default(),
        }
    }
}

#[test]
fn test_cache_from_sessions_empty() {
    let cache = AgentSessionCache::from_sessions(vec![], vec![]);
    assert!(cache
        .find_for_directory(Some("/home/user/project"))
        .is_none());
}

#[test]
fn test_cache_find_claude_session() {
    let claude_sessions = vec![
        helpers::make_claude_session("session-1", "/home/user/project-a"),
        helpers::make_claude_session("session-2", "/home/user/project-b"),
    ];

    let cache = AgentSessionCache::from_sessions(claude_sessions, vec![]);

    let found = cache.find_for_directory(Some("/home/user/project-a"));
    assert!(found.is_some());
    let session = found.unwrap();
    assert_eq!(session.id, "session-1");
    assert!(matches!(session.agent_type, AgentType::ClaudeCode));
}

#[test]
fn test_cache_find_openclaw_session() {
    let openclaw_sessions = vec![helpers::make_openclaw_session(
        "openclaw-1",
        "/home/user/project-c",
    )];

    let cache = AgentSessionCache::from_sessions(vec![], openclaw_sessions);

    let found = cache.find_for_directory(Some("/home/user/project-c"));
    assert!(found.is_some());
    let session = found.unwrap();
    assert_eq!(session.id, "openclaw-1");
    assert!(matches!(session.agent_type, AgentType::OpenClaw));
}

#[test]
fn test_cache_claude_takes_precedence_over_openclaw() {
    // When both Claude and OpenClaw have sessions for same directory,
    // Claude should take precedence
    let claude_sessions = vec![helpers::make_claude_session(
        "claude-1",
        "/home/user/shared-project",
    )];
    let openclaw_sessions = vec![helpers::make_openclaw_session(
        "openclaw-1",
        "/home/user/shared-project",
    )];

    let cache = AgentSessionCache::from_sessions(claude_sessions, openclaw_sessions);

    let found = cache.find_for_directory(Some("/home/user/shared-project"));
    assert!(found.is_some());
    let session = found.unwrap();
    assert_eq!(session.id, "claude-1");
    assert!(matches!(session.agent_type, AgentType::ClaudeCode));
}

#[test]
fn test_cache_find_returns_none_for_missing_directory() {
    let claude_sessions = vec![helpers::make_claude_session(
        "session-1",
        "/home/user/project-a",
    )];

    let cache = AgentSessionCache::from_sessions(claude_sessions, vec![]);

    assert!(cache
        .find_for_directory(Some("/home/user/unknown"))
        .is_none());
}

#[test]
fn test_cache_find_with_none_directory() {
    let cache = AgentSessionCache::from_sessions(
        vec![helpers::make_claude_session(
            "session-1",
            "/home/user/project",
        )],
        vec![],
    );

    assert!(cache.find_for_directory(None).is_none());
}

#[test]
fn test_cache_multiple_sessions_lookup_is_o1() {
    // Create many sessions to verify O(1) lookup behavior
    let claude_sessions: Vec<_> = (0..100)
        .map(|i| {
            helpers::make_claude_session(&format!("session-{}", i), &format!("/project/{}", i))
        })
        .collect();

    let cache = AgentSessionCache::from_sessions(claude_sessions, vec![]);

    // All lookups should be O(1) - HashMap-based
    for i in 0..100 {
        let found = cache.find_for_directory(Some(&format!("/project/{}", i)));
        assert!(found.is_some(), "Session {} should be found", i);
        assert_eq!(found.unwrap().id, format!("session-{}", i));
    }
}

#[test]
fn test_cache_sessions_without_directory_are_ignored() {
    let session_without_dir = AgentSession {
        id: "orphan".to_string(),
        agent_type: AgentType::ClaudeCode,
        status: AgentStatus::Running,
        working_directory: None, // No directory
        git_branch: None,
        last_output: None,
        started_at: chrono::Utc::now(),
        last_activity: chrono::Utc::now(),
        window_id: None,
        activity: Default::default(),
    };

    let cache = AgentSessionCache::from_sessions(vec![session_without_dir], vec![]);

    // Can't look up by directory if session has none
    assert!(cache.find_for_directory(Some("/any/path")).is_none());
}

#[test]
fn test_cache_clones_session_on_lookup() {
    // Ensure we get owned copies, not references
    let claude_sessions = vec![helpers::make_claude_session(
        "session-1",
        "/home/user/project",
    )];

    let cache = AgentSessionCache::from_sessions(claude_sessions, vec![]);

    let found1 = cache.find_for_directory(Some("/home/user/project"));
    let found2 = cache.find_for_directory(Some("/home/user/project"));

    assert!(found1.is_some());
    assert!(found2.is_some());
    assert_eq!(found1.unwrap().id, found2.unwrap().id);
}
