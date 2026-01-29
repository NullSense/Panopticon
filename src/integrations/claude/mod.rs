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

/// Focus the terminal window for a Claude session (WSL + Windows)
pub async fn focus_session_window(session: &AgentSession) -> Result<()> {
    // Use PowerShell to focus the Alacritty window
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
