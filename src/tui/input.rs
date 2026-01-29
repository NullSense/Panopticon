//! Input dispatch layer for Elm Architecture (TEA) pattern.
//!
//! Maps key events to messages based on current app mode.
//! Handles key chords (gg, d1-d9, c1-c9) with non-blocking state machine.

use super::{App, Message};
use crate::data::SortMode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Instant;

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

/// Map key events to messages based on current app mode.
///
/// This is the main dispatch function that routes keys to the appropriate
/// mode-specific handler.
pub fn dispatch(app: &App, input: &mut InputState, key: KeyEvent) -> Message {
    // Handle pending chords first
    if let Some(pending) = input.pending.take() {
        input.pending_since = None;
        return handle_chord(app, pending, key.code);
    }

    // Dispatch based on current mode
    if app.state.search_mode {
        dispatch_search_mode(key)
    } else if app.show_description_modal() {
        dispatch_description_modal(input, key)
    } else if app.show_help() {
        dispatch_help_modal(key)
    } else if app.resize_mode() {
        dispatch_resize_mode(key)
    } else if app.show_sort_menu() {
        dispatch_sort_menu(key)
    } else if app.show_link_menu() {
        dispatch_link_menu(app, input, key)
    } else if app.show_filter_menu() {
        dispatch_filter_menu(key)
    } else {
        dispatch_normal_mode(input, key)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mode-specific dispatch functions
// ─────────────────────────────────────────────────────────────────────────────

/// Handle keys in normal mode (main issue list).
fn dispatch_normal_mode(input: &mut InputState, key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Char('q') => Message::Quit,
        KeyCode::Char('j') | KeyCode::Down => Message::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Message::MoveUp,
        KeyCode::Char('G') => Message::GotoBottom,
        KeyCode::Char('g') => {
            input.set_pending(KeyCode::Char('g'));
            Message::None
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => Message::PageDown,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => Message::PageUp,
        KeyCode::Char('/') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::EnterSearch { search_all: true }
        }
        KeyCode::Char('/') => Message::EnterSearch { search_all: false },
        KeyCode::Enter => Message::OpenPrimaryLink,
        KeyCode::Char('o') => Message::OpenLinkMenu,
        KeyCode::Char('t') => Message::TeleportToSession,
        KeyCode::Char('p') => Message::TogglePreview,
        KeyCode::Char('r') => Message::Refresh,
        KeyCode::Char('?') => Message::ToggleHelp,
        KeyCode::Char('h') | KeyCode::Left => Message::CollapseSection,
        KeyCode::Char('l') | KeyCode::Right => Message::ExpandSection,
        KeyCode::Char('s') => Message::ToggleSortMenu,
        KeyCode::Char('f') => Message::ToggleFilterMenu,
        KeyCode::Char('R') => Message::ToggleResizeMode,
        _ => Message::None,
    }
}

/// Handle keys in search mode.
fn dispatch_search_mode(key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Esc => Message::ExitSearch,
        KeyCode::Enter => Message::ConfirmSearch,
        KeyCode::Backspace => Message::SearchBackspace,
        KeyCode::Char(c) => Message::SearchInput(c),
        _ => Message::None,
    }
}

/// Handle keys in description modal.
fn dispatch_description_modal(input: &mut InputState, key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Message::CloseDescriptionModal,
        KeyCode::Char('j') | KeyCode::Down => Message::ScrollDescription(1),
        KeyCode::Char('k') | KeyCode::Up => Message::ScrollDescription(-1),
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::ScrollDescription(10)
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::ScrollDescription(-10)
        }
        KeyCode::Char('G') => Message::ScrollDescription(1000), // Jump to bottom
        KeyCode::Char('g') => {
            input.set_pending(KeyCode::Char('g'));
            Message::None
        }
        _ => Message::None,
    }
}

/// Handle keys in help modal.
fn dispatch_help_modal(key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => Message::CloseModal,
        KeyCode::Char('1') => Message::SetHelpTab(0),
        KeyCode::Char('2') => Message::SetHelpTab(1),
        _ => Message::None,
    }
}

/// Handle keys in resize mode.
fn dispatch_resize_mode(key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('R') => Message::ExitResizeMode,
        KeyCode::Char('h') | KeyCode::Left => Message::ResizeColumnNarrower,
        KeyCode::Char('l') | KeyCode::Right => Message::ResizeColumnWider,
        KeyCode::Tab => Message::ResizeNextColumn,
        KeyCode::BackTab => Message::ResizePrevColumn,
        _ => Message::None,
    }
}

/// Handle keys in sort menu.
fn dispatch_sort_menu(key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Esc => Message::CloseModal,
        KeyCode::Char(c) if ('1'..='6').contains(&c) => {
            let idx = c.to_digit(10).unwrap() as usize;
            if let Some(mode) = SortMode::from_index(idx) {
                Message::SetSortMode(mode)
            } else {
                Message::None
            }
        }
        _ => Message::None,
    }
}

/// Handle keys in filter menu.
fn dispatch_filter_menu(key: KeyEvent) -> Message {
    use crate::data::LinearPriority;
    match key.code {
        KeyCode::Esc => Message::CloseModal,
        // Cycle filters (0-9)
        KeyCode::Char(c) if ('0'..='9').contains(&c) => {
            let idx = c.to_digit(10).unwrap() as usize;
            Message::ToggleCycleFilter(idx)
        }
        // Priority filters
        KeyCode::Char('u') => Message::TogglePriorityFilter(LinearPriority::Urgent),
        KeyCode::Char('h') => Message::TogglePriorityFilter(LinearPriority::High),
        KeyCode::Char('m') => Message::TogglePriorityFilter(LinearPriority::Medium),
        KeyCode::Char('l') => Message::TogglePriorityFilter(LinearPriority::Low),
        KeyCode::Char('n') => Message::TogglePriorityFilter(LinearPriority::NoPriority),
        // Toggle sub-issues
        KeyCode::Char('t') => Message::ToggleSubIssues,
        // Clear/select all
        KeyCode::Char('a') => Message::ClearAllFilters,
        KeyCode::Char('c') => Message::SelectAllFilters,
        _ => Message::None,
    }
}

/// Handle keys in link menu modal.
fn dispatch_link_menu(app: &App, input: &mut InputState, key: KeyEvent) -> Message {
    // Handle links popup first (nested modal)
    if app.show_links_popup() {
        return dispatch_links_popup(key);
    }

    // Handle modal search mode
    if app.modal_search_mode {
        return dispatch_modal_search(key);
    }

    // Handle main link menu
    match key.code {
        KeyCode::Esc => {
            // Clear search first if active, then navigate back, then close
            if !app.modal_search_query.is_empty() {
                Message::ClearModalSearch
            } else {
                Message::NavigateBack
            }
        }
        KeyCode::Char('/') => Message::EnterModalSearch,
        KeyCode::Char('l') => Message::OpenLinksPopup,
        KeyCode::Char('j') | KeyCode::Down => Message::NextChildIssue,
        KeyCode::Char('k') | KeyCode::Up => Message::PrevChildIssue,
        KeyCode::Enter => Message::NavigateToSelectedChild,
        KeyCode::Char('p') => Message::NavigateToParent,
        KeyCode::Char('d') => {
            // Start chord for d1-d9 or description
            input.set_pending(KeyCode::Char('d'));
            Message::None
        }
        KeyCode::Char('c') => {
            // Start chord for c1-c9
            input.set_pending(KeyCode::Char('c'));
            Message::None
        }
        _ => Message::None,
    }
}

/// Handle keys in links popup (nested within link menu).
fn dispatch_links_popup(key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Esc | KeyCode::Char('l') => Message::CloseLinksPopup,
        KeyCode::Char('1') => Message::OpenLinearLink,
        KeyCode::Char('2') => Message::OpenGithubLink,
        KeyCode::Char('3') => Message::OpenVercelLink,
        KeyCode::Char('4') => Message::TeleportToSession,
        _ => Message::None,
    }
}

/// Handle keys in modal search mode (within link menu).
fn dispatch_modal_search(key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => Message::ExitModalSearch,
        KeyCode::Backspace => Message::ModalSearchBackspace,
        KeyCode::Char(c) => Message::ModalSearchInput(c),
        _ => Message::None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Chord handling
// ─────────────────────────────────────────────────────────────────────────────

/// Handle the second key of a chord sequence.
fn handle_chord(app: &App, first: KeyCode, second: KeyCode) -> Message {
    match (first, second) {
        // gg -> go to top (works in normal mode and description modal)
        (KeyCode::Char('g'), KeyCode::Char('g')) => {
            if app.show_description_modal() {
                Message::ScrollDescription(-10000) // Jump to top
            } else {
                Message::GotoTop
            }
        }

        // d1-d9 -> open document (in link menu)
        (KeyCode::Char('d'), KeyCode::Char(c)) if c.is_ascii_digit() && app.show_link_menu() => {
            let digit = c.to_digit(10).unwrap() as usize;
            if digit >= 1 && digit <= 9 {
                Message::OpenDocument(digit - 1)
            } else {
                Message::None
            }
        }

        // d + non-digit -> open description (in link menu)
        (KeyCode::Char('d'), _) if app.show_link_menu() => Message::OpenDescriptionModal,

        // c1-c9 -> navigate to child (in link menu)
        (KeyCode::Char('c'), KeyCode::Char(c)) if c.is_ascii_digit() && app.show_link_menu() => {
            let digit = c.to_digit(10).unwrap() as usize;
            if digit >= 1 && digit <= 9 {
                Message::NavigateToChild(digit - 1)
            } else {
                Message::None
            }
        }

        _ => Message::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

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

    #[test]
    fn test_normal_mode_quit() {
        let mut input = InputState::new();
        let msg = dispatch_normal_mode(&mut input, key_event(KeyCode::Char('q')));
        assert_eq!(msg, Message::Quit);
    }

    #[test]
    fn test_normal_mode_navigation() {
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
    fn test_normal_mode_page_navigation() {
        let mut input = InputState::new();
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event_ctrl(KeyCode::Char('d'))),
            Message::PageDown
        );
        assert_eq!(
            dispatch_normal_mode(&mut input, key_event_ctrl(KeyCode::Char('u'))),
            Message::PageUp
        );
    }

    #[test]
    fn test_normal_mode_search() {
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

    #[test]
    fn test_search_mode() {
        assert_eq!(
            dispatch_search_mode(key_event(KeyCode::Esc)),
            Message::ExitSearch
        );
        assert_eq!(
            dispatch_search_mode(key_event(KeyCode::Enter)),
            Message::ConfirmSearch
        );
        assert_eq!(
            dispatch_search_mode(key_event(KeyCode::Char('a'))),
            Message::SearchInput('a')
        );
        assert_eq!(
            dispatch_search_mode(key_event(KeyCode::Backspace)),
            Message::SearchBackspace
        );
    }

    #[test]
    fn test_input_state_timeout() {
        let mut input = InputState::new();
        input.set_pending(KeyCode::Char('g'));
        assert!(!input.has_timed_out());
        // Note: actual timeout test would need to wait 500ms
    }
}
