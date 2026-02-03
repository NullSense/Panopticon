//! Agent session caching for efficient lookups during refresh
//!
//! The `AgentSessionCache` pre-loads all Claude and OpenClaw sessions once per refresh
//! cycle and provides O(1) lookup by git branch. This dramatically reduces I/O:
//! - Before: 100 issues = 100 file reads per issue
//! - After: 100 issues = 1 file read for Claude + 1 directory scan for OpenClaw

use crate::data::{AgentSession, AgentStatus, AgentType};
use once_cell::sync::Lazy;
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::Path;

static ISSUE_ID_RE: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"(?i)([A-Z]{2,5}-\d+)").unwrap());

/// Pre-loaded agent session cache for a single refresh cycle.
///
/// This cache is created once at the start of each refresh and provides O(1)
/// lookup of sessions by git branch or issue identifier.
#[derive(Debug, Default)]
pub struct AgentSessionCache {
    /// Map from git_branch -> sessions
    by_branch: HashMap<String, Vec<AgentSession>>,
    /// Map from issue identifier (e.g., "DRE-380") -> sessions
    /// Extracted from branch names like "feat/dre-380-unified-orchestration"
    by_identifier: HashMap<String, Vec<AgentSession>>,
    /// All sessions (for showing unlinked sessions)
    all_sessions: Vec<AgentSession>,
}

/// Extract issue identifier from a git branch name.
///
/// Supports common branch naming conventions:
/// - `feat/DRE-380-unified-orchestration` -> `DRE-380`
/// - `fix/dre-123-bug-fix` -> `DRE-123`
/// - `DRE-456-feature` -> `DRE-456`
///
/// Returns uppercase identifier for case-insensitive matching.
fn extract_issue_id(branch: &str) -> Option<String> {
    ISSUE_ID_RE
        .captures(branch)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_uppercase())
}

fn repo_name_from_hint(repo: &str) -> &str {
    repo.rsplit('/').next().unwrap_or(repo)
}

fn matches_repo_hint(session: &AgentSession, repo_name: &str) -> bool {
    session
        .working_directory
        .as_deref()
        .and_then(|dir| Path::new(dir).file_name())
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == repo_name)
}

fn agent_status_rank(status: AgentStatus) -> u8 {
    match status {
        AgentStatus::WaitingForInput => 0,
        AgentStatus::Error => 1,
        AgentStatus::Running => 2,
        AgentStatus::Idle => 3,
        AgentStatus::Done => 4,
    }
}

fn agent_type_rank(agent_type: AgentType) -> u8 {
    match agent_type {
        AgentType::ClaudeCode => 0,
        AgentType::OpenClaw => 1,
    }
}

fn select_best_session(sessions: &[AgentSession], repo_hint: Option<&str>) -> Option<AgentSession> {
    if sessions.is_empty() {
        return None;
    }

    let candidates: Vec<&AgentSession> = if let Some(repo_hint) = repo_hint {
        let repo_name = repo_name_from_hint(repo_hint);
        let filtered: Vec<&AgentSession> = sessions
            .iter()
            .filter(|s| matches_repo_hint(s, repo_name))
            .collect();
        if filtered.is_empty() {
            sessions.iter().collect()
        } else {
            filtered
        }
    } else {
        sessions.iter().collect()
    };

    candidates
        .into_iter()
        .min_by_key(|s| {
            (
                agent_status_rank(s.status),
                Reverse(s.last_activity.timestamp()),
                agent_type_rank(s.agent_type),
            )
        })
        .cloned()
}

fn branch_is_ambiguous_without_repo_hint(sessions: &[AgentSession]) -> bool {
    if sessions.len() <= 1 {
        return false;
    }
    let mut repo_names: HashSet<String> = HashSet::new();
    let mut none_count = 0;
    for session in sessions {
        if let Some(dir) = session.working_directory.as_deref() {
            let repo_name = Path::new(dir)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string());
            if let Some(repo_name) = repo_name {
                repo_names.insert(repo_name);
            }
        } else {
            none_count += 1;
        }
    }
    if repo_names.len() > 1 {
        return true;
    }
    repo_names.is_empty() && none_count > 1
}

impl AgentSessionCache {
    /// Create a new cache from pre-fetched session lists.
    ///
    /// Claude sessions are inserted first, then OpenClaw sessions are added.
    /// Duplicate session IDs are ignored.
    pub fn from_sessions(
        claude_sessions: Vec<AgentSession>,
        openclaw_sessions: Vec<AgentSession>,
    ) -> Self {
        let mut by_branch: HashMap<String, Vec<AgentSession>> = HashMap::new();
        let mut by_identifier: HashMap<String, Vec<AgentSession>> = HashMap::new();
        let mut all_sessions = Vec::new();
        let mut seen_ids: HashSet<String> = HashSet::new();

        // Insert Claude sessions first (they take precedence)
        for session in claude_sessions {
            if !seen_ids.insert(session.id.clone()) {
                continue;
            }
            all_sessions.push(session.clone());
            if let Some(branch) = &session.git_branch {
                by_branch
                    .entry(branch.clone())
                    .or_default()
                    .push(session.clone());
                // Also index by issue identifier extracted from branch name
                if let Some(id) = extract_issue_id(branch) {
                    by_identifier.entry(id).or_default().push(session);
                }
            }
        }

        // Insert OpenClaw sessions
        for session in openclaw_sessions {
            if !seen_ids.insert(session.id.clone()) {
                continue;
            }
            all_sessions.push(session.clone());
            if let Some(branch) = &session.git_branch {
                by_branch
                    .entry(branch.clone())
                    .or_default()
                    .push(session.clone());
                // Also index by identifier
                if let Some(id) = extract_issue_id(branch) {
                    by_identifier.entry(id).or_default().push(session);
                }
            }
        }

        Self {
            by_branch,
            by_identifier,
            all_sessions,
        }
    }

    /// Load all sessions from Claude and OpenClaw sources.
    ///
    /// This performs exactly:
    /// - 1 file read for Claude sessions (~/.local/share/panopticon/claude_state.json)
    /// - 1 directory scan for OpenClaw sessions (~/.openclaw/agents/*/sessions/)
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

        // Load OpenClaw sessions (single HTTP call)
        let openclaw_sessions = match super::openclaw::find_all_sessions().await {
            Ok(sessions) => sessions,
            Err(e) => {
                tracing::debug!("Failed to load OpenClaw sessions: {}", e);
                vec![]
            }
        };

        Self::from_sessions(claude_sessions, openclaw_sessions)
    }

    /// Find an agent session for a git branch.
    ///
    /// Returns `None` if:
    /// - The branch is `None`
    /// - No session exists for the given branch
    ///
    /// This is an O(1) HashMap lookup.
    pub fn find_for_branch(&self, branch: Option<&str>) -> Option<AgentSession> {
        let sessions = branch.and_then(|b| self.by_branch.get(b));
        if let Some(sessions) = sessions {
            if branch_is_ambiguous_without_repo_hint(sessions) {
                return None;
            }
            return select_best_session(sessions, None);
        }
        None
    }

    /// Find an agent session by issue identifier (e.g., "DRE-380").
    ///
    /// This matches sessions whose git branch contains the issue ID.
    /// Case-insensitive matching.
    pub fn find_for_identifier(&self, identifier: &str) -> Option<AgentSession> {
        let key = identifier.to_uppercase();
        self.by_identifier
            .get(&key)
            .and_then(|s| select_best_session(s, None))
    }

    /// Find all agent sessions by branch and/or identifier.
    ///
    /// Identifier matches are included first, then branch matches are unioned.
    /// Results are filtered by repo hint when provided. Without a repo hint,
    /// ambiguous branch matches are dropped to avoid mis-linking.
    pub fn find_all_for_branch_or_identifier(
        &self,
        branch: Option<&str>,
        identifier: &str,
        repo_hint: Option<&str>,
    ) -> Vec<AgentSession> {
        let mut results: Vec<AgentSession> = Vec::new();
        let mut seen_ids: HashSet<String> = HashSet::new();
        let key = identifier.to_uppercase();

        if let Some(sessions) = self.by_identifier.get(&key) {
            if repo_hint.is_some() && !sessions.is_empty() {
                let repo_name = repo_name_from_hint(repo_hint.unwrap_or_default());
                let filtered: Vec<AgentSession> = sessions
                    .iter()
                    .filter(|s| matches_repo_hint(s, repo_name))
                    .cloned()
                    .collect();
                for session in filtered {
                    if seen_ids.insert(session.id.clone()) {
                        results.push(session);
                    }
                }
            } else if !branch_is_ambiguous_without_repo_hint(sessions) {
                for session in sessions {
                    if seen_ids.insert(session.id.clone()) {
                        results.push(session.clone());
                    }
                }
            }
        }

        if let Some(branch) = branch {
            if let Some(sessions) = self.by_branch.get(branch) {
                if repo_hint.is_some() && !sessions.is_empty() {
                    let repo_name = repo_name_from_hint(repo_hint.unwrap_or_default());
                    for session in sessions {
                        if matches_repo_hint(session, repo_name)
                            && seen_ids.insert(session.id.clone())
                        {
                            results.push(session.clone());
                        }
                    }
                } else if !branch_is_ambiguous_without_repo_hint(sessions) {
                    for session in sessions {
                        if seen_ids.insert(session.id.clone()) {
                            results.push(session.clone());
                        }
                    }
                }
            }
        }

        results
    }

    /// Find an agent session by branch OR identifier (fallback).
    ///
    /// Tries identifier match first, then falls back to branch match.
    /// This handles cases where Linear's branchName doesn't match the git branch.
    pub fn find_for_branch_or_identifier(
        &self,
        branch: Option<&str>,
        identifier: &str,
        repo_hint: Option<&str>,
    ) -> Option<AgentSession> {
        let sessions = self.find_all_for_branch_or_identifier(branch, identifier, repo_hint);
        select_best_session(&sessions, repo_hint)
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

    fn make_session(
        id: &str,
        dir: &str,
        branch: Option<&str>,
        agent_type: AgentType,
    ) -> AgentSession {
        AgentSession {
            id: id.to_string(),
            agent_type,
            status: AgentStatus::Running,
            working_directory: Some(dir.to_string()),
            git_branch: branch.map(|s| s.to_string()),
            last_output: None,
            started_at: Utc::now(),
            last_activity: Utc::now(),
            window_id: None,
            activity: Default::default(),
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
        let claude = vec![make_session(
            "claude-1",
            "/project",
            Some("main"),
            AgentType::ClaudeCode,
        )];
        let mut openclaw = vec![make_session(
            "openclaw-1",
            "/project",
            Some("main"),
            AgentType::OpenClaw,
        )];
        openclaw[0].status = AgentStatus::Idle;

        let cache = AgentSessionCache::from_sessions(claude, openclaw);

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

    #[test]
    fn test_extract_issue_id() {
        // Common branch naming patterns
        assert_eq!(
            extract_issue_id("feat/DRE-380-unified-orchestration"),
            Some("DRE-380".to_string())
        );
        assert_eq!(
            extract_issue_id("fix/dre-123-bug-fix"),
            Some("DRE-123".to_string())
        );
        assert_eq!(
            extract_issue_id("DRE-456-feature"),
            Some("DRE-456".to_string())
        );
        assert_eq!(extract_issue_id("ABC-789"), Some("ABC-789".to_string()));
        // No match
        assert_eq!(extract_issue_id("main"), None);
        assert_eq!(extract_issue_id("feature-branch"), None);
    }

    #[test]
    fn test_find_by_identifier() {
        let sessions = vec![
            make_session(
                "s1",
                "/project",
                Some("feat/dre-380-unified-orchestration"),
                AgentType::ClaudeCode,
            ),
            make_session(
                "s2",
                "/project-b",
                Some("fix/abc-123-bug"),
                AgentType::ClaudeCode,
            ),
        ];

        let cache = AgentSessionCache::from_sessions(sessions, vec![]);

        // Should find by identifier (case-insensitive)
        let found = cache.find_for_identifier("DRE-380").unwrap();
        assert_eq!(found.id, "s1");

        let found = cache.find_for_identifier("dre-380").unwrap();
        assert_eq!(found.id, "s1");

        let found = cache.find_for_identifier("ABC-123").unwrap();
        assert_eq!(found.id, "s2");

        // Should not find non-existent identifier
        assert!(cache.find_for_identifier("XYZ-999").is_none());
    }

    #[test]
    fn test_find_for_branch_or_identifier() {
        let sessions = vec![make_session(
            "s1",
            "/project",
            Some("feat/dre-380-unified-orchestration"),
            AgentType::ClaudeCode,
        )];

        let cache = AgentSessionCache::from_sessions(sessions, vec![]);

        // Identifier and branch both point to the same session
        let found = cache
            .find_for_branch_or_identifier(
                Some("feat/dre-380-unified-orchestration"),
                "DRE-380",
                None,
            )
            .unwrap();
        assert_eq!(found.id, "s1");

        // Falls back to identifier if branch doesn't match
        let found = cache
            .find_for_branch_or_identifier(None, "DRE-380", None)
            .unwrap();
        assert_eq!(found.id, "s1");

        // Falls back to identifier if branch is different
        let found = cache
            .find_for_branch_or_identifier(Some("nonexistent-branch"), "DRE-380", None)
            .unwrap();
        assert_eq!(found.id, "s1");

        // Returns None if neither matches
        assert!(cache
            .find_for_branch_or_identifier(Some("other"), "XYZ-999", None)
            .is_none());
    }
}
