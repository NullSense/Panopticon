use crate::data::{AgentSession, AgentStatus, AgentType};
use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;

/// Find Claude Code sessions by scanning ~/.claude/projects
pub async fn find_all_sessions() -> Result<Vec<AgentSession>> {
    let claude_dir = dirs::home_dir()
        .map(|h| h.join(".claude").join("projects"))
        .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

    if !claude_dir.exists() {
        return Ok(vec![]);
    }

    let mut sessions = Vec::new();

    // Scan project directories
    let entries = std::fs::read_dir(&claude_dir)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(session) = parse_claude_session(&path).await {
                sessions.push(session);
            }
        }
    }

    Ok(sessions)
}

/// Find a session that matches a given working directory
pub async fn find_session_for_directory(dir: Option<&str>) -> Option<AgentSession> {
    let dir = dir?;
    let sessions = find_all_sessions().await.ok()?;

    sessions
        .into_iter()
        .find(|s| s.working_directory.as_deref() == Some(dir))
}

async fn parse_claude_session(project_dir: &PathBuf) -> Option<AgentSession> {
    // Look for session state file
    let state_file = project_dir.join("session.json");
    if !state_file.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&state_file).ok()?;
    let state: serde_json::Value = serde_json::from_str(&content).ok()?;

    let id = state["id"].as_str()?.to_string();
    let working_dir = state["workingDirectory"].as_str().map(String::from);

    // Determine status based on process state
    let status = if is_process_running(&id).await {
        // Check if waiting for input
        if state["waitingForInput"].as_bool().unwrap_or(false) {
            AgentStatus::WaitingForInput
        } else if state["lastActivity"]
            .as_str()
            .and_then(|s| s.parse::<chrono::DateTime<Utc>>().ok())
            .map(|t| Utc::now().signed_duration_since(t).num_seconds() > 30)
            .unwrap_or(false)
        {
            AgentStatus::Idle
        } else {
            AgentStatus::Running
        }
    } else {
        AgentStatus::Done
    };

    let last_output = state["lastOutput"].as_str().map(|s| {
        // Truncate to last 200 chars
        if s.len() > 200 {
            format!("...{}", &s[s.len() - 200..])
        } else {
            s.to_string()
        }
    });

    Some(AgentSession {
        id,
        agent_type: AgentType::ClaudeCode,
        status,
        working_directory: working_dir,
        last_output,
        started_at: state["startedAt"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
        window_id: state["windowId"].as_str().map(String::from),
    })
}

async fn is_process_running(session_id: &str) -> bool {
    // Check if there's a running process associated with this session
    // This is a simplified check - in reality we'd use process inspection
    let output = tokio::process::Command::new("pgrep")
        .args(["-f", &format!("claude.*{}", session_id)])
        .output()
        .await;

    output.map(|o| o.status.success()).unwrap_or(false)
}

/// Focus the terminal window for a Claude session (WSL + Windows)
pub async fn focus_session_window(session: &AgentSession) -> Result<()> {
    // Use PowerShell to focus the Alacritty window
    // This requires the window title to contain identifiable info

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

/// Get recent output from a Claude session (for preview mode)
pub async fn get_session_output(session: &AgentSession, lines: usize) -> Result<String> {
    let claude_dir = dirs::home_dir()
        .map(|h| h.join(".claude").join("projects"))
        .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

    let output_file = claude_dir.join(&session.id).join("output.log");

    if !output_file.exists() {
        return Ok(String::new());
    }

    let content = tokio::fs::read_to_string(&output_file).await?;
    let output_lines: Vec<&str> = content.lines().collect();
    let start = output_lines.len().saturating_sub(lines);

    Ok(output_lines[start..].join("\n"))
}
