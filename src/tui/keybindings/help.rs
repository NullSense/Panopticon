//! Auto-generate help text from the keybindings registry.

use super::registry::BINDINGS;
use super::{Category, KeyPattern, Mode};

/// Generate compact footer hints for a given mode.
/// Returns a string like "l: links | /: search | Esc: back"
pub fn generate_footer_hints(mode: Mode) -> &'static str {
    match mode {
        Mode::LinkMenu => "  j/k: nav | o: enter | l: links | /: search | d: desc | Esc: back",
        Mode::LinksPopup => "  1-4: open link | l/Esc: close",
        Mode::Description => "  j/k: scroll | gg/G: top/bottom | Esc: close",
        Mode::Help => "  1/2: tabs | Esc: close",
        Mode::SortMenu => "  1-6: select | Esc: close",
        Mode::FilterMenu => "  0-9: cycles | p0-9: projects | s0-8: assignees (s9=all) | u/h/m/l/n: priority | Esc: close",
        Mode::Resize => "  h/l: width | Tab: column | Esc: done",
        Mode::Normal => "  j/k: nav | o: details | l: links | /: search | ?: help",
        Mode::Search | Mode::ModalSearch => "  Enter: confirm | Esc: cancel",
    }
}

/// Generate keyboard shortcuts help text for the main help popup.
/// This combines Normal and Search mode bindings.
pub fn generate_keyboard_shortcuts() -> Vec<&'static str> {
    // We return a static slice to match the existing Vec<&'static str> return type
    // The help text is organized by category with headers
    vec![
        "",
        "  Navigation",
        "  ──────────",
        "  j/k, ↑/↓     Move up/down",
        "  gg           Go to top",
        "  G            Go to bottom",
        "  Ctrl+d/u     Jump to next/prev section",
        "  Ctrl+e/y     Scroll viewport (vim-style)",
        "  h/←          Collapse section",
        "  →            Expand section",
        "  z            Toggle fold on current section",
        "",
        "  Search",
        "  ──────",
        "  /            Search active work",
        "  Ctrl+/       Search all Linear issues",
        "  j/k          Navigate through matches",
        "  Enter        Confirm search",
        "  Esc          Exit search mode",
        "",
        "  Actions",
        "  ───────",
        "  o, Enter     Open issue details",
        "  l            Open links popup (Linear/GitHub/...)",
        "  t            Teleport to Claude session",
        "  p            Toggle preview panel",
        "  s            Open sort menu",
        "  f            Open filter menu",
        "  r            Refresh data",
        "",
        "  q            Quit",
        "  ?            Toggle this help",
        "",
    ]
}

/// Format a binding's keys for display (primary + alternatives).
#[allow(dead_code)]
fn format_binding_keys(pattern: &KeyPattern, alternatives: &[KeyPattern]) -> String {
    let mut parts = vec![pattern.display()];
    for alt in alternatives {
        parts.push(alt.display());
    }
    parts.join(", ")
}

/// Generate help text for a specific mode (for future use).
#[allow(dead_code)]
pub fn generate_help_for_mode(mode: Mode) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_category: Option<Category> = None;

    // Filter bindings for this mode and that should show in help
    let mode_bindings: Vec<_> = BINDINGS
        .iter()
        .filter(|b| b.modes.contains(&mode) && b.show_in_help)
        .collect();

    for binding in mode_bindings {
        // Add category header if changed
        if current_category != Some(binding.category) {
            if current_category.is_some() {
                lines.push(String::new());
            }
            lines.push(format!("  {}", binding.category.label()));
            lines.push(format!("  {}", "─".repeat(binding.category.label().len())));
            current_category = Some(binding.category);
        }

        // Format the binding
        let keys = format_binding_keys(&binding.pattern, binding.alternatives);
        lines.push(format!("  {:14}{}", keys, binding.description));
    }

    lines
}
