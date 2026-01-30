//! Agent session caching for efficient lookups during refresh
//!
//! The `AgentSessionCache` pre-loads all Claude and Moltbot sessions once per refresh
//! cycle and provides O(1) lookup by git branch. This dramatically reduces I/O:
//! - Before: 100 issues = 100 file reads + 100 HTTP calls
//! - After: 100 issues = 1 file read + 1 HTTP call

use crate::data::AgentSession;
use std::collections::HashMap;

/// Pre-loaded agent session cache for a single refresh cycle.
///
/// This cache is created once at the start of each refresh and provides O(1)
/// lookup of sessions by git branch. Claude sessions take precedence
/// over Moltbot sessions when both exist for the same branch.
#[derive(Debug, Default)]
pub struct AgentSessionCache {
    /// Map from git_branch -> AgentSession
    by_branch: HashMap<String, AgentSession>,
    /// All sessions (for showing unlinked sessions)
    all_sessions: Vec<AgentSession>,
}

impl AgentSessionCache {
    /// Create a new cache from pre-fetched session lists.
    ///
    /// Claude sessions are inserted first, then Moltbot sessions are added
    /// only if there's no existing entry for that branch. This ensures
    /// Claude takes precedence.
    pub fn from_sessions(
        claude_sessions: Vec<AgentSession>,
        moltbot_sessions: Vec<AgentSession>,
    ) -> Self {
        let mut by_branch = HashMap::new();
        let mut all_sessions = Vec::new();

        // Insert Claude sessions first (they take precedence)
        for session in claude_sessions {
            all_sessions.push(session.clone());
            if let Some(branch) = &session.git_branch {
                by_branch.insert(branch.clone(), session);
            }
        }

        // Insert Moltbot sessions only if branch not already mapped
        for session in moltbot_sessions {
            all_sessions.push(session.clone());
            if let Some(branch) = &session.git_branch {
                by_branch.entry(branch.clone()).or_insert(session);
            }
        }

        Self { by_branch, all_sessions }
    }

    /// Load all sessions from Claude and Moltbot sources.
    ///
    /// This performs exactly:
    /// - 1 file read for Claude sessions (claude_state.json)
    /// - 1 HTTP call for Moltbot sessions (Gateway API)
    ///
    /// Errors in either source are logged and treated as empty lists.
    pub async fn load() -> Self {
        // Load Claude sessions (single file read)
        let claude_sessions = match super::claude::find_all_sessions().await {
            Ok(sessions) => sessions,
            Err(e) => {
                tracing::debug!("Failed to load Claude sessions: {}", e);
                vec![]
            }
        };

        // Load Moltbot sessions (single HTTP call)
        let moltbot_sessions = match super::moltbot::find_all_sessions().await {
            Ok(sessions) => sessions,
            Err(e) => {
                tracing::debug!("Failed to load Moltbot sessions: {}", e);
                vec![]
            }
        };

        Self::from_sessions(claude_sessions, moltbot_sessions)
    }

    /// Find an agent session for a git branch.
    ///
    /// Returns `None` if:
    /// - The branch is `None`
    /// - No session exists for the given branch
    ///
    /// This is an O(1) HashMap lookup.
    pub fn find_for_branch(&self, branch: Option<&str>) -> Option<AgentSession> {
        branch.and_then(|b| self.by_branch.get(b).cloned())
    }

    /// Find an agent session for a working directory (legacy).
    ///
    /// This is kept for backwards compatibility but find_for_branch should be preferred.
    #[allow(dead_code)]
    pub fn find_for_directory(&self, dir: Option<&str>) -> Option<AgentSession> {
        let dir = dir?;
        self.all_sessions
            .iter()
            .find(|s| s.working_directory.as_deref() == Some(dir))
            .cloned()
    }

    /// Get all sessions (for showing unlinked sessions)
    pub fn all_sessions(&self) -> &[AgentSession] {
        &self.all_sessions
    }

    /// Get the number of cached sessions.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.all_sessions.len()
    }

    /// Check if the cache is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.all_sessions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{AgentStatus, AgentType};
    use chrono::Utc;

    fn make_session(id: &str, dir: &str, branch: Option<&str>, agent_type: AgentType) -> AgentSession {
        AgentSession {
            id: id.to_string(),
            agent_type,
            status: AgentStatus::Running,
            working_directory: Some(dir.to_string()),
            git_branch: branch.map(|s| s.to_string()),
            last_output: None,
            started_at: Utc::now(),
            window_id: None,
        }
    }

    #[test]
    fn test_empty_cache() {
        let cache = AgentSessionCache::from_sessions(vec![], vec![]);
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_claude_precedence() {
        let claude = vec![make_session("claude-1", "/project", Some("main"), AgentType::ClaudeCode)];
        let moltbot = vec![make_session("moltbot-1", "/project", Some("main"), AgentType::Clawdbot)];

        let cache = AgentSessionCache::from_sessions(claude, moltbot);

        let found = cache.find_for_branch(Some("main")).unwrap();
        assert_eq!(found.id, "claude-1");
    }

    #[test]
    fn test_find_by_branch() {
        let sessions = vec![
            make_session("s1", "/project-a", Some("feat-auth"), AgentType::ClaudeCode),
            make_session("s2", "/project-b", Some("main"), AgentType::ClaudeCode),
        ];

        let cache = AgentSessionCache::from_sessions(sessions, vec![]);

        assert!(cache.find_for_branch(Some("feat-auth")).is_some());
        assert!(cache.find_for_branch(Some("main")).is_some());
        assert!(cache.find_for_branch(Some("nonexistent")).is_none());
    }

    #[test]
    fn test_all_sessions_includes_unlinked() {
        let sessions = vec![
            make_session("s1", "/project-a", Some("main"), AgentType::ClaudeCode),
            make_session("s2", "/project-b", None, AgentType::ClaudeCode), // No branch
        ];

        let cache = AgentSessionCache::from_sessions(sessions, vec![]);

        assert_eq!(cache.all_sessions().len(), 2);
        assert_eq!(cache.find_for_branch(Some("main")).unwrap().id, "s1");
        // s2 has no branch, can't be found by branch but is in all_sessions
        assert!(cache.all_sessions().iter().any(|s| s.id == "s2"));
    }
}
