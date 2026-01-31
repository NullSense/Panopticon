//! Input dispatch layer for Elm Architecture (TEA) pattern.
//!
//! Maps key events to messages based on current app mode.
//! Handles key chords (gg, d1-d9, c1-c9) with non-blocking state machine.
//!
//! # Keybinding Conventions
//!
//! - **Esc and q**: Always close/go back in ALL modal views (except text input modes)
//! - **Text input modes** (search, modal search): Only Esc works, q is text input
//! - **Normal mode**: q quits the application
//!
//! When adding new modals, always include `KeyCode::Esc | KeyCode::Char('q')` for close.

use crossterm::event::KeyCode;
use std::time::Instant;

// Re-export dispatch functions from keybindings module
pub use super::keybindings::{dispatch, handle_chord_timeout};

/// State machine for handling key chords (gg, d1-d9, c1-c9).
///
/// Instead of blocking with `event::poll()` inline, we track pending keys
/// and check for timeout in the main event loop.
#[derive(Debug, Default)]
pub struct InputState {
    /// The first key of a potential chord sequence
    pub pending: Option<KeyCode>,
    /// When the pending key was pressed (for timeout detection)
    pub pending_since: Option<Instant>,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there's a pending chord that has timed out (500ms).
    pub fn has_timed_out(&self) -> bool {
        if let Some(since) = self.pending_since {
            since.elapsed().as_millis() > 500
        } else {
            false
        }
    }

    /// Clear the pending chord state.
    pub fn clear(&mut self) {
        self.pending = None;
        self.pending_since = None;
    }

    /// Set a pending chord key.
    pub fn set_pending(&mut self, key: KeyCode) {
        self.pending = Some(key);
        self.pending_since = Some(Instant::now());
    }
}
