//! Tests for keybinding dispatch, particularly shifted/uppercase keys.
//!
//! These tests verify that uppercase characters like G, R, ? work correctly
//! even when they have a SHIFT modifier (which is how terminals report them).

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use panopticon::config::{
    CacheConfig, Config, GithubConfig, LinearConfig, NotificationConfig, PollingConfig, Tokens,
    UiConfig, VercelConfig,
};
use panopticon::tui::input::InputState;
use panopticon::tui::keybindings::dispatch;
use panopticon::tui::{App, Message, ModalState};

// ============================================================================
// Test Helpers
// ============================================================================

fn test_config() -> Config {
    Config {
        tokens: Tokens {
            linear: String::new(),
            github: String::new(),
            vercel: None,
        },
        linear: LinearConfig::default(),
        github: GithubConfig::default(),
        vercel: VercelConfig::default(),
        polling: PollingConfig::default(),
        cache: CacheConfig::default(),
        notifications: NotificationConfig::default(),
        ui: UiConfig::default(),
    }
}

/// Create a key event with no modifiers
fn key_event(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}

/// Create a key event with SHIFT modifier (how terminals report uppercase)
fn key_event_shift(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}

// ============================================================================
// Shifted Key Tests (Normal Mode)
// ============================================================================

#[test]
fn test_uppercase_g_with_shift_modifier_goes_to_bottom() {
    // When user presses Shift+G, terminal sends KeyCode::Char('G') with SHIFT modifier
    let config = test_config();
    let app = App::new(config);
    let mut input = InputState::new();

    // Press Shift+G (uppercase G with SHIFT modifier)
    let key = key_event_shift(KeyCode::Char('G'));
    let msg = dispatch(&app, &mut input, key);

    assert_eq!(msg, Message::GotoBottom, "Shift+G should go to bottom");
}

#[test]
fn test_uppercase_g_without_shift_modifier_goes_to_bottom() {
    // Some terminals may send just 'G' without SHIFT modifier
    let config = test_config();
    let app = App::new(config);
    let mut input = InputState::new();

    // Press G (uppercase without explicit SHIFT)
    let key = key_event(KeyCode::Char('G'));
    let msg = dispatch(&app, &mut input, key);

    assert_eq!(msg, Message::GotoBottom, "G should go to bottom");
}

#[test]
fn test_uppercase_r_with_shift_enters_resize_mode() {
    let config = test_config();
    let app = App::new(config);
    let mut input = InputState::new();

    // Press Shift+R
    let key = key_event_shift(KeyCode::Char('R'));
    let msg = dispatch(&app, &mut input, key);

    assert_eq!(msg, Message::ToggleResizeMode, "Shift+R should enter resize mode");
}

#[test]
fn test_question_mark_with_shift_toggles_help() {
    // ? requires Shift+/ on US keyboards, terminal sends '?' with SHIFT
    let config = test_config();
    let app = App::new(config);
    let mut input = InputState::new();

    // Press Shift+/ (which produces '?')
    let key = key_event_shift(KeyCode::Char('?'));
    let msg = dispatch(&app, &mut input, key);

    assert_eq!(msg, Message::ToggleHelp, "? should toggle help");
}

// ============================================================================
// Shifted Key Tests (Description Mode)
// ============================================================================

#[test]
fn test_uppercase_g_in_description_mode_scrolls_to_bottom() {
    let config = test_config();
    let mut app = App::new(config);
    let mut input = InputState::new();

    // Set up app state to be in description modal
    app.modal = ModalState::Description;

    // Press Shift+G
    let key = key_event_shift(KeyCode::Char('G'));
    let msg = dispatch(&app, &mut input, key);

    // Should scroll to bottom (large positive value)
    match msg {
        Message::ScrollDescription(n) if n > 0 => {}
        other => panic!("Expected ScrollDescription(positive), got {:?}", other),
    }
}

// ============================================================================
// Lowercase keys should still work
// ============================================================================

#[test]
fn test_lowercase_j_moves_down() {
    let config = test_config();
    let app = App::new(config);
    let mut input = InputState::new();

    let key = key_event(KeyCode::Char('j'));
    let msg = dispatch(&app, &mut input, key);

    assert_eq!(msg, Message::MoveDown);
}

#[test]
fn test_lowercase_k_moves_up() {
    let config = test_config();
    let app = App::new(config);
    let mut input = InputState::new();

    let key = key_event(KeyCode::Char('k'));
    let msg = dispatch(&app, &mut input, key);

    assert_eq!(msg, Message::MoveUp);
}

#[test]
fn test_lowercase_r_refreshes() {
    let config = test_config();
    let app = App::new(config);
    let mut input = InputState::new();

    let key = key_event(KeyCode::Char('r'));
    let msg = dispatch(&app, &mut input, key);

    assert_eq!(msg, Message::Refresh);
}
