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
pub fn dispatch_normal_mode(input: &mut InputState, key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Char('q') => Message::Quit,
        KeyCode::Char('j') | KeyCode::Down => Message::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Message::MoveUp,
        KeyCode::Char('G') => Message::GotoBottom,
        KeyCode::Char('g') => {
            input.set_pending(KeyCode::Char('g'));
            Message::None
        }
        // Section navigation
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::JumpNextSection
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::JumpPrevSection
        }
        // Viewport scrolling (vim Ctrl+e/y - scroll without moving cursor)
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::ScrollViewport(1)
        }
        KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::ScrollViewport(-1)
        }
        // Search
        KeyCode::Char('/') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::EnterSearch { search_all: true }
        }
        KeyCode::Char('/') => Message::EnterSearch { search_all: false },
        // Actions
        KeyCode::Char('o') | KeyCode::Enter => Message::OpenLinkMenu, // Open issue details
        KeyCode::Char('l') => Message::OpenLinksPopup,                // Open links popup directly
        KeyCode::Char('t') => Message::TeleportToSession,
        KeyCode::Char('p') => Message::TogglePreview,
        KeyCode::Char('r') => Message::Refresh,
        KeyCode::Char('?') => Message::ToggleHelp,
        // Section folding
        KeyCode::Char('z') => Message::ToggleSectionFold,
        KeyCode::Char('h') | KeyCode::Left => Message::CollapseSection,
        KeyCode::Right => Message::ExpandSection,
        // Menus
        KeyCode::Char('s') => Message::ToggleSortMenu,
        KeyCode::Char('f') => Message::ToggleFilterMenu,
        KeyCode::Char('R') => Message::ToggleResizeMode,
        _ => Message::None,
    }
}

/// Handle keys in search mode (typing query).
/// Only basic navigation and text input work here.
pub fn dispatch_search_mode(key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Esc => Message::ExitSearch,
        KeyCode::Enter => Message::ConfirmSearch,
        KeyCode::Backspace => Message::SearchBackspace,
        // Basic navigation works during search
        KeyCode::Down => Message::MoveDown,
        KeyCode::Up => Message::MoveUp,
        // All chars are search input (including j/k/n/N)
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
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter | KeyCode::Char('R') => Message::ExitResizeMode,
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
        KeyCode::Esc | KeyCode::Char('q') => Message::CloseModal,
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
        KeyCode::Esc | KeyCode::Char('q') => Message::CloseModal,
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

/// Handle keys in link menu modal (issue details view).
/// Keybindings are unified with normal mode where possible.
fn dispatch_link_menu(app: &App, input: &mut InputState, key: KeyEvent) -> Message {
    // Handle links popup first (nested modal)
    if app.show_links_popup() {
        return dispatch_links_popup(key);
    }

    // Handle modal search mode
    if app.modal_search_mode {
        return dispatch_modal_search(key);
    }

    // Handle main link menu (issue details)
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            // Clear search first if active, then navigate back, then close
            if !app.modal_search_query.is_empty() {
                Message::ClearModalSearch
            } else {
                Message::NavigateBack
            }
        }
        // Search
        KeyCode::Char('/') => Message::EnterModalSearch,
        // Navigation (same as normal mode)
        KeyCode::Char('j') | KeyCode::Down => Message::NextChildIssue,
        KeyCode::Char('k') | KeyCode::Up => Message::PrevChildIssue,
        // Actions (unified with normal mode)
        KeyCode::Char('o') | KeyCode::Enter => Message::NavigateToSelectedChild, // Enter into child
        KeyCode::Char('l') => Message::OpenLinksPopup,                           // Open links
        KeyCode::Char('p') => Message::NavigateToParent,
        KeyCode::Char('t') => Message::TeleportToSession,
        // Chords for documents and children
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
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('l') => Message::CloseLinksPopup,
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

/// Handle chord timeout - return fallback action for pending chord
/// Called when a chord key was pressed but timed out without a second key
pub fn handle_chord_timeout(app: &App, input: &InputState) -> Message {
    match input.pending {
        // 'd' alone in link menu -> open description
        Some(KeyCode::Char('d')) if app.show_link_menu() => Message::OpenDescriptionModal,
        // Other pending keys have no fallback action
        _ => Message::None,
    }
}
