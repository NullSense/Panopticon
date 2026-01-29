//! Dispatch logic using the keybindings registry.
//!
//! This module provides the main dispatch function that matches key events
//! against the registry and returns appropriate messages.

use super::registry::BINDINGS;
use super::{KeyPattern, Mode};
use crate::data::{LinearPriority, SortMode};
use crate::tui::input::InputState;
use crate::tui::{App, Message};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Determine the current mode from app state.
fn current_mode(app: &App) -> Mode {
    if app.state.search_mode {
        Mode::Search
    } else if app.show_description_modal() {
        Mode::Description
    } else if app.show_help() {
        Mode::Help
    } else if app.resize_mode() {
        Mode::Resize
    } else if app.show_sort_menu() {
        Mode::SortMenu
    } else if app.show_filter_menu() {
        Mode::FilterMenu
    } else if app.show_link_menu() {
        if app.show_links_popup() {
            Mode::LinksPopup
        } else if app.modal_search_mode {
            Mode::ModalSearch
        } else {
            Mode::LinkMenu
        }
    } else {
        Mode::Normal
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

    let mode = current_mode(app);

    // Text input modes handle characters specially
    if mode.is_text_input() {
        return dispatch_text_input(mode, key);
    }

    // Check for chord starters in appropriate modes
    if let Some(msg) = check_chord_start(app, input, mode, key) {
        return msg;
    }

    // Regular single-key dispatch
    dispatch_single_key(app, mode, key)
}

/// Dispatch for text input modes (Search, ModalSearch).
fn dispatch_text_input(mode: Mode, key: KeyEvent) -> Message {
    match mode {
        Mode::Search => match key.code {
            KeyCode::Esc => Message::ExitSearch,
            KeyCode::Enter => Message::ConfirmSearch,
            KeyCode::Backspace => Message::SearchBackspace,
            KeyCode::Down => Message::MoveDown,
            KeyCode::Up => Message::MoveUp,
            KeyCode::Char(c) => Message::SearchInput(c),
            _ => Message::None,
        },
        Mode::ModalSearch => match key.code {
            KeyCode::Esc | KeyCode::Enter => Message::ExitModalSearch,
            KeyCode::Backspace => Message::ModalSearchBackspace,
            KeyCode::Char(c) => Message::ModalSearchInput(c),
            _ => Message::None,
        },
        _ => Message::None,
    }
}

/// Check if a key starts a chord sequence.
fn check_chord_start(
    _app: &App,
    input: &mut InputState,
    mode: Mode,
    key: KeyEvent,
) -> Option<Message> {
    // Only specific modes support chords
    match mode {
        Mode::Normal => {
            // 'g' starts gg chord
            if key.code == KeyCode::Char('g') && key.modifiers.is_empty() {
                input.set_pending(KeyCode::Char('g'));
                return Some(Message::None);
            }
        }
        Mode::Description => {
            // 'g' starts gg chord for scroll to top
            if key.code == KeyCode::Char('g') && key.modifiers.is_empty() {
                input.set_pending(KeyCode::Char('g'));
                return Some(Message::None);
            }
        }
        Mode::LinkMenu => {
            // 'd' starts d1-d9 chord (or opens description on timeout)
            if key.modifiers.is_empty() && key.code == KeyCode::Char('d') {
                input.set_pending(KeyCode::Char('d'));
                return Some(Message::None);
            }
        }
        _ => {}
    }
    None
}

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
            if (1..=9).contains(&digit) {
                Message::OpenDocument(digit - 1)
            } else {
                Message::None
            }
        }

        // d + non-digit -> open description (in link menu)
        (KeyCode::Char('d'), _) if app.show_link_menu() => Message::OpenDescriptionModal,

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

/// Dispatch a single key (non-chord) based on current mode.
fn dispatch_single_key(app: &App, mode: Mode, key: KeyEvent) -> Message {
    // Try to find a matching binding in the registry
    for binding in BINDINGS.iter() {
        // Check if binding applies to current mode
        if !binding.modes.contains(&mode) {
            continue;
        }

        // Check if the key matches the pattern or any alternative
        if matches_pattern(&binding.pattern, &key)
            || binding
                .alternatives
                .iter()
                .any(|alt| matches_pattern(alt, &key))
        {
            return binding_to_message(app, mode, binding, &key);
        }
    }

    Message::None
}

/// Check if a key event matches a pattern.
fn matches_pattern(pattern: &KeyPattern, key: &KeyEvent) -> bool {
    match pattern {
        KeyPattern::Single(code) => {
            if key.code != *code {
                return false;
            }
            // Allow empty modifiers, or SHIFT for characters that require it
            if key.modifiers.is_empty() {
                return true;
            }
            // Allow SHIFT modifier for uppercase letters and shifted symbols
            if key.modifiers == KeyModifiers::SHIFT {
                if let KeyCode::Char(c) = key.code {
                    // Uppercase letters (A-Z) and common shifted symbols
                    return c.is_ascii_uppercase() || "~!@#$%^&*()_+{}|:\"<>?".contains(c);
                }
            }
            false
        }
        KeyPattern::WithModifier { key: code, mods } => {
            key.code == *code && key.modifiers == *mods
        }
        KeyPattern::DigitRange(range) => {
            if let KeyCode::Char(c) = key.code {
                if let Some(digit) = c.to_digit(10) {
                    let d = digit as u8;
                    return range.contains(&d) && key.modifiers.is_empty();
                }
            }
            false
        }
        // Chords and ChordDigit are handled separately in check_chord_start/handle_chord
        KeyPattern::Chord { .. } | KeyPattern::ChordDigit { .. } => false,
    }
}

/// Convert a matched binding to the appropriate message.
fn binding_to_message(app: &App, mode: Mode, binding: &super::KeyBinding, key: &KeyEvent) -> Message {
    // Handle special cases that need parameters from the key
    let result = match mode {
        Mode::Normal => match_normal_mode(key),
        Mode::Description => match_description_mode(key),
        Mode::Help => match_help_mode(key),
        Mode::Resize => match_resize_mode(key),
        Mode::SortMenu => match_sort_menu(key),
        Mode::FilterMenu => match_filter_menu(key),
        Mode::LinkMenu => match_link_menu(app, key),
        Mode::LinksPopup => match_links_popup(key),
        _ => None,
    };
    result.unwrap_or_else(|| {
        // Fallback: try to infer message from description
        message_from_description(binding.description)
    })
}

/// Match normal mode keys to messages.
fn match_normal_mode(key: &KeyEvent) -> Option<Message> {
    Some(match key.code {
        KeyCode::Char('q') => Message::Quit,
        KeyCode::Char('j') | KeyCode::Down => Message::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Message::MoveUp,
        KeyCode::Char('G') => Message::GotoBottom,
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::JumpNextSection
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::JumpPrevSection
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::ScrollViewport(1)
        }
        KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::ScrollViewport(-1)
        }
        KeyCode::Char('/') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::EnterSearch { search_all: true }
        }
        KeyCode::Char('/') => Message::EnterSearch { search_all: false },
        KeyCode::Char('o') | KeyCode::Enter => Message::OpenLinkMenu,
        KeyCode::Char('l') => Message::OpenLinksPopup,
        KeyCode::Char('t') => Message::TeleportToSession,
        KeyCode::Char('p') => Message::TogglePreview,
        KeyCode::Char('r') => Message::Refresh,
        KeyCode::Char('?') => Message::ToggleHelp,
        KeyCode::Char('z') => Message::ToggleSectionFold,
        KeyCode::Left => Message::CollapseSection,
        KeyCode::Right => Message::ExpandSection,
        KeyCode::Char('s') => Message::ToggleSortMenu,
        KeyCode::Char('f') => Message::ToggleFilterMenu,
        KeyCode::Char('R') => Message::ToggleResizeMode,
        _ => return None,
    })
}

/// Match description mode keys to messages.
fn match_description_mode(key: &KeyEvent) -> Option<Message> {
    Some(match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Message::CloseDescriptionModal,
        KeyCode::Char('j') | KeyCode::Down => Message::ScrollDescription(1),
        KeyCode::Char('k') | KeyCode::Up => Message::ScrollDescription(-1),
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::ScrollDescription(10)
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Message::ScrollDescription(-10)
        }
        KeyCode::Char('G') => Message::ScrollDescription(1000),
        _ => return None,
    })
}

/// Match help mode keys to messages.
fn match_help_mode(key: &KeyEvent) -> Option<Message> {
    Some(match key.code {
        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => Message::CloseModal,
        KeyCode::Char('1') => Message::SetHelpTab(0),
        KeyCode::Char('2') => Message::SetHelpTab(1),
        _ => return None,
    })
}

/// Match resize mode keys to messages.
fn match_resize_mode(key: &KeyEvent) -> Option<Message> {
    Some(match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter | KeyCode::Char('R') => {
            Message::ExitResizeMode
        }
        KeyCode::Char('h') | KeyCode::Left => Message::ResizeColumnNarrower,
        KeyCode::Char('l') | KeyCode::Right => Message::ResizeColumnWider,
        KeyCode::Tab => Message::ResizeNextColumn,
        KeyCode::BackTab => Message::ResizePrevColumn,
        _ => return None,
    })
}

/// Match sort menu keys to messages.
fn match_sort_menu(key: &KeyEvent) -> Option<Message> {
    Some(match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Message::CloseModal,
        KeyCode::Char(c) if ('1'..='6').contains(&c) => {
            let idx = c.to_digit(10).unwrap() as usize;
            if let Some(mode) = SortMode::from_index(idx) {
                Message::SetSortMode(mode)
            } else {
                return None;
            }
        }
        _ => return None,
    })
}

/// Match filter menu keys to messages.
fn match_filter_menu(key: &KeyEvent) -> Option<Message> {
    Some(match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Message::CloseModal,
        KeyCode::Char(c) if ('0'..='9').contains(&c) => {
            let idx = c.to_digit(10).unwrap() as usize;
            Message::ToggleCycleFilter(idx)
        }
        KeyCode::Char('u') => Message::TogglePriorityFilter(LinearPriority::Urgent),
        KeyCode::Char('h') => Message::TogglePriorityFilter(LinearPriority::High),
        KeyCode::Char('m') => Message::TogglePriorityFilter(LinearPriority::Medium),
        KeyCode::Char('l') => Message::TogglePriorityFilter(LinearPriority::Low),
        KeyCode::Char('n') => Message::TogglePriorityFilter(LinearPriority::NoPriority),
        KeyCode::Char('t') => Message::ToggleSubIssues,
        KeyCode::Char('d') => Message::ToggleCompletedFilter,
        KeyCode::Char('x') => Message::ToggleCanceledFilter,
        KeyCode::Char('a') => Message::ClearAllFilters,
        KeyCode::Char('c') => Message::SelectAllFilters,
        _ => return None,
    })
}

/// Match link menu keys to messages.
fn match_link_menu(app: &App, key: &KeyEvent) -> Option<Message> {
    Some(match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            // Clear search first if active, then navigate back
            if !app.modal_search_query.is_empty() {
                Message::ClearModalSearch
            } else {
                Message::NavigateBack
            }
        }
        KeyCode::Char('/') => Message::EnterModalSearch,
        KeyCode::Char('j') | KeyCode::Down => Message::NextChildIssue,
        KeyCode::Char('k') | KeyCode::Up => Message::PrevChildIssue,
        KeyCode::Char('o') | KeyCode::Enter => Message::NavigateToSelectedChild,
        KeyCode::Char('l') => Message::OpenLinksPopup,
        KeyCode::Char('t') => Message::TeleportToSession,
        _ => return None,
    })
}

/// Match links popup keys to messages.
fn match_links_popup(key: &KeyEvent) -> Option<Message> {
    Some(match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('l') => Message::CloseLinksPopup,
        KeyCode::Char('1') => Message::OpenLinearLink,
        KeyCode::Char('2') => Message::OpenGithubLink,
        KeyCode::Char('3') => Message::OpenVercelLink,
        KeyCode::Char('4') => Message::TeleportToSession,
        _ => return None,
    })
}

/// Try to infer a message from the binding description.
/// This is a fallback and won't produce parameterized messages.
fn message_from_description(desc: &str) -> Message {
    match desc {
        "Move down" => Message::MoveDown,
        "Move up" => Message::MoveUp,
        "Go to top" => Message::GotoTop,
        "Go to bottom" => Message::GotoBottom,
        "Quit" => Message::Quit,
        "Toggle help" => Message::ToggleHelp,
        "Close modal" => Message::CloseModal,
        _ => Message::None,
    }
}
