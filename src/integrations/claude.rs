use crate::data::{AgentSession, AgentStatus, AgentType};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::time::SystemTime;

/// Find Claude Code sessions by scanning ~/.claude/projects
///
/// Directory structure:
/// ~/.claude/projects/
/// ├── -home-user-project-path/     # Folder name = path with / → -
/// │   ├── {uuid}.jsonl             # Session conversation log
/// │   └── {uuid}/                  # Session directory
pub async fn find_all_sessions() -> Result<Vec<AgentSession>> {
    let claude_dir = dirs::home_dir()
        .map(|h| h.join(".claude").join("projects"))
        .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

    if !claude_dir.exists() {
        return Ok(vec![]);
    }

    let mut sessions = Vec::new();

    // Scan project directories (named like -home-user-path)
    let entries = std::fs::read_dir(&claude_dir)?;
    for entry in entries.flatten() {
        let project_path = entry.path();
        if !project_path.is_dir() {
            continue;
        }

        // Extract working directory from project folder name
        // Convert -home-user-path to /home/user/path
        let project_name = project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let working_dir = if project_name.starts_with('-') {
            Some(project_name.replacen('-', "/", 1).replace('-', "/"))
        } else {
            None
        };

        // Find most recent .jsonl file in this project
        if let Some(session) = find_most_recent_session(&project_path, working_dir).await {
            sessions.push(session);
        }
    }

    // Sort by most recent first
    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));

    Ok(sessions)
}

async fn find_most_recent_session(
    project_dir: &PathBuf,
    working_dir: Option<String>,
) -> Option<AgentSession> {
    let entries = std::fs::read_dir(project_dir).ok()?;

    let mut most_recent: Option<(PathBuf, SystemTime)> = None;

    for entry in entries.flatten() {
        let path = entry.path();

        // Look for .jsonl files (session logs)
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            if let Ok(metadata) = path.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if most_recent.as_ref().map(|(_, t)| modified > *t).unwrap_or(true) {
                        most_recent = Some((path.clone(), modified));
                    }
                }
            }
        }
    }

    let (jsonl_path, modified_time) = most_recent?;
    parse_jsonl_session(&jsonl_path, working_dir, modified_time).await
}

async fn parse_jsonl_session(
    jsonl_path: &PathBuf,
    working_dir: Option<String>,
    modified_time: SystemTime,
) -> Option<AgentSession> {
    // Get session ID from filename (UUID.jsonl -> UUID)
    let session_id = jsonl_path
        .file_stem()
        .and_then(|n| n.to_str())?
        .to_string();

    // Read the file to get session info
    let content = std::fs::read_to_string(jsonl_path).ok()?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return None;
    }

    // Parse first line to get session start info
    let first_line: serde_json::Value = serde_json::from_str(lines.first()?).ok()?;

    // Try to get working directory from first message if not already known
    let cwd = working_dir.or_else(|| {
        first_line["cwd"].as_str().map(String::from)
    });

    // Parse last line to check recent activity
    let last_line: serde_json::Value = lines.last()
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or(serde_json::Value::Null);

    // Determine status based on file modification time and content
    let seconds_since_modified = SystemTime::now()
        .duration_since(modified_time)
        .map(|d| d.as_secs())
        .unwrap_or(u64::MAX);

    let is_process_running = is_claude_process_running().await;

    let status = determine_session_status(
        seconds_since_modified,
        is_process_running,
        &last_line,
    );

    // Extract timestamp from first message for started_at
    let started_at = first_line["timestamp"]
        .as_str()
        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
        .or_else(|| {
            // Fall back to file creation time
            jsonl_path.metadata().ok()
                .and_then(|m| m.created().ok())
                .and_then(|t| {
                    let duration = t.duration_since(SystemTime::UNIX_EPOCH).ok()?;
                    DateTime::from_timestamp(duration.as_secs() as i64, 0)
                })
        })
        .unwrap_or_else(Utc::now);

    // Extract last output from recent assistant messages
    let last_output = extract_last_output(&lines);

    Some(AgentSession {
        id: session_id,
        agent_type: AgentType::ClaudeCode,
        status,
        working_directory: cwd,
        last_output,
        started_at,
        window_id: None,
    })
}

fn determine_session_status(
    seconds_since_modified: u64,
    is_process_running: bool,
    last_line: &serde_json::Value,
) -> AgentStatus {
    // If file hasn't been modified in a while, session is likely done
    if seconds_since_modified > 300 && !is_process_running {
        return AgentStatus::Done;
    }

    // Check last message type for hints
    let last_type = last_line["type"].as_str().unwrap_or("");

    // If last message was from user or contains permission request
    if last_type == "user" {
        if seconds_since_modified < 60 {
            return AgentStatus::Running;
        }
    }

    // Check for permission/approval patterns in content
    let content = last_line["message"]["content"]
        .as_str()
        .or_else(|| last_line["content"].as_str())
        .unwrap_or("");

    if content.contains("permission") || content.contains("approve") || content.contains("allow") {
        return AgentStatus::WaitingForInput;
    }

    // Determine based on recency
    if seconds_since_modified < 30 {
        AgentStatus::Running
    } else if seconds_since_modified < 120 {
        AgentStatus::Idle
    } else if is_process_running {
        AgentStatus::Idle
    } else {
        AgentStatus::Done
    }
}

async fn is_claude_process_running() -> bool {
    // Check for any running Claude Code process
    let output = tokio::process::Command::new("pgrep")
        .args(["-f", "claude"])
        .output()
        .await;

    output.map(|o| o.status.success()).unwrap_or(false)
}

fn extract_last_output(lines: &[&str]) -> Option<String> {
    // Find last assistant message with content
    for line in lines.iter().rev().take(10) {
        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) {
            let msg_type = msg["type"].as_str().unwrap_or("");
            if msg_type == "assistant" {
                let content = msg["message"]["content"]
                    .as_str()
                    .or_else(|| msg["content"].as_str());

                if let Some(text) = content {
                    // Truncate to reasonable length
                    let truncated = if text.len() > 200 {
                        format!("…{}", &text[text.len().saturating_sub(200)..])
                    } else {
                        text.to_string()
                    };
                    return Some(truncated);
                }
            }
        }
    }
    None
}

/// Find a session that matches a given working directory
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

    // Find the project directory that matches this session
    let entries = std::fs::read_dir(&claude_dir)?;
    for entry in entries.flatten() {
        let project_path = entry.path();
        let jsonl_path = project_path.join(format!("{}.jsonl", session.id));

        if jsonl_path.exists() {
            let content = tokio::fs::read_to_string(&jsonl_path).await?;
            let all_lines: Vec<&str> = content.lines().collect();
            let start = all_lines.len().saturating_sub(lines);
            return Ok(all_lines[start..].join("\n"));
        }
    }

    Ok(String::new())
}
