//! Claude Code integration
//!
//! Event-driven detection of Claude Code sessions via hooks.
//!
//! Architecture:
//! 1. On startup, inject hooks into ~/.claude/settings.json
//! 2. Hooks call `panopticon internal-hook` on session lifecycle events
//! 3. internal-hook updates ~/.local/share/panopticon/claude_state.json
//! 4. File watcher detects changes and updates session list

pub mod setup;
pub mod state;
pub mod watcher;

use crate::data::AgentSession;
use anyhow::Result;
use std::process::Command;

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

/// Focus the terminal window for a Claude session
/// Uses tmux switch-client if running inside tmux, otherwise falls back to WSL/Windows
pub async fn focus_session_window(session: &AgentSession) -> Result<()> {
    // First, try to find if there's a tmux session for this working directory
    if let Some(dir) = &session.working_directory {
        // Check if we can find an issue session mapping
        if let Some(session_name) = state::find_session_by_directory(dir) {
            if tmux_session_exists(&session_name) {
                return focus_tmux_session(&session_name);
            }
        }
    }

    // Try to switch to tmux session by session ID
    if tmux_session_exists(&session.id) {
        return focus_tmux_session(&session.id);
    }

    // Fallback: Use PowerShell to focus the Alacritty window (WSL + Windows)
    let search_term = session
        .working_directory
        .as_ref()
        .and_then(|d| d.split('/').last())
        .unwrap_or(&session.id);

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

// ─────────────────────────────────────────────────────────────────────────────
// tmux integration
// ─────────────────────────────────────────────────────────────────────────────

/// Check if tmux is available on the system
pub fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if a tmux session with the given name exists
pub fn tmux_session_exists(session_name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Focus/switch to a tmux session
pub fn focus_tmux_session(session_name: &str) -> Result<()> {
    // Check if we're inside tmux
    let inside_tmux = std::env::var("TMUX").is_ok();

    if inside_tmux {
        // Switch client to the target session
        let output = Command::new("tmux")
            .args(["switch-client", "-t", session_name])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to switch to tmux session: {}", stderr);
        }
    } else {
        // Attach to the session (will fail if not in a terminal)
        let output = Command::new("tmux")
            .args(["attach-session", "-t", session_name])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to attach to tmux session: {}", stderr);
        }
    }

    Ok(())
}

/// Spawn a Claude agent in a new tmux session
pub async fn spawn_agent_session(
    identifier: &str,
    title: &str,
    description: Option<&str>,
    working_dir: &str,
) -> Result<()> {
    // Create the tmux session
    let output = Command::new("tmux")
        .args([
            "new-session",
            "-d",                    // detached
            "-s", identifier,        // session name
            "-c", working_dir,       // working directory
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to create tmux session: {}", stderr);
    }

    // Start Claude in the session
    let output = Command::new("tmux")
        .args(["send-keys", "-t", identifier, "claude", "Enter"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to start claude: {}", stderr);
    }

    // Wait a moment for Claude to initialize
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // Build the initial prompt
    let desc_text = description
        .map(|d| {
            // Truncate description to 2000 chars
            let truncated: String = d.chars().take(2000).collect();
            format!("\n\nDescription:\n{}", truncated)
        })
        .unwrap_or_default();

    let prompt = format!(
        "You are working on Linear issue {}:\n\nTitle: {}{}\n\nPlease analyze the requirements and propose an implementation plan.",
        identifier, title, desc_text
    );

    // Escape the prompt for tmux send-keys (handle special characters)
    let escaped_prompt = prompt
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`");

    // Send the prompt to Claude
    let output = Command::new("tmux")
        .args(["send-keys", "-t", identifier, &escaped_prompt, "Enter"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to send prompt to claude: {}", stderr);
    }

    Ok(())
}

/// Initialize Claude integration (call on startup)
pub fn init() -> Result<()> {
    // Ensure hooks are installed
    setup::ensure_hooks()?;
    Ok(())
}

/// Handle internal hook command (called by Claude hooks)
pub fn handle_hook(event: &str, session_id: &str, cwd: &str) -> Result<()> {
    let status = match event {
        "start" => "running",
        "active" => "running",
        "stop" => "done",
        _ => "idle",
    };

    state::update_session(session_id, cwd, status)?;
    Ok(())
}
