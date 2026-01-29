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

/// Container for all Claude session states
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ClaudeState {
    pub sessions: HashMap<String, ClaudeSessionState>,
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
