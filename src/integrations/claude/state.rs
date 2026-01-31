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

/// Activity statistics for a Claude session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActivityStats {
    pub files_read: u32,
    pub files_edited: u32,
    pub files_written: u32,
    pub commands_run: u32,
    pub searches: u32,
}

/// Information about an active subagent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentInfo {
    pub agent_id: String,
    pub agent_type: String,
    pub started_at: i64,
}

/// Rich activity state for a Claude session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClaudeActivityState {
    /// Current tool being used (if any)
    pub current_tool: Option<String>,
    /// Target of current tool (file, command, pattern)
    pub current_target: Option<String>,
    /// Last user prompt (truncated)
    pub last_prompt: Option<String>,
    /// Model identifier (e.g., "claude-sonnet-4-5-20250929")
    pub model: Option<String>,
    /// Permission mode (default, plan, acceptEdits, etc.)
    pub permission_mode: Option<String>,
    /// Activity counters
    #[serde(default)]
    pub stats: ActivityStats,
    /// Active subagents
    #[serde(default)]
    pub subagents: Vec<SubagentInfo>,
    /// Last error (if any)
    pub last_error: Option<String>,
}

/// State of a single Claude session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeSessionState {
    pub path: String,
    #[serde(default)]
    pub git_branch: Option<String>,
    pub status: String,
    pub last_active: i64, // Unix timestamp in seconds
    /// Rich activity data (optional for backwards compatibility)
    #[serde(default)]
    pub activity: ClaudeActivityState,
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

/// Activity update from a hook event
#[derive(Debug, Clone, Default)]
pub struct ActivityUpdate {
    /// Hook event type
    pub event: String,
    /// Tool name (for tool events)
    pub tool_name: Option<String>,
    /// Tool target (file, command, etc.)
    pub tool_target: Option<String>,
    /// User prompt (for prompt events)
    pub prompt: Option<String>,
    /// Model identifier (for start events)
    pub model: Option<String>,
    /// Permission mode
    pub permission_mode: Option<String>,
    /// Error message (for failure events)
    pub error: Option<String>,
    /// Subagent info (type, id) for subagent events
    pub subagent: Option<(String, String)>,
}

/// Update a single session in the state file (legacy, no activity)
pub fn update_session(
    session_id: &str,
    path: &str,
    git_branch: Option<&str>,
    status: &str,
) -> Result<()> {
    update_session_with_activity(session_id, path, git_branch, status, None)
}

/// Update a single session with rich activity data
pub fn update_session_with_activity(
    session_id: &str,
    path: &str,
    git_branch: Option<&str>,
    status: &str,
    activity_update: Option<ActivityUpdate>,
) -> Result<()> {
    let mut state = read_state().unwrap_or_default();

    let now = Utc::now().timestamp();

    if status == "stop" {
        // Mark as done but keep the session
        if let Some(session) = state.sessions.get_mut(session_id) {
            session.status = "done".to_string();
            session.last_active = now;
            // Clear current tool on stop
            session.activity.current_tool = None;
            session.activity.current_target = None;
            // Update git_branch if provided (in case branch changed)
            if git_branch.is_some() {
                session.git_branch = git_branch.map(|s| s.to_string());
            }
        }
    } else {
        // Get or create session
        let session = state.sessions.entry(session_id.to_string()).or_insert_with(|| {
            ClaudeSessionState {
                path: path.to_string(),
                git_branch: git_branch.map(|s| s.to_string()),
                status: status.to_string(),
                last_active: now,
                activity: ClaudeActivityState::default(),
            }
        });

        // Update basic fields
        session.path = path.to_string();
        // Always refresh git branch to catch user branch switches
        session.git_branch = git_branch.map(|s| s.to_string());
        session.status = status.to_string();
        session.last_active = now;

        // Apply activity update if provided
        if let Some(update) = activity_update {
            apply_activity_update(&mut session.activity, &update, now);
        }
    }

    // Clean up old sessions (older than 7 days)
    let cutoff = now - (7 * 86400);
    state.sessions.retain(|_, s| s.last_active > cutoff);

    write_state(&state)
}

/// Apply an activity update to the session's activity state
fn apply_activity_update(activity: &mut ClaudeActivityState, update: &ActivityUpdate, now: i64) {
    // Update permission mode if provided
    if let Some(mode) = &update.permission_mode {
        activity.permission_mode = Some(mode.clone());
    }

    match update.event.as_str() {
        "start" => {
            // Session start - capture model
            if let Some(model) = &update.model {
                activity.model = Some(model.clone());
            }
            // Reset stats for new session
            activity.stats = ActivityStats::default();
            activity.subagents.clear();
            activity.last_error = None;
        }
        "prompt" => {
            // User submitted prompt - capture it, clear tool state (thinking)
            if let Some(prompt) = &update.prompt {
                activity.last_prompt = Some(truncate_prompt(prompt, 100));
            }
            activity.current_tool = None;
            activity.current_target = None;
            activity.last_error = None;
        }
        "tool_start" => {
            // Tool starting - set current tool
            activity.current_tool = update.tool_name.clone();
            activity.current_target = update.tool_target.clone();
        }
        "tool_done" => {
            // Tool completed - update stats, clear current tool
            if let Some(tool) = &update.tool_name {
                update_stats_for_tool(&mut activity.stats, tool);
            }
            activity.current_tool = None;
            activity.current_target = None;
        }
        "tool_fail" => {
            // Tool failed - record error, clear current tool
            activity.last_error = update.error.clone();
            activity.current_tool = None;
            activity.current_target = None;
        }
        "subagent_start" => {
            // Subagent started
            if let Some((agent_type, agent_id)) = &update.subagent {
                activity.subagents.push(SubagentInfo {
                    agent_id: agent_id.clone(),
                    agent_type: agent_type.clone(),
                    started_at: now,
                });
            }
        }
        "subagent_stop" => {
            // Subagent stopped - remove from list
            if let Some((_, agent_id)) = &update.subagent {
                activity.subagents.retain(|s| s.agent_id != *agent_id);
            }
        }
        _ => {}
    }
}

/// Update activity stats based on tool name
fn update_stats_for_tool(stats: &mut ActivityStats, tool_name: &str) {
    match tool_name {
        "Read" => stats.files_read += 1,
        "Edit" => stats.files_edited += 1,
        "Write" => stats.files_written += 1,
        "Bash" => stats.commands_run += 1,
        "Grep" | "Glob" => stats.searches += 1,
        _ => {}
    }
}

/// Truncate a prompt for storage (in characters)
///
/// Safe for UTF-8: counts characters, not bytes
fn truncate_prompt(prompt: &str, max_len: usize) -> String {
    let first_line = prompt.lines().next().unwrap_or(prompt);
    let char_count = first_line.chars().count();
    if char_count <= max_len {
        return first_line.to_string();
    }

    let prefix_chars = max_len.saturating_sub(3);
    let prefix: String = first_line.chars().take(prefix_chars).collect();
    format!("{}...", prefix)
}

/// Convert state to AgentSessions
/// Deduplicates by working directory, keeping only the most recent session per directory.
/// Also marks sessions as Done if they haven't had activity in 60 minutes.
pub fn sessions_from_state(state: &ClaudeState) -> Vec<AgentSession> {
    let now = Utc::now().timestamp();
    let stale_threshold = 60 * 60; // 60 minutes in seconds

    // First, deduplicate by path - keep only the most recent session per directory
    let mut by_path: HashMap<String, (&String, &ClaudeSessionState)> = HashMap::new();
    for (id, session) in &state.sessions {
        let dominated = by_path
            .get(&session.path)
            .is_some_and(|(_, existing)| existing.last_active >= session.last_active);
        if !dominated {
            by_path.insert(session.path.clone(), (id, session));
        }
    }

    by_path
        .into_values()
        .map(|(id, s)| {
            // Mark as Done if no activity in 30 minutes (likely closed without Stop hook)
            let is_stale = now - s.last_active > stale_threshold;
            let status = match s.status.as_str() {
                "running" | "start" | "active" if is_stale => AgentStatus::Done,
                "running" | "start" | "active" => AgentStatus::Running,
                "idle" => AgentStatus::Idle,
                "waiting" => AgentStatus::WaitingForInput,
                "done" | "stop" => AgentStatus::Done,
                _ => AgentStatus::Idle,
            };

            let last_activity = Utc
                .timestamp_opt(s.last_active, 0)
                .single()
                .unwrap_or_else(Utc::now);

            // Use last_activity as started_at since we don't track session creation time
            let started_at = last_activity;

            AgentSession {
                id: id.clone(),
                agent_type: AgentType::ClaudeCode,
                status,
                working_directory: Some(s.path.clone()),
                git_branch: s.git_branch.clone(),
                last_output: None,
                started_at,
                last_activity,
                window_id: None,
                activity: map_activity_to_data(&s.activity),
            }
        })
        .collect()
}

/// Map state activity to data model activity
fn map_activity_to_data(state: &ClaudeActivityState) -> crate::data::AgentActivity {
    crate::data::AgentActivity {
        current_tool: state.current_tool.clone(),
        current_target: state.current_target.clone(),
        last_prompt: state.last_prompt.clone(),
        model_short: state.model.as_ref().map(|m| extract_model_short(m)),
        permission_mode: state.permission_mode.clone(),
        stats: crate::data::AgentActivityStats {
            files_read: state.stats.files_read,
            files_edited: state.stats.files_edited,
            files_written: state.stats.files_written,
            commands_run: state.stats.commands_run,
        },
        subagent_count: state.subagents.len() as u32,
        last_error: state.last_error.clone(),
        // OpenClaw-specific fields (not applicable to Claude Code)
        surface: None,
        surface_label: None,
        profile: None,
    }
}

/// Extract short model name from full identifier
///
/// "claude-sonnet-4-5-20250929" -> "sonnet"
/// "claude-opus-4-5-20251101" -> "opus"
/// "claude-haiku-..." -> "haiku"
fn extract_model_short(model: &str) -> String {
    if model.contains("opus") {
        "opus".to_string()
    } else if model.contains("sonnet") {
        "sonnet".to_string()
    } else if model.contains("haiku") {
        "haiku".to_string()
    } else {
        // Fallback: first segment after "claude-"
        model
            .strip_prefix("claude-")
            .and_then(|s| s.split('-').next())
            .unwrap_or("claude")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_model_short_sonnet() {
        assert_eq!(extract_model_short("claude-sonnet-4-5-20250929"), "sonnet");
        assert_eq!(extract_model_short("claude-3-5-sonnet-20241022"), "sonnet");
    }

    #[test]
    fn test_extract_model_short_opus() {
        assert_eq!(extract_model_short("claude-opus-4-5-20251101"), "opus");
        assert_eq!(extract_model_short("claude-3-opus-20240229"), "opus");
    }

    #[test]
    fn test_extract_model_short_haiku() {
        assert_eq!(extract_model_short("claude-3-haiku-20240307"), "haiku");
    }

    #[test]
    fn test_extract_model_short_fallback() {
        assert_eq!(extract_model_short("claude-unknown-model"), "unknown");
        assert_eq!(extract_model_short("some-other-model"), "claude");
    }

    #[test]
    fn test_update_stats_for_tool() {
        let mut stats = ActivityStats::default();

        update_stats_for_tool(&mut stats, "Read");
        assert_eq!(stats.files_read, 1);

        update_stats_for_tool(&mut stats, "Edit");
        assert_eq!(stats.files_edited, 1);

        update_stats_for_tool(&mut stats, "Write");
        assert_eq!(stats.files_written, 1);

        update_stats_for_tool(&mut stats, "Bash");
        assert_eq!(stats.commands_run, 1);

        update_stats_for_tool(&mut stats, "Grep");
        assert_eq!(stats.searches, 1);

        update_stats_for_tool(&mut stats, "Glob");
        assert_eq!(stats.searches, 2);

        // Unknown tools don't increment anything
        update_stats_for_tool(&mut stats, "Unknown");
        assert_eq!(stats.files_read, 1);
        assert_eq!(stats.files_edited, 1);
    }

    #[test]
    fn test_apply_activity_update_start() {
        let mut activity = ClaudeActivityState::default();
        let update = ActivityUpdate {
            event: "start".to_string(),
            model: Some("claude-sonnet-4-5-20250929".to_string()),
            ..Default::default()
        };

        apply_activity_update(&mut activity, &update, 0);

        assert_eq!(activity.model, Some("claude-sonnet-4-5-20250929".to_string()));
        // Stats should be reset
        assert_eq!(activity.stats.files_read, 0);
    }

    #[test]
    fn test_apply_activity_update_prompt() {
        let mut activity = ClaudeActivityState {
            current_tool: Some("Read".to_string()),
            current_target: Some("file.rs".to_string()),
            ..Default::default()
        };
        let update = ActivityUpdate {
            event: "prompt".to_string(),
            prompt: Some("Fix the bug".to_string()),
            ..Default::default()
        };

        apply_activity_update(&mut activity, &update, 0);

        assert_eq!(activity.last_prompt, Some("Fix the bug".to_string()));
        // Tool should be cleared (we're now thinking)
        assert!(activity.current_tool.is_none());
        assert!(activity.current_target.is_none());
    }

    #[test]
    fn test_apply_activity_update_tool_start() {
        let mut activity = ClaudeActivityState::default();
        let update = ActivityUpdate {
            event: "tool_start".to_string(),
            tool_name: Some("Read".to_string()),
            tool_target: Some("src/main.rs".to_string()),
            ..Default::default()
        };

        apply_activity_update(&mut activity, &update, 0);

        assert_eq!(activity.current_tool, Some("Read".to_string()));
        assert_eq!(activity.current_target, Some("src/main.rs".to_string()));
    }

    #[test]
    fn test_apply_activity_update_tool_done() {
        let mut activity = ClaudeActivityState {
            current_tool: Some("Read".to_string()),
            current_target: Some("file.rs".to_string()),
            ..Default::default()
        };
        let update = ActivityUpdate {
            event: "tool_done".to_string(),
            tool_name: Some("Read".to_string()),
            ..Default::default()
        };

        apply_activity_update(&mut activity, &update, 0);

        // Tool should be cleared
        assert!(activity.current_tool.is_none());
        assert!(activity.current_target.is_none());
        // Stats should be incremented
        assert_eq!(activity.stats.files_read, 1);
    }

    #[test]
    fn test_apply_activity_update_tool_fail() {
        let mut activity = ClaudeActivityState {
            current_tool: Some("Bash".to_string()),
            ..Default::default()
        };
        let update = ActivityUpdate {
            event: "tool_fail".to_string(),
            error: Some("Command failed".to_string()),
            ..Default::default()
        };

        apply_activity_update(&mut activity, &update, 0);

        assert!(activity.current_tool.is_none());
        assert_eq!(activity.last_error, Some("Command failed".to_string()));
    }

    #[test]
    fn test_apply_activity_update_subagent_start() {
        let mut activity = ClaudeActivityState::default();
        let update = ActivityUpdate {
            event: "subagent_start".to_string(),
            subagent: Some(("Explore".to_string(), "agent-123".to_string())),
            ..Default::default()
        };

        apply_activity_update(&mut activity, &update, 1000);

        assert_eq!(activity.subagents.len(), 1);
        assert_eq!(activity.subagents[0].agent_type, "Explore");
        assert_eq!(activity.subagents[0].agent_id, "agent-123");
    }

    #[test]
    fn test_apply_activity_update_subagent_stop() {
        let mut activity = ClaudeActivityState {
            subagents: vec![SubagentInfo {
                agent_id: "agent-123".to_string(),
                agent_type: "Explore".to_string(),
                started_at: 1000,
            }],
            ..Default::default()
        };
        let update = ActivityUpdate {
            event: "subagent_stop".to_string(),
            subagent: Some(("Explore".to_string(), "agent-123".to_string())),
            ..Default::default()
        };

        apply_activity_update(&mut activity, &update, 2000);

        assert!(activity.subagents.is_empty());
    }

    #[test]
    fn test_truncate_prompt() {
        // Short prompt unchanged
        assert_eq!(truncate_prompt("Hello", 100), "Hello");

        // Long prompt truncated
        let long_prompt = "a".repeat(150);
        let truncated = truncate_prompt(&long_prompt, 100);
        assert!(truncated.len() <= 100);
        assert!(truncated.ends_with("..."));

        // Multi-line: only first line used
        assert_eq!(truncate_prompt("First line\nSecond line", 100), "First line");
    }

    #[test]
    fn test_map_activity_to_data() {
        let state = ClaudeActivityState {
            current_tool: Some("Read".to_string()),
            current_target: Some("file.rs".to_string()),
            last_prompt: Some("Fix bug".to_string()),
            model: Some("claude-sonnet-4-5-20250929".to_string()),
            permission_mode: Some("plan".to_string()),
            stats: ActivityStats {
                files_read: 5,
                files_edited: 2,
                files_written: 1,
                commands_run: 3,
                searches: 4,
            },
            subagents: vec![SubagentInfo {
                agent_id: "a".to_string(),
                agent_type: "Explore".to_string(),
                started_at: 0,
            }],
            last_error: None,
        };

        let data = map_activity_to_data(&state);

        assert_eq!(data.current_tool, Some("Read".to_string()));
        assert_eq!(data.current_target, Some("file.rs".to_string()));
        assert_eq!(data.model_short, Some("sonnet".to_string()));
        assert_eq!(data.permission_mode, Some("plan".to_string()));
        assert_eq!(data.stats.files_read, 5);
        assert_eq!(data.stats.files_edited, 2);
        assert_eq!(data.subagent_count, 1);
    }
}
