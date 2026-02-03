//! Claude Code integration
//!
//! Event-driven detection of Claude Code sessions via hooks.
//!
//! Architecture:
//! 1. On startup, inject hooks into ~/.claude/settings.json
//! 2. Hooks call `panopticon internal-hook` on session lifecycle events
//! 3. internal-hook updates ~/.local/share/panopticon/claude_state.json
//! 4. File watcher detects changes and updates session list

pub mod hook_input;
pub mod setup;
pub mod state;
pub mod watcher;

use crate::data::AgentSession;
use anyhow::Result;

/// Find all active Claude Code sessions
pub async fn find_all_sessions() -> Result<Vec<AgentSession>> {
    let state = state::read_state().unwrap_or_default();
    Ok(state::sessions_from_state(&state))
}

/// Find a session for a specific working directory
pub async fn find_session_for_directory(dir: Option<&str>) -> Option<AgentSession> {
    let dir = dir?;
    let sessions = find_all_sessions().await.ok()?;

    sessions
        .into_iter()
        .find(|s| s.working_directory.as_deref() == Some(dir))
}

/// Sanitize a string for safe inclusion in a PowerShell `-like` pattern.
/// Removes characters that could be used for injection.
fn sanitize_for_powershell(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' '))
        .collect()
}

/// Focus the terminal window for a Claude session (WSL + Windows)
pub async fn focus_session_window(session: &AgentSession) -> Result<()> {
    let raw_term = session
        .working_directory
        .as_ref()
        .and_then(|d| d.split('/').next_back())
        .unwrap_or(&session.id);

    let search_term = sanitize_for_powershell(raw_term);

    let script = format!(
        r#"
        Add-Type @"
        using System;
        using System.Runtime.InteropServices;
        public class Win32 {{
            [DllImport("user32.dll")]
            public static extern bool SetForegroundWindow(IntPtr hWnd);
        }}
"@
        $procs = Get-Process | Where-Object {{ $_.MainWindowTitle -like "*{}*" }}
        if ($procs) {{
            [Win32]::SetForegroundWindow($procs[0].MainWindowHandle)
        }}
        "#,
        search_term
    );

    let output = tokio::process::Command::new("powershell.exe")
        .args(["-Command", &script])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to focus window: {}", stderr);
    }

    Ok(())
}

/// Initialize Claude integration (call on startup)
pub fn init() -> Result<()> {
    // Ensure hooks are installed
    setup::ensure_hooks()?;
    Ok(())
}

/// Get the current git branch for a directory
fn get_git_branch(dir: &str) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty() && s != "HEAD") // Filter out detached HEAD
}

/// Handle internal hook command (called by Claude hooks)
///
/// Accepts optional HookInput parsed from stdin for rich activity tracking.
pub fn handle_hook(
    event: &str,
    session_id: &str,
    cwd: &str,
    input: Option<&hook_input::HookInput>,
) -> Result<()> {
    let status = event_to_status(event);

    // Capture git branch on every event (keeps branch in sync if user switches)
    let git_branch = get_git_branch(cwd);

    // Build activity update from hook input
    let activity_update = input.map(|i| build_activity_update(event, i));

    state::update_session_with_activity(
        session_id,
        cwd,
        git_branch.as_deref(),
        status,
        activity_update,
    )?;
    Ok(())
}

/// Build an ActivityUpdate from hook input
fn build_activity_update(event: &str, input: &hook_input::HookInput) -> state::ActivityUpdate {
    state::ActivityUpdate {
        event: event.to_string(),
        tool_name: input.tool_name.clone(),
        tool_target: input.tool_target(),
        prompt: input.prompt.clone(),
        model: input.model.clone(),
        permission_mode: input.permission_mode.clone(),
        error: input.error.clone(),
        subagent: input.agent_type.clone().zip(input.agent_id.clone()),
    }
}

/// Map hook event to session status (public for testing)
#[doc(hidden)]
pub fn event_to_status(event: &str) -> &'static str {
    match event {
        "start" => "running",
        "prompt" | "active" => "running",
        "tool_start" | "tool_done" | "tool_fail" => "running",
        "subagent_start" | "subagent_stop" => "running",
        "stop" => "stop",
        _ => "idle",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_to_status_start() {
        assert_eq!(event_to_status("start"), "running");
    }

    #[test]
    fn test_event_to_status_prompt() {
        assert_eq!(event_to_status("prompt"), "running");
    }

    #[test]
    fn test_event_to_status_active_backwards_compat() {
        // "active" was the old event name, kept for backwards compatibility
        assert_eq!(event_to_status("active"), "running");
    }

    #[test]
    fn test_event_to_status_tool_events() {
        assert_eq!(event_to_status("tool_start"), "running");
        assert_eq!(event_to_status("tool_done"), "running");
        assert_eq!(event_to_status("tool_fail"), "running");
    }

    #[test]
    fn test_event_to_status_subagent_events() {
        assert_eq!(event_to_status("subagent_start"), "running");
        assert_eq!(event_to_status("subagent_stop"), "running");
    }

    #[test]
    fn test_event_to_status_stop() {
        assert_eq!(event_to_status("stop"), "stop");
    }

    #[test]
    fn test_event_to_status_unknown_defaults_to_idle() {
        assert_eq!(event_to_status("unknown"), "idle");
        assert_eq!(event_to_status(""), "idle");
        assert_eq!(event_to_status("random_event"), "idle");
    }

    #[test]
    fn test_build_activity_update_tool_event() {
        let input = hook_input::HookInput {
            tool_name: Some("Read".to_string()),
            tool_input: Some(serde_json::json!({
                "file_path": "/home/user/src/main.rs"
            })),
            permission_mode: Some("default".to_string()),
            ..Default::default()
        };

        let update = build_activity_update("tool_start", &input);

        assert_eq!(update.event, "tool_start");
        assert_eq!(update.tool_name, Some("Read".to_string()));
        assert!(update.tool_target.is_some());
        assert_eq!(update.permission_mode, Some("default".to_string()));
    }

    #[test]
    fn test_build_activity_update_prompt_event() {
        let input = hook_input::HookInput {
            prompt: Some("Fix the bug in auth.rs".to_string()),
            model: Some("claude-sonnet-4-5-20250929".to_string()),
            ..Default::default()
        };

        let update = build_activity_update("prompt", &input);

        assert_eq!(update.event, "prompt");
        assert_eq!(update.prompt, Some("Fix the bug in auth.rs".to_string()));
        assert_eq!(update.model, Some("claude-sonnet-4-5-20250929".to_string()));
    }

    #[test]
    fn test_build_activity_update_subagent_event() {
        let input = hook_input::HookInput {
            agent_type: Some("Explore".to_string()),
            agent_id: Some("agent-123".to_string()),
            ..Default::default()
        };

        let update = build_activity_update("subagent_start", &input);

        assert_eq!(update.event, "subagent_start");
        assert_eq!(
            update.subagent,
            Some(("Explore".to_string(), "agent-123".to_string()))
        );
    }

    #[test]
    fn test_build_activity_update_error_event() {
        let input = hook_input::HookInput {
            tool_name: Some("Bash".to_string()),
            error: Some("Command failed with exit code 1".to_string()),
            ..Default::default()
        };

        let update = build_activity_update("tool_fail", &input);

        assert_eq!(update.event, "tool_fail");
        assert_eq!(update.tool_name, Some("Bash".to_string()));
        assert_eq!(
            update.error,
            Some("Command failed with exit code 1".to_string())
        );
    }
}
