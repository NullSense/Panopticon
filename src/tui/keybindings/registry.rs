//! Static registry of all keybindings.
//!
//! This is THE SINGLE SOURCE OF TRUTH for all keybindings in the application.
//! Both dispatch logic and help generation derive from this registry.

use super::{Category, KeyBinding, KeyPattern, Mode};
use crossterm::event::{KeyCode, KeyModifiers};

/// All keybindings in the application.
///
/// Bindings are organized by mode and category. The dispatch system
/// searches this list to find matching bindings for key events.
pub static BINDINGS: &[KeyBinding] = &[
    // ═══════════════════════════════════════════════════════════════════════════
    // NORMAL MODE
    // ═══════════════════════════════════════════════════════════════════════════

    // Navigation
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('j')),
        description: "Move down",
        category: Category::Navigation,
        alternatives: &[KeyPattern::Single(KeyCode::Down)],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('k')),
        description: "Move up",
        category: Category::Navigation,
        alternatives: &[KeyPattern::Single(KeyCode::Up)],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Chord {
            first: KeyCode::Char('g'),
            second: KeyCode::Char('g'),
        },
        description: "Go to top",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('G')),
        description: "Go to bottom",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::WithModifier {
            key: KeyCode::Char('d'),
            mods: KeyModifiers::CONTROL,
        },
        description: "Jump to next section",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::WithModifier {
            key: KeyCode::Char('u'),
            mods: KeyModifiers::CONTROL,
        },
        description: "Jump to prev section",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::WithModifier {
            key: KeyCode::Char('e'),
            mods: KeyModifiers::CONTROL,
        },
        description: "Scroll viewport down",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::WithModifier {
            key: KeyCode::Char('y'),
            mods: KeyModifiers::CONTROL,
        },
        description: "Scroll viewport up",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Left),
        description: "Collapse section",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Right),
        description: "Expand section",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('z')),
        description: "Toggle fold on current section",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: true,
    },
    // Search
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('/')),
        description: "Search active work",
        category: Category::Search,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::WithModifier {
            key: KeyCode::Char('/'),
            mods: KeyModifiers::CONTROL,
        },
        description: "Search all Linear issues",
        category: Category::Search,
        alternatives: &[],
        show_in_help: true,
    },
    // Actions
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('o')),
        description: "Open issue details",
        category: Category::Actions,
        alternatives: &[KeyPattern::Single(KeyCode::Enter)],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('l')),
        description: "Open links popup",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('t')),
        description: "Teleport to Claude session",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('p')),
        description: "Toggle preview panel",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('r')),
        description: "Refresh data",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: true,
    },
    // Modals
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('s')),
        description: "Open sort menu",
        category: Category::Modals,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('f')),
        description: "Open filter menu",
        category: Category::Modals,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('R')),
        description: "Enter resize mode",
        category: Category::Modals,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('?')),
        description: "Toggle help",
        category: Category::Modals,
        alternatives: &[],
        show_in_help: true,
    },
    // Application
    KeyBinding {
        modes: &[Mode::Normal],
        pattern: KeyPattern::Single(KeyCode::Char('q')),
        description: "Quit",
        category: Category::Application,
        alternatives: &[],
        show_in_help: true,
    },
    // ═══════════════════════════════════════════════════════════════════════════
    // SEARCH MODE
    // ═══════════════════════════════════════════════════════════════════════════
    KeyBinding {
        modes: &[Mode::Search],
        pattern: KeyPattern::Single(KeyCode::Esc),
        description: "Exit search mode",
        category: Category::Search,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Search],
        pattern: KeyPattern::Single(KeyCode::Enter),
        description: "Confirm search",
        category: Category::Search,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Search],
        pattern: KeyPattern::Single(KeyCode::Backspace),
        description: "Delete character",
        category: Category::Search,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::Search],
        pattern: KeyPattern::Single(KeyCode::Down),
        description: "Move down",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: true,
    },
    KeyBinding {
        modes: &[Mode::Search],
        pattern: KeyPattern::Single(KeyCode::Up),
        description: "Move up",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: true,
    },
    // ═══════════════════════════════════════════════════════════════════════════
    // DESCRIPTION MODAL
    // ═══════════════════════════════════════════════════════════════════════════
    KeyBinding {
        modes: &[Mode::Description],
        pattern: KeyPattern::Single(KeyCode::Esc),
        description: "Close description",
        category: Category::Modals,
        alternatives: &[KeyPattern::Single(KeyCode::Char('q'))],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::Description],
        pattern: KeyPattern::Single(KeyCode::Char('j')),
        description: "Scroll down",
        category: Category::Navigation,
        alternatives: &[KeyPattern::Single(KeyCode::Down)],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::Description],
        pattern: KeyPattern::Single(KeyCode::Char('k')),
        description: "Scroll up",
        category: Category::Navigation,
        alternatives: &[KeyPattern::Single(KeyCode::Up)],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::Description],
        pattern: KeyPattern::WithModifier {
            key: KeyCode::Char('d'),
            mods: KeyModifiers::CONTROL,
        },
        description: "Page down",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::Description],
        pattern: KeyPattern::WithModifier {
            key: KeyCode::Char('u'),
            mods: KeyModifiers::CONTROL,
        },
        description: "Page up",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::Description],
        pattern: KeyPattern::Single(KeyCode::Char('G')),
        description: "Jump to bottom",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::Description],
        pattern: KeyPattern::Chord {
            first: KeyCode::Char('g'),
            second: KeyCode::Char('g'),
        },
        description: "Jump to top",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: false,
    },
    // ═══════════════════════════════════════════════════════════════════════════
    // HELP MODAL
    // ═══════════════════════════════════════════════════════════════════════════
    KeyBinding {
        modes: &[Mode::Help],
        pattern: KeyPattern::Single(KeyCode::Esc),
        description: "Close help",
        category: Category::Modals,
        alternatives: &[
            KeyPattern::Single(KeyCode::Char('?')),
            KeyPattern::Single(KeyCode::Char('q')),
        ],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::Help],
        pattern: KeyPattern::Single(KeyCode::Char('1')),
        description: "Show shortcuts tab",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::Help],
        pattern: KeyPattern::Single(KeyCode::Char('2')),
        description: "Show status legend tab",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: false,
    },
    // ═══════════════════════════════════════════════════════════════════════════
    // RESIZE MODE
    // ═══════════════════════════════════════════════════════════════════════════
    KeyBinding {
        modes: &[Mode::Resize],
        pattern: KeyPattern::Single(KeyCode::Esc),
        description: "Exit resize mode",
        category: Category::Modals,
        alternatives: &[
            KeyPattern::Single(KeyCode::Char('q')),
            KeyPattern::Single(KeyCode::Enter),
            KeyPattern::Single(KeyCode::Char('R')),
        ],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::Resize],
        pattern: KeyPattern::Single(KeyCode::Char('h')),
        description: "Make column narrower",
        category: Category::Actions,
        alternatives: &[KeyPattern::Single(KeyCode::Left)],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::Resize],
        pattern: KeyPattern::Single(KeyCode::Char('l')),
        description: "Make column wider",
        category: Category::Actions,
        alternatives: &[KeyPattern::Single(KeyCode::Right)],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::Resize],
        pattern: KeyPattern::Single(KeyCode::Tab),
        description: "Next column",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::Resize],
        pattern: KeyPattern::Single(KeyCode::BackTab),
        description: "Previous column",
        category: Category::Navigation,
        alternatives: &[],
        show_in_help: false,
    },
    // ═══════════════════════════════════════════════════════════════════════════
    // SORT MENU
    // ═══════════════════════════════════════════════════════════════════════════
    KeyBinding {
        modes: &[Mode::SortMenu],
        pattern: KeyPattern::Single(KeyCode::Esc),
        description: "Close sort menu",
        category: Category::Modals,
        alternatives: &[KeyPattern::Single(KeyCode::Char('q'))],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::SortMenu],
        pattern: KeyPattern::DigitRange(1..=6),
        description: "Select sort mode",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    // ═══════════════════════════════════════════════════════════════════════════
    // FILTER MENU
    // ═══════════════════════════════════════════════════════════════════════════
    KeyBinding {
        modes: &[Mode::FilterMenu],
        pattern: KeyPattern::Single(KeyCode::Esc),
        description: "Close filter menu",
        category: Category::Modals,
        alternatives: &[KeyPattern::Single(KeyCode::Char('q'))],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::FilterMenu],
        pattern: KeyPattern::DigitRange(0..=9),
        description: "Toggle cycle filter",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::FilterMenu],
        pattern: KeyPattern::Single(KeyCode::Char('u')),
        description: "Toggle Urgent priority",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::FilterMenu],
        pattern: KeyPattern::Single(KeyCode::Char('h')),
        description: "Toggle High priority",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::FilterMenu],
        pattern: KeyPattern::Single(KeyCode::Char('m')),
        description: "Toggle Medium priority",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::FilterMenu],
        pattern: KeyPattern::Single(KeyCode::Char('l')),
        description: "Toggle Low priority",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::FilterMenu],
        pattern: KeyPattern::Single(KeyCode::Char('n')),
        description: "Toggle No Priority",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::FilterMenu],
        pattern: KeyPattern::Single(KeyCode::Char('t')),
        description: "Toggle sub-issues",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::FilterMenu],
        pattern: KeyPattern::Single(KeyCode::Char('d')),
        description: "Toggle completed issues",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::FilterMenu],
        pattern: KeyPattern::Single(KeyCode::Char('x')),
        description: "Toggle canceled issues",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::FilterMenu],
        pattern: KeyPattern::Single(KeyCode::Char('a')),
        description: "Clear all filters",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::FilterMenu],
        pattern: KeyPattern::Single(KeyCode::Char('c')),
        description: "Select all filters",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    // ═══════════════════════════════════════════════════════════════════════════
    // LINK MENU (Issue Details)
    // ═══════════════════════════════════════════════════════════════════════════
    KeyBinding {
        modes: &[Mode::LinkMenu],
        pattern: KeyPattern::Single(KeyCode::Esc),
        description: "Go back / close",
        category: Category::Navigation,
        alternatives: &[KeyPattern::Single(KeyCode::Char('q'))],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::LinkMenu],
        pattern: KeyPattern::Single(KeyCode::Char('/')),
        description: "Search children",
        category: Category::Search,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::LinkMenu],
        pattern: KeyPattern::Single(KeyCode::Char('j')),
        description: "Next child issue",
        category: Category::Navigation,
        alternatives: &[KeyPattern::Single(KeyCode::Down)],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::LinkMenu],
        pattern: KeyPattern::Single(KeyCode::Char('k')),
        description: "Previous child issue",
        category: Category::Navigation,
        alternatives: &[KeyPattern::Single(KeyCode::Up)],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::LinkMenu],
        pattern: KeyPattern::Single(KeyCode::Char('o')),
        description: "Enter selected issue",
        category: Category::Actions,
        alternatives: &[KeyPattern::Single(KeyCode::Enter)],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::LinkMenu],
        pattern: KeyPattern::Single(KeyCode::Char('l')),
        description: "Open links popup",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::LinkMenu],
        pattern: KeyPattern::Single(KeyCode::Char('t')),
        description: "Teleport to Claude session",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    // Chord starter for documents
    KeyBinding {
        modes: &[Mode::LinkMenu],
        pattern: KeyPattern::ChordDigit {
            prefix: 'd',
            range: 1..=9,
        },
        description: "Open document by number",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    // ═══════════════════════════════════════════════════════════════════════════
    // LINKS POPUP (nested within link menu)
    // ═══════════════════════════════════════════════════════════════════════════
    KeyBinding {
        modes: &[Mode::LinksPopup],
        pattern: KeyPattern::Single(KeyCode::Esc),
        description: "Close links popup",
        category: Category::Modals,
        alternatives: &[
            KeyPattern::Single(KeyCode::Char('q')),
            KeyPattern::Single(KeyCode::Char('l')),
        ],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::LinksPopup],
        pattern: KeyPattern::Single(KeyCode::Char('1')),
        description: "Open Linear link",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::LinksPopup],
        pattern: KeyPattern::Single(KeyCode::Char('2')),
        description: "Open GitHub link",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::LinksPopup],
        pattern: KeyPattern::Single(KeyCode::Char('3')),
        description: "Open Vercel link",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::LinksPopup],
        pattern: KeyPattern::Single(KeyCode::Char('4')),
        description: "Teleport to Claude session",
        category: Category::Actions,
        alternatives: &[],
        show_in_help: false,
    },
    // ═══════════════════════════════════════════════════════════════════════════
    // MODAL SEARCH (within link menu)
    // ═══════════════════════════════════════════════════════════════════════════
    KeyBinding {
        modes: &[Mode::ModalSearch],
        pattern: KeyPattern::Single(KeyCode::Esc),
        description: "Exit search",
        category: Category::Search,
        alternatives: &[KeyPattern::Single(KeyCode::Enter)],
        show_in_help: false,
    },
    KeyBinding {
        modes: &[Mode::ModalSearch],
        pattern: KeyPattern::Single(KeyCode::Backspace),
        description: "Delete character",
        category: Category::Search,
        alternatives: &[],
        show_in_help: false,
    },
];
