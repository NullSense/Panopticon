//! Tests for Claude Code hook injection (setup.rs)
//!
//! These tests ensure that panopticon correctly generates hooks in the
//! new Claude Code format and detects existing hooks in both old and new formats.

mod test_utils;

use panopticon::integrations::claude::setup::{
    add_panopticon_hook, generate_hook_entry, has_panopticon_hooks, hooks_installed_at_path,
    inject_hooks_to_path, validate_settings,
};
use serde_json::json;
use test_utils::claude_settings::*;

// ============================================================================
// Hook Format Generation Tests
// ============================================================================

mod hook_generation {
    use super::*;

    #[test]
    fn test_generated_hook_has_matcher_field() {
        let hook = generate_hook_entry("start");
        assert!(
            hook.get("matcher").is_some(),
            "Generated hook should have 'matcher' field"
        );
        assert_eq!(
            hook["matcher"].as_str(),
            Some(""),
            "Matcher should be empty string"
        );
    }

    #[test]
    fn test_generated_hook_has_hooks_array() {
        let hook = generate_hook_entry("start");
        assert!(
            hook.get("hooks").is_some(),
            "Generated hook should have 'hooks' field"
        );
        assert!(
            hook["hooks"].is_array(),
            "'hooks' field should be an array"
        );
        assert_eq!(
            hook["hooks"].as_array().unwrap().len(),
            1,
            "'hooks' array should have one entry"
        );
    }

    #[test]
    fn test_generated_hook_command_has_type_field() {
        let hook = generate_hook_entry("start");
        let inner_hook = &hook["hooks"][0];
        assert_eq!(
            inner_hook["type"].as_str(),
            Some("command"),
            "Inner hook should have type 'command'"
        );
    }

    #[test]
    fn test_generated_hook_command_contains_panopticon() {
        let hook = generate_hook_entry("start");
        let command = hook["hooks"][0]["command"].as_str().unwrap();
        assert!(
            command.contains("panopticon"),
            "Command should contain 'panopticon'"
        );
        assert!(
            command.contains("internal-hook"),
            "Command should contain 'internal-hook'"
        );
    }

    #[test]
    fn test_hook_events_match_types() {
        let start_hook = generate_hook_entry("start");
        let active_hook = generate_hook_entry("active");
        let stop_hook = generate_hook_entry("stop");

        assert!(start_hook["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("--event start"));
        assert!(active_hook["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("--event active"));
        assert!(stop_hook["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("--event stop"));
    }
}

// ============================================================================
// Hook Detection Tests
// ============================================================================

mod hook_detection {
    use super::*;

    #[test]
    fn test_detect_new_format_hooks() {
        let settings = settings_with_new_format_hooks();
        assert!(
            has_panopticon_hooks(&settings),
            "Should detect panopticon hooks in new format"
        );
    }

    #[test]
    fn test_detect_old_format_hooks() {
        let settings = settings_with_old_format_hooks();
        assert!(
            has_panopticon_hooks(&settings),
            "Should detect panopticon hooks in old format"
        );
    }

    #[test]
    fn test_no_false_positive_on_other_commands() {
        let settings = settings_with_other_hooks();
        assert!(
            !has_panopticon_hooks(&settings),
            "Should not detect panopticon hooks when only other hooks present"
        );
    }

    #[test]
    fn test_hooks_installed_returns_false_when_no_file() {
        let env = TestClaudeEnv::new();
        assert!(
            !hooks_installed_at_path(&env.settings_path),
            "Should return false when settings file doesn't exist"
        );
    }

    #[test]
    fn test_hooks_installed_returns_false_when_empty() {
        let env = TestClaudeEnv::new();
        env.write_settings(&empty_settings());
        assert!(
            !hooks_installed_at_path(&env.settings_path),
            "Should return false when settings are empty"
        );
    }

    #[test]
    fn test_hooks_installed_returns_true_with_new_format() {
        let env = TestClaudeEnv::new();
        env.write_settings(&settings_with_new_format_hooks());
        assert!(
            hooks_installed_at_path(&env.settings_path),
            "Should return true when new format hooks exist"
        );
    }

    #[test]
    fn test_hooks_installed_returns_true_with_old_format() {
        let env = TestClaudeEnv::new();
        env.write_settings(&settings_with_old_format_hooks());
        assert!(
            hooks_installed_at_path(&env.settings_path),
            "Should return true when old format hooks exist"
        );
    }
}

// ============================================================================
// Settings Operations Tests
// ============================================================================

mod settings_operations {
    use super::*;

    #[test]
    fn test_inject_creates_settings_if_missing() {
        let env = TestClaudeEnv::new();
        assert!(!env.settings_exists(), "Settings should not exist initially");

        inject_hooks_to_path(&env.settings_path).expect("inject should succeed");

        assert!(env.settings_exists(), "Settings should exist after injection");
    }

    #[test]
    fn test_inject_creates_claude_directory_if_missing() {
        let env = TestClaudeEnv::new();
        assert!(
            !env.claude_dir.exists(),
            ".claude directory should not exist initially"
        );

        inject_hooks_to_path(&env.settings_path).expect("inject should succeed");

        assert!(
            env.claude_dir.exists(),
            ".claude directory should exist after injection"
        );
    }

    #[test]
    fn test_inject_preserves_existing_settings() {
        let env = TestClaudeEnv::new();
        env.write_settings(&settings_with_user_preferences());

        inject_hooks_to_path(&env.settings_path).expect("inject should succeed");

        let settings = env.read_settings();
        assert_eq!(
            settings["alwaysThinkingEnabled"].as_bool(),
            Some(true),
            "alwaysThinkingEnabled should be preserved"
        );
        assert!(
            settings["enabledPlugins"]["some-plugin"].as_bool() == Some(true),
            "enabledPlugins should be preserved"
        );
        assert!(
            settings["permissions"]["allow"].is_array(),
            "permissions should be preserved"
        );
    }

    #[test]
    fn test_inject_preserves_other_hooks() {
        let env = TestClaudeEnv::new();
        env.write_settings(&settings_with_other_hooks());

        inject_hooks_to_path(&env.settings_path).expect("inject should succeed");

        let settings = env.read_settings();
        let session_start = settings["hooks"]["SessionStart"].as_array().unwrap();

        // Should have both the original hook and the new panopticon hook
        assert!(
            session_start.len() >= 2,
            "Should have original hook plus panopticon hook"
        );

        // Check original hook is preserved
        let has_other_hook = session_start.iter().any(|h| {
            h["hooks"]
                .as_array()
                .map(|arr| {
                    arr.iter().any(|inner| {
                        inner["command"]
                            .as_str()
                            .map(|c| c.contains("some-other-tool"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        });
        assert!(has_other_hook, "Original hook should be preserved");
    }

    #[test]
    fn test_inject_handles_invalid_json_by_starting_fresh() {
        let env = TestClaudeEnv::new();
        env.create_claude_dir();
        std::fs::write(&env.settings_path, "not valid json {{{").unwrap();

        inject_hooks_to_path(&env.settings_path).expect("inject should succeed with invalid JSON");

        let settings = env.read_settings();
        assert!(
            settings["hooks"].is_object(),
            "Should have created valid hooks object"
        );
    }

    #[test]
    fn test_all_three_hook_types_generated() {
        let env = TestClaudeEnv::new();

        inject_hooks_to_path(&env.settings_path).expect("inject should succeed");

        let settings = env.read_settings();
        assert!(
            settings["hooks"]["SessionStart"].is_array(),
            "SessionStart hooks should exist"
        );
        assert!(
            settings["hooks"]["UserPromptSubmit"].is_array(),
            "UserPromptSubmit hooks should exist"
        );
        assert!(
            settings["hooks"]["Stop"].is_array(),
            "Stop hooks should exist"
        );
    }
}

// ============================================================================
// Duplication Prevention Tests
// ============================================================================

mod duplication_prevention {
    use super::*;

    #[test]
    fn test_no_duplicate_when_new_format_exists() {
        let env = TestClaudeEnv::new();
        env.write_settings(&settings_with_new_format_hooks());

        inject_hooks_to_path(&env.settings_path).expect("inject should succeed");

        let settings = env.read_settings();
        let session_start = settings["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(
            session_start.len(),
            1,
            "Should not add duplicate hook when new format exists"
        );
    }

    #[test]
    fn test_no_duplicate_when_old_format_exists() {
        let env = TestClaudeEnv::new();
        env.write_settings(&settings_with_old_format_hooks());

        inject_hooks_to_path(&env.settings_path).expect("inject should succeed");

        let settings = env.read_settings();
        let session_start = settings["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(
            session_start.len(),
            1,
            "Should not add duplicate hook when old format exists"
        );
    }

    #[test]
    fn test_ensure_hooks_idempotent() {
        let env = TestClaudeEnv::new();

        // Inject multiple times
        inject_hooks_to_path(&env.settings_path).expect("first inject should succeed");
        inject_hooks_to_path(&env.settings_path).expect("second inject should succeed");
        inject_hooks_to_path(&env.settings_path).expect("third inject should succeed");

        let settings = env.read_settings();
        let session_start = settings["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(
            session_start.len(),
            1,
            "Multiple injections should not create duplicates"
        );
    }

    #[test]
    fn test_add_hook_to_existing_array() {
        let mut hooks = json!({
            "SessionStart": [
                {"matcher": "", "hooks": [{"type": "command", "command": "other-tool"}]}
            ]
        });

        add_panopticon_hook(&mut hooks, "SessionStart", "start");

        let session_start = hooks["SessionStart"].as_array().unwrap();
        assert_eq!(session_start.len(), 2, "Should add hook to existing array");
    }
}

// ============================================================================
// Settings Validation Tests
// ============================================================================

mod validation {
    use super::*;

    #[test]
    fn test_validate_empty_settings() {
        let settings = empty_settings();
        assert!(
            validate_settings(&settings),
            "Empty object should be valid settings"
        );
    }

    #[test]
    fn test_validate_settings_with_hooks() {
        let settings = settings_with_new_format_hooks();
        assert!(
            validate_settings(&settings),
            "Settings with valid hooks should be valid"
        );
    }

    #[test]
    fn test_validate_rejects_non_object() {
        let settings = json!("not an object");
        assert!(
            !validate_settings(&settings),
            "Non-object should be invalid"
        );
    }

    #[test]
    fn test_validate_rejects_hooks_not_object() {
        let settings = invalid_settings_hooks_not_object();
        assert!(
            !validate_settings(&settings),
            "Settings where hooks is not an object should be invalid"
        );
    }

    #[test]
    fn test_validate_rejects_hook_type_not_array() {
        let settings = invalid_settings_hook_type_not_array();
        assert!(
            !validate_settings(&settings),
            "Settings where hook type is not an array should be invalid"
        );
    }
}
