//! Claude Code session state management
//!
//! Handles atomic read/write of session state to:
//! ~/.local/share/panopticon/claude_state.json

use crate::data::{AgentSession, AgentStatus, AgentType};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;

/// State of a single Claude session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeSessionState {
    pub path: String,
    pub status: String,
    pub last_active: i64, // Unix timestamp in seconds
}

/// Mapping between Linear issues and tmux sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSessionMapping {
    pub issue_identifier: String,
    pub tmux_session_name: String,
    pub working_directory: String,
    pub created_at: i64,
}

/// Container for all Claude session states
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ClaudeState {
    pub sessions: HashMap<String, ClaudeSessionState>,
    #[serde(default)]
    pub issue_sessions: HashMap<String, IssueSessionMapping>,
    #[serde(default)]
    pub recent_directories: Vec<String>,
}

/// Get the path to the state file
pub fn state_file_path() -> Result<PathBuf> {
    let data_dir = directories::ProjectDirs::from("com", "panopticon", "panopticon")
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?
        .data_dir()
        .to_path_buf();

    // Ensure directory exists
    fs::create_dir_all(&data_dir)?;

    Ok(data_dir.join("claude_state.json"))
}

/// Read the current state (with file locking)
pub fn read_state() -> Result<ClaudeState> {
    let path = state_file_path()?;

    if !path.exists() {
        return Ok(ClaudeState::default());
    }

    let file = File::open(&path)?;
    file.lock_shared()?; // Shared lock for reading

    let mut content = String::new();
    let mut reader = std::io::BufReader::new(&file);
    reader.read_to_string(&mut content)?;

    file.unlock()?;

    if content.is_empty() {
        return Ok(ClaudeState::default());
    }

    serde_json::from_str(&content).map_err(|e| anyhow::anyhow!("Failed to parse state: {}", e))
}

/// Write state (with file locking)
pub fn write_state(state: &ClaudeState) -> Result<()> {
    let path = state_file_path()?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = File::create(&path)?;
    file.lock_exclusive()?; // Exclusive lock for writing

    let content = serde_json::to_string_pretty(state)?;
    let mut writer = std::io::BufWriter::new(&file);
    writer.write_all(content.as_bytes())?;

    file.unlock()?;

    Ok(())
}

/// Update a single session in the state file
pub fn update_session(session_id: &str, path: &str, status: &str) -> Result<()> {
    let mut state = read_state().unwrap_or_default();

    let now = Utc::now().timestamp();

    if status == "stop" {
        // Mark as done but keep the session
        if let Some(session) = state.sessions.get_mut(session_id) {
            session.status = "done".to_string();
            session.last_active = now;
        }
    } else {
        state.sessions.insert(
            session_id.to_string(),
            ClaudeSessionState {
                path: path.to_string(),
                status: status.to_string(),
                last_active: now,
            },
        );
    }

    // Clean up old sessions (older than 24 hours)
    let cutoff = now - 86400;
    state.sessions.retain(|_, s| s.last_active > cutoff);

    write_state(&state)
}

/// Convert state to AgentSessions
pub fn sessions_from_state(state: &ClaudeState) -> Vec<AgentSession> {
    state
        .sessions
        .iter()
        .map(|(id, s)| {
            let status = match s.status.as_str() {
                "running" | "start" | "active" => AgentStatus::Running,
                "idle" => AgentStatus::Idle,
                "waiting" => AgentStatus::WaitingForInput,
                "done" | "stop" => AgentStatus::Done,
                _ => AgentStatus::Idle,
            };

            let started_at = Utc
                .timestamp_opt(s.last_active, 0)
                .single()
                .unwrap_or_else(Utc::now);

            AgentSession {
                id: id.clone(),
                agent_type: AgentType::ClaudeCode,
                status,
                working_directory: Some(s.path.clone()),
                last_output: None,
                started_at,
                window_id: None,
            }
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Issue-Session Mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Record a mapping between a Linear issue and a tmux session
pub fn record_issue_session(
    issue_identifier: &str,
    tmux_session_name: &str,
    working_directory: &str,
) -> Result<()> {
    let mut state = read_state().unwrap_or_default();
    let now = Utc::now().timestamp();

    state.issue_sessions.insert(
        issue_identifier.to_string(),
        IssueSessionMapping {
            issue_identifier: issue_identifier.to_string(),
            tmux_session_name: tmux_session_name.to_string(),
            working_directory: working_directory.to_string(),
            created_at: now,
        },
    );

    // Clean up old mappings (older than 7 days)
    let cutoff = now - (7 * 86400);
    state.issue_sessions.retain(|_, m| m.created_at > cutoff);

    write_state(&state)
}

/// Find a tmux session name by working directory
pub fn find_session_by_directory(directory: &str) -> Option<String> {
    let state = read_state().ok()?;
    state
        .issue_sessions
        .values()
        .find(|m| m.working_directory == directory)
        .map(|m| m.tmux_session_name.clone())
}

/// Find a tmux session name by Linear issue identifier
pub fn find_session_by_issue(issue_identifier: &str) -> Option<String> {
    let state = read_state().ok()?;
    state
        .issue_sessions
        .get(issue_identifier)
        .map(|m| m.tmux_session_name.clone())
}

// ─────────────────────────────────────────────────────────────────────────────
// Recent Directories
// ─────────────────────────────────────────────────────────────────────────────

/// Get list of recent directories
pub fn get_recent_directories() -> Result<Vec<String>> {
    let state = read_state()?;
    Ok(state.recent_directories)
}

/// Save a directory to the recent list
pub fn save_recent_directory(directory: &str) -> Result<()> {
    let mut state = read_state().unwrap_or_default();

    // Remove if already exists (to move to front)
    state.recent_directories.retain(|d| d != directory);

    // Add to front
    state.recent_directories.insert(0, directory.to_string());

    // Keep only the 10 most recent
    state.recent_directories.truncate(10);

    write_state(&state)
}
