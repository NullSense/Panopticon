//! Agent session caching for efficient lookups during refresh
//!
//! The `AgentSessionCache` pre-loads all Claude and Moltbot sessions once per refresh
//! cycle and provides O(1) lookup by working directory. This dramatically reduces I/O:
//! - Before: 100 issues = 100 file reads + 100 HTTP calls
//! - After: 100 issues = 1 file read + 1 HTTP call

use crate::data::AgentSession;
use std::collections::HashMap;

/// Pre-loaded agent session cache for a single refresh cycle.
///
/// This cache is created once at the start of each refresh and provides O(1)
/// lookup of sessions by working directory. Claude sessions take precedence
/// over Moltbot sessions when both exist for the same directory.
#[derive(Debug, Default)]
pub struct AgentSessionCache {
    /// Map from working_directory -> AgentSession
    by_directory: HashMap<String, AgentSession>,
}

impl AgentSessionCache {
    /// Create a new cache from pre-fetched session lists.
    ///
    /// Claude sessions are inserted first, then Moltbot sessions are added
    /// only if there's no existing entry for that directory. This ensures
    /// Claude takes precedence.
    pub fn from_sessions(
        claude_sessions: Vec<AgentSession>,
        moltbot_sessions: Vec<AgentSession>,
    ) -> Self {
        let mut by_directory = HashMap::new();

        // Insert Claude sessions first (they take precedence)
        for session in claude_sessions {
            if let Some(dir) = &session.working_directory {
                by_directory.insert(dir.clone(), session);
            }
        }

        // Insert Moltbot sessions only if directory not already mapped
        for session in moltbot_sessions {
            if let Some(dir) = &session.working_directory {
                by_directory.entry(dir.clone()).or_insert(session);
            }
        }

        Self { by_directory }
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

    /// Find an agent session for a working directory.
    ///
    /// Returns `None` if:
    /// - The directory is `None`
    /// - No session exists for the given directory
    ///
    /// This is an O(1) HashMap lookup.
    pub fn find_for_directory(&self, dir: Option<&str>) -> Option<AgentSession> {
        dir.and_then(|d| self.by_directory.get(d).cloned())
    }

    /// Get the number of cached sessions.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.by_directory.len()
    }

    /// Check if the cache is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.by_directory.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{AgentStatus, AgentType};
    use chrono::Utc;

    fn make_session(id: &str, dir: &str, agent_type: AgentType) -> AgentSession {
        AgentSession {
            id: id.to_string(),
            agent_type,
            status: AgentStatus::Running,
            working_directory: Some(dir.to_string()),
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
        let claude = vec![make_session("claude-1", "/project", AgentType::ClaudeCode)];
        let moltbot = vec![make_session("moltbot-1", "/project", AgentType::Clawdbot)];

        let cache = AgentSessionCache::from_sessions(claude, moltbot);

        let found = cache.find_for_directory(Some("/project")).unwrap();
        assert_eq!(found.id, "claude-1");
    }
}
