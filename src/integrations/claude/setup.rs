//! Claude Code hook injection
//!
//! Safely patches ~/.claude/settings.json to add Panopticon hooks

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

/// Get the path to Claude's settings.json
pub fn claude_settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("settings.json"))
}

/// Validate that a JSON value is a valid Claude settings object
fn validate_settings(settings: &Value) -> bool {
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

/// Check if our hooks are already installed
pub fn hooks_installed() -> bool {
    let path = match claude_settings_path() {
        Some(p) => p,
        None => return false,
    };

    if !path.exists() {
        return false;
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let settings: Value = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Check if our hooks are present
    if let Some(hooks) = settings.get("hooks") {
        if let Some(session_start) = hooks.get("SessionStart") {
            if let Some(arr) = session_start.as_array() {
                return arr.iter().any(|h| {
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

/// Inject our hooks into Claude's settings
pub fn inject_hooks() -> Result<()> {
    let path = claude_settings_path()
        .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

    // Read existing settings or create empty object
    let mut settings: Value = if path.exists() {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        if content.trim().is_empty() {
            json!({})
        } else {
            match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(e) => {
                    // Create backup of invalid file before overwriting
                    let backup_path = path.with_extension("json.bak");
                    if let Err(backup_err) = fs::copy(&path, &backup_path) {
                        tracing::warn!(
                            "Failed to backup invalid settings to {}: {}",
                            backup_path.display(),
                            backup_err
                        );
                    } else {
                        tracing::warn!(
                            "Backed up invalid settings to {}",
                            backup_path.display()
                        );
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
        // Ensure .claude directory exists
        if let Some(parent) = path.parent() {
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

    // Helper to add our hook to a hook type
    fn add_panopticon_hook(hooks: &mut Value, hook_type: &str, event: &str) {
        let hook_entry = json!({
            "commands": format!("panopticon internal-hook --event {}", event)
        });

        if let Some(existing) = hooks.get_mut(hook_type) {
            if let Some(arr) = existing.as_array_mut() {
                // Check if already present
                let already_present = arr.iter().any(|h| {
                    h.get("commands")
                        .and_then(|c| c.as_str())
                        .map(|s| s.contains("panopticon"))
                        .unwrap_or(false)
                });
                if !already_present {
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

    // Add our hooks
    if let Some(hooks) = settings.get_mut("hooks") {
        add_panopticon_hook(hooks, "SessionStart", "start");
        add_panopticon_hook(hooks, "UserPromptSubmit", "active");
        add_panopticon_hook(hooks, "Stop", "stop");
    }

    // Validate final settings before writing
    if !validate_settings(&settings) {
        anyhow::bail!("Generated settings are invalid, refusing to write");
    }

    // Write back
    let content = serde_json::to_string_pretty(&settings)?;
    fs::write(&path, &content)
        .with_context(|| format!("Failed to write settings to {}", path.display()))?;

    tracing::info!("Injected Panopticon hooks into Claude settings");

    Ok(())
}

/// Ensure hooks are installed (called on startup)
pub fn ensure_hooks() -> Result<()> {
    if !hooks_installed() {
        inject_hooks()?;
    }
    Ok(())
}
