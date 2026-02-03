//! OpenClaw integration
//!
//! Discovers OpenClaw sessions from local state files:
//! - `~/.openclaw/agents/*/sessions/sessions.json` for session metadata
//! - `~/.openclaw/agents/*/sessions/<sessionId>.jsonl` for working directory

pub mod state;
pub mod status;
pub mod watcher;

use crate::data::AgentSession;
use anyhow::Result;

/// Find all OpenClaw sessions from local state files
///
/// Reads sessions.json and transcript headers to get:
/// - Session ID and status (from updatedAt timestamp)
/// - Working directory (from transcript header)
/// - Model name (from sessions.json)
/// - Git branch (from working directory's .git/HEAD)
pub async fn find_all_sessions() -> Result<Vec<AgentSession>> {
    let state_dir = watcher::state_dir();
    let sessions = watcher::load_all_sessions(&state_dir);
    Ok(sessions)
}

/// Find an OpenClaw session for a given working directory
#[allow(dead_code)]
pub async fn find_session_for_directory(dir: Option<&str>) -> Option<AgentSession> {
    let dir = dir?;
    let sessions = find_all_sessions().await.ok()?;

    sessions
        .into_iter()
        .find(|s| s.working_directory.as_deref() == Some(dir))
}
