#![allow(dead_code)]
//! Test utilities for Claude settings.json testing

use serde_json::{json, Value};
use std::path::PathBuf;
use tempfile::TempDir;

/// A test environment with a temporary home directory for Claude settings
pub struct TestClaudeEnv {
    pub temp_dir: TempDir,
    pub claude_dir: PathBuf,
    pub settings_path: PathBuf,
}

impl TestClaudeEnv {
    /// Create a new test environment with temporary directories
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let claude_dir = temp_dir.path().join(".claude");
        let settings_path = claude_dir.join("settings.json");

        Self {
            temp_dir,
            claude_dir,
            settings_path,
        }
    }

    /// Create the .claude directory
    pub fn create_claude_dir(&self) {
        std::fs::create_dir_all(&self.claude_dir).expect("Failed to create .claude dir");
    }

    /// Write settings to the settings.json file
    pub fn write_settings(&self, settings: &Value) {
        self.create_claude_dir();
        let content = serde_json::to_string_pretty(settings).expect("Failed to serialize settings");
        std::fs::write(&self.settings_path, content).expect("Failed to write settings");
    }

    /// Read settings from the settings.json file
    pub fn read_settings(&self) -> Value {
        let content =
            std::fs::read_to_string(&self.settings_path).expect("Failed to read settings");
        serde_json::from_str(&content).expect("Failed to parse settings")
    }

    /// Check if settings file exists
    pub fn settings_exists(&self) -> bool {
        self.settings_path.exists()
    }
}

impl Default for TestClaudeEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Empty settings fixture
pub fn empty_settings() -> Value {
    json!({})
}

/// Settings with hooks in the OLD format (pre-2024)
pub fn settings_with_old_format_hooks() -> Value {
    json!({
        "hooks": {
            "SessionStart": [
                {"commands": "panopticon internal-hook --event start"}
            ],
            "UserPromptSubmit": [
                {"commands": "panopticon internal-hook --event active"}
            ],
            "Stop": [
                {"commands": "panopticon internal-hook --event stop"}
            ]
        }
    })
}

/// Settings with hooks in the NEW format (2024+)
pub fn settings_with_new_format_hooks() -> Value {
    json!({
        "hooks": {
            "SessionStart": [
                {
                    "matcher": "",
                    "hooks": [{"type": "command", "command": "panopticon internal-hook --event start"}]
                }
            ],
            "UserPromptSubmit": [
                {
                    "matcher": "",
                    "hooks": [{"type": "command", "command": "panopticon internal-hook --event active"}]
                }
            ],
            "Stop": [
                {
                    "matcher": "",
                    "hooks": [{"type": "command", "command": "panopticon internal-hook --event stop"}]
                }
            ]
        }
    })
}

/// Settings with other (non-panopticon) hooks
pub fn settings_with_other_hooks() -> Value {
    json!({
        "hooks": {
            "SessionStart": [
                {
                    "matcher": "",
                    "hooks": [{"type": "command", "command": "some-other-tool --flag"}]
                }
            ]
        },
        "someOtherSetting": true,
        "anotherSetting": "value"
    })
}

/// Settings with mixed hooks (panopticon + others)
pub fn settings_with_mixed_hooks() -> Value {
    json!({
        "hooks": {
            "SessionStart": [
                {
                    "matcher": "",
                    "hooks": [{"type": "command", "command": "other-tool --init"}]
                },
                {
                    "matcher": "",
                    "hooks": [{"type": "command", "command": "panopticon internal-hook --event start"}]
                }
            ]
        }
    })
}

/// Settings with existing user preferences (should be preserved)
pub fn settings_with_user_preferences() -> Value {
    json!({
        "alwaysThinkingEnabled": true,
        "enabledPlugins": {
            "some-plugin": true
        },
        "permissions": {
            "allow": ["Bash(git:*)"]
        }
    })
}

/// Invalid settings (hooks is not an object)
pub fn invalid_settings_hooks_not_object() -> Value {
    json!({
        "hooks": "not an object"
    })
}

/// Invalid settings (hook type is not an array)
pub fn invalid_settings_hook_type_not_array() -> Value {
    json!({
        "hooks": {
            "SessionStart": "not an array"
        }
    })
}
