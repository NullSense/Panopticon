//! Tests for TUI input handling (dispatch layer).
//!
//! Tests the key-to-message mapping for different app modes.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use panopticon::tui::input::InputState;

// ============================================================================
// Test Helpers
// ============================================================================

fn key_event(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}

fn key_event_ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}

// ============================================================================
// Input State Tests
// ============================================================================
// Note: Mode-specific dispatch tests would require a full App instance.
// These tests focus on the InputState chord handling machinery.

mod input_state {
    use super::*;

    #[test]
    fn test_new_state_has_no_pending() {
        let input = InputState::new();
        assert!(input.pending.is_none());
        assert!(input.pending_since.is_none());
    }

    #[test]
    fn test_set_pending() {
        let mut input = InputState::new();
        input.set_pending(KeyCode::Char('g'));
        assert!(input.pending.is_some());
        assert!(input.pending_since.is_some());
    }

    #[test]
    fn test_clear_pending() {
        let mut input = InputState::new();
        input.set_pending(KeyCode::Char('g'));
        input.clear();
        assert!(input.pending.is_none());
        assert!(input.pending_since.is_none());
    }

    #[test]
    fn test_timeout_not_immediate() {
        let mut input = InputState::new();
        input.set_pending(KeyCode::Char('g'));
        assert!(!input.has_timed_out());
        // Note: actual timeout test would need to wait 500ms
    }
}

// ============================================================================
// Key Event Helper Tests
// ============================================================================

mod key_event_helpers {
    use super::*;

    #[test]
    fn test_key_event_no_modifiers() {
        let event = key_event(KeyCode::Char('j'));
        assert_eq!(event.code, KeyCode::Char('j'));
        assert!(event.modifiers.is_empty());
    }

    #[test]
    fn test_key_event_ctrl() {
        let event = key_event_ctrl(KeyCode::Char('d'));
        assert_eq!(event.code, KeyCode::Char('d'));
        assert!(event.modifiers.contains(KeyModifiers::CONTROL));
    }
}
