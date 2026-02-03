//! Unified agent watcher combining Claude and OpenClaw sources
//!
//! Provides a single interface for monitoring all agent sessions with
//! automatic deduplication and precedence handling.

use super::merger::merge_sessions;
use crate::data::AgentSession;
use crate::integrations::claude::watcher::ClaudeWatcher;
use crate::integrations::openclaw::watcher::OpenClawWatcher;
use anyhow::Result;
use std::sync::{Arc, RwLock};

/// Unified watcher that monitors both Claude and OpenClaw sessions
pub struct UnifiedAgentWatcher {
    claude_watcher: Option<ClaudeWatcher>,
    openclaw_watcher: Option<OpenClawWatcher>,
    sessions: Arc<RwLock<Vec<AgentSession>>>,
}

impl UnifiedAgentWatcher {
    /// Create a new unified watcher
    pub fn new() -> Result<Self> {
        let claude_watcher = ClaudeWatcher::new().ok();
        let openclaw_watcher = OpenClawWatcher::new().ok();

        let sessions = Arc::new(RwLock::new(Vec::new()));

        let watcher = Self {
            claude_watcher,
            openclaw_watcher,
            sessions,
        };

        // Initial merge
        watcher.refresh_sessions();

        Ok(watcher)
    }

    /// Poll for changes from both watchers
    ///
    /// Returns true if any sessions changed
    pub fn poll(&self) -> bool {
        let claude_changed = self
            .claude_watcher
            .as_ref()
            .map(|w| w.poll())
            .unwrap_or(false);

        let openclaw_changed = self
            .openclaw_watcher
            .as_ref()
            .map(|w| w.poll())
            .unwrap_or(false);

        if claude_changed || openclaw_changed {
            self.refresh_sessions();
            return true;
        }

        false
    }

    /// Refresh the merged session list
    fn refresh_sessions(&self) {
        let claude_sessions = self
            .claude_watcher
            .as_ref()
            .map(|w| w.get_sessions_snapshot())
            .unwrap_or_default();

        let openclaw_sessions = self
            .openclaw_watcher
            .as_ref()
            .map(|w| w.get_sessions_snapshot())
            .unwrap_or_default();

        let merged = merge_sessions(claude_sessions, openclaw_sessions);

        match self.sessions.write() {
            Ok(mut guard) => *guard = merged,
            Err(e) => tracing::warn!("Unified sessions lock poisoned: {e}"),
        }
    }

    /// Get a snapshot of all merged sessions
    pub fn get_sessions_snapshot(&self) -> Vec<AgentSession> {
        self.sessions.read().map(|g| g.clone()).unwrap_or_default()
    }

    /// Check if any watchers are active
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.claude_watcher.is_some() || self.openclaw_watcher.is_some()
    }
}

impl Default for UnifiedAgentWatcher {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            claude_watcher: None,
            openclaw_watcher: None,
            sessions: Arc::new(RwLock::new(Vec::new())),
        })
    }
}
