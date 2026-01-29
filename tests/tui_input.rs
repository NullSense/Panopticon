//! Tests for TUI input handling (dispatch layer).
//!
//! Tests the key-to-message mapping for different app modes.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use panopticon::tui::input::{dispatch_normal_mode, dispatch_search_mode, InputState};
use panopticon::tui::Message;

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
// Normal Mode Tests
// ============================================================================

mod normal_mode {
    use super::*;

    #[test]
    fn test_quit() {
        let mut input = InputState::new();
        let msg = dispatch_normal_mode(&mut input, key_event(KeyCode::Char('q')));
        assert_eq!(msg, Message::Quit);
    }

    #[test]
    fn test_navigation() {
        let mut input = InputState::new();
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event(KeyCode::Char('j'))),
            Message::MoveDown
        );
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event(KeyCode::Char('k'))),
            Message::MoveUp
        );
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event(KeyCode::Char('G'))),
            Message::GotoBottom
        );
    }

    #[test]
    fn test_section_navigation() {
        let mut input = InputState::new();
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event_ctrl(KeyCode::Char('d'))),
            Message::JumpNextSection
        );
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event_ctrl(KeyCode::Char('u'))),
            Message::JumpPrevSection
        );
    }

    #[test]
    fn test_search_match_navigation() {
        let mut input = InputState::new();
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event(KeyCode::Char('n'))),
            Message::NextSearchMatch
        );
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event(KeyCode::Char('N'))),
            Message::PrevSearchMatch
        );
    }

    #[test]
    fn test_open_issue_details() {
        let mut input = InputState::new();
        // 'o' and Enter open issue details (link menu)
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event(KeyCode::Char('o'))),
            Message::OpenLinkMenu
        );
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event(KeyCode::Enter)),
            Message::OpenLinkMenu
        );
    }

    #[test]
    fn test_open_links_popup() {
        let mut input = InputState::new();
        // 'l' opens links popup directly
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event(KeyCode::Char('l'))),
            Message::OpenLinksPopup
        );
    }

    #[test]
    fn test_viewport_scroll() {
        let mut input = InputState::new();
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event_ctrl(KeyCode::Char('e'))),
            Message::ScrollViewport(1)
        );
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event_ctrl(KeyCode::Char('y'))),
            Message::ScrollViewport(-1)
        );
    }

    #[test]
    fn test_section_fold_toggle() {
        let mut input = InputState::new();
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event(KeyCode::Char('z'))),
            Message::ToggleSectionFold
        );
    }

    #[test]
    fn test_search() {
        let mut input = InputState::new();
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event(KeyCode::Char('/'))),
            Message::EnterSearch { search_all: false }
        );
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event_ctrl(KeyCode::Char('/'))),
            Message::EnterSearch { search_all: true }
        );
    }

    #[test]
    fn test_chord_pending_state() {
        let mut input = InputState::new();
        let msg = dispatch_normal_mode(&mut input, key_event(KeyCode::Char('g')));
        assert_eq!(msg, Message::None);
        assert!(input.pending.is_some());
        assert!(input.pending_since.is_some());
    }
}

// ============================================================================
// Search Mode Tests
// ============================================================================

mod search_mode {
    use super::*;

    #[test]
    fn test_exit_search() {
        assert_eq!(dispatch_search_mode(key_event(KeyCode::Esc)), Message::ExitSearch);
    }

    #[test]
    fn test_confirm_search() {
        assert_eq!(dispatch_search_mode(key_event(KeyCode::Enter)), Message::ConfirmSearch);
    }

    #[test]
    fn test_search_input() {
        assert_eq!(
            dispatch_search_mode(key_event(KeyCode::Char('a'))),
            Message::SearchInput('a')
        );
    }

    #[test]
    fn test_search_backspace() {
        assert_eq!(
            dispatch_search_mode(key_event(KeyCode::Backspace)),
            Message::SearchBackspace
        );
    }
}

// ============================================================================
// Input State Tests
// ============================================================================

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
