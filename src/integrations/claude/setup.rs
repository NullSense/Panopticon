//! Claude Code hook injection
//!
//! Safely patches ~/.claude/settings.json to add Panopticon hooks

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// Get the path to Claude's settings.json
pub fn claude_settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("settings.json"))
}

/// Validate that a JSON value is a valid Claude settings object
pub fn validate_settings(settings: &Value) -> bool {
    // Settings must be an object
    if !settings.is_object() {
        return false;
    }

    // If hooks exists, it must be an object
    if let Some(hooks) = settings.get("hooks") {
        if !hooks.is_object() {
            return false;
        }

        // Each hook type must be an array
        if let Some(obj) = hooks.as_object() {
            for (_, hook_list) in obj {
                if !hook_list.is_array() {
                    return false;
                }
            }
        }
    }

    true
}

/// Check if panopticon hooks exist in a settings JSON value
pub fn has_panopticon_hooks(settings: &Value) -> bool {
    if let Some(hooks) = settings.get("hooks") {
        if let Some(session_start) = hooks.get("SessionStart") {
            if let Some(arr) = session_start.as_array() {
                return arr.iter().any(|h| {
                    // Check new format: { "matcher": "", "hooks": [{ "command": "..." }] }
                    if let Some(hook_arr) = h.get("hooks").and_then(|h| h.as_array()) {
                        if hook_arr.iter().any(|inner| {
                            inner
                                .get("command")
                                .and_then(|c| c.as_str())
                                .map(|s| s.contains("panopticon"))
                                .unwrap_or(false)
                        }) {
                            return true;
                        }
                    }
                    // Check old format: { "commands": "..." }
                    h.get("commands")
                        .and_then(|c| c.as_str())
                        .map(|s| s.contains("panopticon"))
                        .unwrap_or(false)
                });
            }
        }
    }
    false
}

/// Check if our hooks are already installed at a specific path (testable version)
pub fn hooks_installed_at_path(settings_path: &Path) -> bool {
    if !settings_path.exists() {
        return false;
    }

    let content = match fs::read_to_string(settings_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let settings: Value = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(_) => return false,
    };

    has_panopticon_hooks(&settings)
}

/// Check if our hooks are already installed
pub fn hooks_installed() -> bool {
    let path = match claude_settings_path() {
        Some(p) => p,
        None => return false,
    };
    hooks_installed_at_path(&path)
}

/// Generate a panopticon hook entry in the new Claude Code format
pub fn generate_hook_entry(event: &str) -> Value {
    json!({
        "matcher": "",
        "hooks": [
            {
                "type": "command",
                "command": format!("panopticon internal-hook --event {}", event)
            }
        ]
    })
}

/// Add panopticon hook to a hooks object, replacing any existing panopticon hook
pub fn add_panopticon_hook(hooks: &mut Value, hook_type: &str, event: &str) {
    let hook_entry = generate_hook_entry(event);
    let expected_command = format!("panopticon internal-hook --event {}", event);

    if let Some(existing) = hooks.get_mut(hook_type) {
        if let Some(arr) = existing.as_array_mut() {
            // Remove any existing panopticon hooks (might have wrong event)
            arr.retain(|h| {
                // Check new format
                if let Some(hook_arr) = h.get("hooks").and_then(|h| h.as_array()) {
                    let is_panopticon = hook_arr.iter().any(|inner| {
                        inner
                            .get("command")
                            .and_then(|c| c.as_str())
                            .map(|s| s.contains("panopticon"))
                            .unwrap_or(false)
                    });
                    return !is_panopticon;
                }
                // Check old format
                let is_panopticon = h
                    .get("commands")
                    .and_then(|c| c.as_str())
                    .map(|s| s.contains("panopticon"))
                    .unwrap_or(false);
                !is_panopticon
            });

            // Check if correct hook already exists after cleanup
            let has_correct = arr.iter().any(|h| {
                if let Some(hook_arr) = h.get("hooks").and_then(|h| h.as_array()) {
                    hook_arr.iter().any(|inner| {
                        inner
                            .get("command")
                            .and_then(|c| c.as_str())
                            .map(|s| s == expected_command)
                            .unwrap_or(false)
                    })
                } else {
                    false
                }
            });

            if !has_correct {
                arr.push(hook_entry);
            }
        } else {
            // Hook type exists but is not an array, fix it
            tracing::warn!("Hook type {} was not an array, fixing", hook_type);
            hooks[hook_type] = json!([hook_entry]);
        }
    } else {
        hooks[hook_type] = json!([hook_entry]);
    }
}

/// Inject our hooks into Claude's settings at a specific path (testable version)
pub fn inject_hooks_to_path(settings_path: &Path) -> Result<()> {
    // Read existing settings or create empty object
    let mut settings: Value = if settings_path.exists() {
        let content = fs::read_to_string(settings_path)
            .with_context(|| format!("Failed to read {}", settings_path.display()))?;

        if content.trim().is_empty() {
            json!({})
        } else {
            match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(e) => {
                    // Create backup of invalid file before overwriting
                    let backup_path = settings_path.with_extension("json.bak");
                    if let Err(backup_err) = fs::copy(settings_path, &backup_path) {
                        tracing::warn!(
                            "Failed to backup invalid settings to {}: {}",
                            backup_path.display(),
                            backup_err
                        );
                    } else {
                        tracing::warn!("Backed up invalid settings to {}", backup_path.display());
                    }
                    tracing::warn!(
                        "Existing settings.json is invalid JSON ({}), starting fresh",
                        e
                    );
                    json!({})
                }
            }
        }
    } else {
        // Ensure parent directory exists
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }
        json!({})
    };

    // Validate existing settings structure
    if !validate_settings(&settings) {
        tracing::warn!("Settings structure is invalid, rebuilding hooks section");
        // Only reset the hooks section, preserve other settings
        settings["hooks"] = json!({});
    }

    // Ensure hooks object exists
    if settings.get("hooks").is_none() {
        settings["hooks"] = json!({});
    }

    // Add our hooks
    if let Some(hooks) = settings.get_mut("hooks") {
        // Session lifecycle
        add_panopticon_hook(hooks, "SessionStart", "start");
        add_panopticon_hook(hooks, "UserPromptSubmit", "prompt");
        add_panopticon_hook(hooks, "Stop", "stop");

        // Tool execution (for rich activity tracking)
        add_panopticon_hook(hooks, "PreToolUse", "tool_start");
        add_panopticon_hook(hooks, "PostToolUse", "tool_done");
        add_panopticon_hook(hooks, "PostToolUseFailure", "tool_fail");

        // Subagent tracking
        add_panopticon_hook(hooks, "SubagentStart", "subagent_start");
        add_panopticon_hook(hooks, "SubagentStop", "subagent_stop");
    }

    // Validate final settings before writing
    if !validate_settings(&settings) {
        anyhow::bail!("Generated settings are invalid, refusing to write");
    }

    // Write back
    let content = serde_json::to_string_pretty(&settings)?;
    fs::write(settings_path, &content)
        .with_context(|| format!("Failed to write settings to {}", settings_path.display()))?;

    tracing::info!("Injected Panopticon hooks into Claude settings");

    Ok(())
}

/// Inject our hooks into Claude's settings
pub fn inject_hooks() -> Result<()> {
    let path =
        claude_settings_path().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    inject_hooks_to_path(&path)
}

/// Check if all required hooks are present with correct events
fn all_hooks_present(settings: &Value) -> bool {
    let required_hooks = [
        ("SessionStart", "start"),
        ("UserPromptSubmit", "prompt"),
        ("Stop", "stop"),
        ("PreToolUse", "tool_start"),
        ("PostToolUse", "tool_done"),
        ("PostToolUseFailure", "tool_fail"),
        ("SubagentStart", "subagent_start"),
        ("SubagentStop", "subagent_stop"),
    ];

    let Some(hooks) = settings.get("hooks") else {
        return false;
    };

    for (hook_type, event) in required_hooks {
        let Some(hook_arr) = hooks.get(hook_type).and_then(|h| h.as_array()) else {
            return false;
        };

        // Check if panopticon hook with correct event exists
        let has_correct_hook = hook_arr.iter().any(|h| {
            if let Some(inner_hooks) = h.get("hooks").and_then(|h| h.as_array()) {
                inner_hooks.iter().any(|inner| {
                    inner
                        .get("command")
                        .and_then(|c| c.as_str())
                        .map(|s| {
                            s.contains("panopticon")
                                && s.contains(&format!("--event {}", event))
                        })
                        .unwrap_or(false)
                })
            } else {
                false
            }
        });

        if !has_correct_hook {
            return false;
        }
    }

    true
}

/// Ensure hooks are installed (called on startup)
pub fn ensure_hooks() -> Result<()> {
    let path = match claude_settings_path() {
        Some(p) => p,
        None => return Ok(()),
    };

    if !path.exists() {
        inject_hooks()?;
        return Ok(());
    }

    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let settings: Value = serde_json::from_str(&content).unwrap_or(json!({}));

    // Re-inject if any hooks are missing or have wrong events
    if !all_hooks_present(&settings) {
        tracing::info!("Updating Claude hooks (missing or outdated hooks detected)");
        inject_hooks()?;
    }

    Ok(())
}
