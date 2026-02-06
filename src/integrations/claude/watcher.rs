//! File watcher for Claude state changes
//!
//! Uses the notify crate to watch claude_state.json for changes.
//! Provides real-time updates when Claude sessions start/stop.

use super::state::{read_state, sessions_from_state, state_file_path};
use crate::data::AgentSession;
use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Watcher that maintains current Claude sessions
pub struct ClaudeWatcher {
    sessions: Arc<RwLock<Vec<AgentSession>>>,
    _watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
}

impl ClaudeWatcher {
    /// Create a new watcher
    pub fn new() -> Result<Self> {
        let sessions = Arc::new(RwLock::new(Vec::new()));

        // Initial load
        if let Ok(state) = read_state() {
            match sessions.write() {
                Ok(mut guard) => *guard = sessions_from_state(&state),
                Err(e) => tracing::warn!("Claude sessions lock poisoned on init: {e}"),
            }
        }

        let (tx, rx) = channel();

        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(1)),
        )?;

        // Watch the state file
        if let Ok(path) = state_file_path() {
            // Watch the parent directory (file might not exist yet)
            if let Some(parent) = path.parent() {
                let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
            }
        }

        Ok(Self {
            sessions,
            _watcher: watcher,
            receiver: rx,
        })
    }

    /// Poll for changes and update sessions
    ///
    /// Debounces multiple events - only reads state once even if file changed multiple times.
    pub fn poll(&self) -> bool {
        let mut has_events = false;

        // Drain all pending events (debounce - only care that SOMETHING changed)
        loop {
            match self.receiver.try_recv() {
                Ok(Ok(_event)) => {
                    has_events = true;
                    // Don't read here - just mark that we have events
                }
                Ok(Err(_)) => {
                    // Watcher error, ignore
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

        // Only read state ONCE after draining all events
        if has_events {
            if let Ok(state) = read_state() {
                match self.sessions.write() {
                    Ok(mut guard) => {
                        *guard = sessions_from_state(&state);
                        return true;
                    }
                    Err(e) => tracing::warn!("Claude sessions lock poisoned on poll: {e}"),
                }
            }
        }

        false
    }

    /// Get current sessions (returns Arc to avoid cloning entire Vec)
    #[allow(dead_code)]
    pub fn get_sessions(&self) -> Arc<RwLock<Vec<AgentSession>>> {
        Arc::clone(&self.sessions)
    }

    /// Get a snapshot of current sessions (clones the data)
    pub fn get_sessions_snapshot(&self) -> Vec<AgentSession> {
        self.sessions.read().map(|g| g.clone()).unwrap_or_default()
    }
}
