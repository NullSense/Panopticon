//! Centralized keybindings system.
//!
//! This module provides a single source of truth for all keybindings in the application.
//! The registry defines all bindings, and dispatch/help generation are derived from it.

mod dispatch;
mod help;
mod registry;

pub use dispatch::{dispatch, handle_chord_timeout};
pub use help::{generate_footer_hints, generate_keyboard_shortcuts};

use crossterm::event::{KeyCode, KeyModifiers};
use std::ops::RangeInclusive;

/// All contexts where keybindings apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Main issue list view
    Normal,
    /// Search input mode (typing in search bar)
    Search,
    /// Search within link menu modal
    ModalSearch,
    /// Description popup
    Description,
    /// Help popup
    Help,
    /// Column resize mode
    Resize,
    /// Sort menu popup
    SortMenu,
    /// Filter menu popup
    FilterMenu,
    /// Issue details modal (link menu)
    LinkMenu,
    /// Links popup (nested within link menu)
    LinksPopup,
}

impl Mode {
    /// Returns true if this mode accepts text input (chars are not dispatched as commands).
    pub fn is_text_input(&self) -> bool {
        matches!(self, Mode::Search | Mode::ModalSearch)
    }
}

/// Categories for grouping bindings in help display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Navigation,
    Search,
    Actions,
    Modals,
    Application,
}

impl Category {
    pub fn label(&self) -> &'static str {
        match self {
            Category::Navigation => "Navigation",
            Category::Search => "Search",
            Category::Actions => "Actions",
            Category::Modals => "Modals",
            Category::Application => "Application",
        }
    }
}

/// Pattern for matching key events.
#[derive(Debug, Clone)]
pub enum KeyPattern {
    /// Single key without modifiers (e.g., 'j', Enter, Esc)
    Single(KeyCode),
    /// Key with modifiers (e.g., Ctrl+d)
    WithModifier { key: KeyCode, mods: KeyModifiers },
    /// Two-key chord (e.g., gg)
    Chord { first: KeyCode, second: KeyCode },
    /// Chord with digit range (e.g., d1-d9)
    ChordDigit {
        prefix: char,
        range: RangeInclusive<u8>,
    },
    /// Single digit range (e.g., 1-6 in sort menu)
    DigitRange(RangeInclusive<u8>),
}

impl KeyPattern {
    /// Format this pattern for display in help text.
    #[allow(dead_code)]
    pub fn display(&self) -> String {
        match self {
            KeyPattern::Single(code) => format_keycode(code),
            KeyPattern::WithModifier { key, mods } => {
                let mut result = String::new();
                if mods.contains(KeyModifiers::CONTROL) {
                    result.push_str("Ctrl+");
                }
                if mods.contains(KeyModifiers::ALT) {
                    result.push_str("Alt+");
                }
                if mods.contains(KeyModifiers::SHIFT) {
                    result.push_str("Shift+");
                }
                result.push_str(&format_keycode(key));
                result
            }
            KeyPattern::Chord { first, second } => {
                format!("{}{}", format_keycode(first), format_keycode(second))
            }
            KeyPattern::ChordDigit { prefix, range } => {
                format!("{}{}-{}", prefix, range.start(), range.end())
            }
            KeyPattern::DigitRange(range) => {
                format!("{}-{}", range.start(), range.end())
            }
        }
    }
}

/// Format a KeyCode for display.
#[allow(dead_code)]
fn format_keycode(code: &KeyCode) -> String {
    match code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "Shift+Tab".to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        _ => format!("{:?}", code),
    }
}

/// A complete keybinding definition.
#[derive(Debug, Clone)]
pub struct KeyBinding {
    /// Modes where this binding applies
    pub modes: &'static [Mode],
    /// The key pattern to match
    pub pattern: KeyPattern,
    /// Human-readable description for help text
    pub description: &'static str,
    /// Category for grouping in help
    pub category: Category,
    /// Alternative key patterns (e.g., j and Down for same action)
    pub alternatives: &'static [KeyPattern],
    /// Whether to show this binding in help (false for internal bindings)
    pub show_in_help: bool,
}
