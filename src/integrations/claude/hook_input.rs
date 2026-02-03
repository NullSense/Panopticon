//! Parse JSON input from Claude Code hooks via stdin
//!
//! Claude Code passes rich context to hooks as JSON on stdin.
//! This module parses that input to extract tool usage, prompts, model info, etc.

use serde::Deserialize;

/// Input data from Claude Code hooks (received via stdin as JSON)
///
/// Different hook events populate different fields:
/// - All events: session_id, cwd, permission_mode, hook_event_name
/// - PreToolUse/PostToolUse: tool_name, tool_input
/// - PostToolUseFailure: tool_name, tool_input, error
/// - UserPromptSubmit: prompt
/// - SessionStart: model, source
/// - SubagentStart/Stop: agent_id, agent_type
#[derive(Debug, Clone, Default, Deserialize)]
pub struct HookInput {
    /// Unique session identifier
    pub session_id: Option<String>,

    /// Path to conversation transcript file
    pub transcript_path: Option<String>,

    /// Current working directory
    pub cwd: Option<String>,

    /// Permission mode: "default", "plan", "acceptEdits", "bypassPermissions"
    pub permission_mode: Option<String>,

    /// Hook event name
    pub hook_event_name: Option<String>,

    // Tool-related fields (PreToolUse, PostToolUse, PostToolUseFailure)
    /// Tool name: "Read", "Edit", "Write", "Bash", "Grep", "Glob", "Task", etc.
    pub tool_name: Option<String>,

    /// Tool input parameters (varies by tool)
    pub tool_input: Option<serde_json::Value>,

    /// Tool use ID
    pub tool_use_id: Option<String>,

    // Failure info (PostToolUseFailure)
    /// Error message on tool failure
    pub error: Option<String>,

    /// Whether failure was from user interrupt
    pub is_interrupt: Option<bool>,

    // Prompt (UserPromptSubmit)
    /// User's prompt text
    pub prompt: Option<String>,

    // Session info (SessionStart)
    /// Session source: "startup", "resume", "clear", "compact"
    pub source: Option<String>,

    /// Model identifier (e.g., "claude-sonnet-4-5-20250929")
    pub model: Option<String>,

    // Subagent info (SubagentStart/Stop)
    /// Subagent ID
    pub agent_id: Option<String>,

    /// Subagent type: "Bash", "Explore", "Plan", or custom agent name
    pub agent_type: Option<String>,
}

impl HookInput {
    /// Read and parse hook input from stdin
    ///
    /// Returns None if:
    /// - stdin is a TTY (interactive terminal, no piped input)
    /// - stdin is empty or not valid JSON
    /// - read fails or exceeds size limit
    ///
    /// Uses a size cap to prevent memory exhaustion on malformed input.
    pub fn from_stdin() -> Option<Self> {
        use std::io::{IsTerminal, Read};

        // Bail early if stdin is a TTY (no piped input)
        // This prevents blocking when hook is run manually
        if std::io::stdin().is_terminal() {
            return None;
        }

        let stdin = std::io::stdin();
        let handle = stdin.lock();

        // Read with size limit (1MB max) to prevent memory exhaustion
        const MAX_SIZE: usize = 1024 * 1024;
        let mut buffer = Vec::with_capacity(4096);
        match handle.take(MAX_SIZE as u64).read_to_end(&mut buffer) {
            Ok(0) => return None,
            Err(_) => return None,
            Ok(_) => {}
        }

        // Parse JSON
        serde_json::from_slice(&buffer).ok()
    }

    /// Extract human-readable target from tool_input
    ///
    /// Priority: file_path > command > pattern > url > prompt (for Task)
    pub fn tool_target(&self) -> Option<String> {
        let input = self.tool_input.as_ref()?;

        // File path (Read, Edit, Write)
        if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
            return Some(shorten_path(path));
        }

        // Command (Bash)
        if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
            return Some(truncate(cmd, 40));
        }

        // Pattern (Grep, Glob)
        if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
            return Some(format!("/{}/", truncate(pattern, 30)));
        }

        // URL (WebFetch, WebSearch)
        if let Some(url) = input.get("url").and_then(|v| v.as_str()) {
            return Some(truncate(url, 40));
        }

        // Query (WebSearch)
        if let Some(query) = input.get("query").and_then(|v| v.as_str()) {
            return Some(truncate(query, 40));
        }

        // Description (Task - subagent)
        if let Some(desc) = input.get("description").and_then(|v| v.as_str()) {
            return Some(truncate(desc, 40));
        }

        None
    }
}

/// Shorten a file path for display
///
/// - Replaces home directory with ~
/// - Shows only filename if path is long
fn shorten_path(path: &str) -> String {
    // Replace home directory with ~
    let shortened = if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        if path.starts_with(home_str.as_ref()) {
            path.replacen(home_str.as_ref(), "~", 1)
        } else {
            path.to_string()
        }
    } else {
        path.to_string()
    };

    // If still too long, show just the filename
    if shortened.chars().count() > 50 {
        std::path::Path::new(path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or(shortened)
    } else {
        shortened
    }
}

/// Truncate a string to max length (in characters), adding "..." if truncated
///
/// Safe for UTF-8: counts characters, not bytes
fn truncate(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        return s.to_string();
    }

    let prefix_chars = max_len.saturating_sub(3);
    let prefix: String = s.chars().take(prefix_chars).collect();
    format!("{}...", prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_target_file_path() {
        let input = HookInput {
            tool_input: Some(serde_json::json!({
                "file_path": "/home/user/project/src/main.rs"
            })),
            ..Default::default()
        };
        let target = input.tool_target().unwrap();
        assert!(target.contains("main.rs"));
    }

    #[test]
    fn test_tool_target_command() {
        let input = HookInput {
            tool_input: Some(serde_json::json!({
                "command": "npm test"
            })),
            ..Default::default()
        };
        assert_eq!(input.tool_target().unwrap(), "npm test");
    }

    #[test]
    fn test_tool_target_pattern() {
        let input = HookInput {
            tool_input: Some(serde_json::json!({
                "pattern": "TODO"
            })),
            ..Default::default()
        };
        assert_eq!(input.tool_target().unwrap(), "/TODO/");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a long string", 10), "this is...");
    }

    #[test]
    fn test_truncate_utf8_safe() {
        // Multi-byte UTF-8 characters should not cause panics
        let emoji = "🎉🎊🎈🎁🎀🎃🎄🎆🎇✨";
        let result = truncate(emoji, 5);
        assert_eq!(result.chars().count(), 5); // "🎉🎊..." = 2 emoji + 3 dots
        assert_eq!(result, "🎉🎊...");

        // Japanese text
        let japanese = "こんにちは世界";
        let result = truncate(japanese, 5);
        assert_eq!(result, "こん...");
    }

    #[test]
    fn test_shorten_path_filename_fallback() {
        let long_path = "/very/long/path/that/exceeds/fifty/characters/easily/src/main.rs";
        let shortened = shorten_path(long_path);
        assert_eq!(shortened, "main.rs");
    }
}
