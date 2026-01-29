//! Moltbot/Clawdbot integration
//!
//! Uses the local Gateway API on port 18789 to fetch active sessions.
//! No fallback to CLI - if the daemon is not running, return empty list.

pub mod client;

use crate::data::AgentSession;
use anyhow::Result;

/// Find all Moltbot sessions via Gateway API
pub async fn find_all_sessions() -> Result<Vec<AgentSession>> {
    // Query the Gateway API directly
    match client::fetch_sessions(None).await {
        Ok(sessions) => Ok(sessions),
        Err(e) => {
            // If connection refused, daemon is not running
            tracing::debug!("Moltbot API not available: {}", e);
            Ok(vec![])
        }
    }
}

/// Find a Moltbot session for a given working directory
pub async fn find_session_for_directory(dir: Option<&str>) -> Option<AgentSession> {
    let dir = dir?;
    let sessions = find_all_sessions().await.ok()?;

    sessions
        .into_iter()
        .find(|s| s.working_directory.as_deref() == Some(dir))
}
