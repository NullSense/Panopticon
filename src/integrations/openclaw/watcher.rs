//! File watcher for OpenClaw session state changes
//!
//! Uses the notify crate to watch `~/.openclaw/agents/*/sessions/sessions.json`
//! for changes. Provides real-time updates when OpenClaw sessions start/stop.
//!
//! # Architecture
//!
//! ```text
//! ~/.openclaw/agents/
//! ├── agent1/
//! │   └── sessions/
//! │       ├── sessions.json    <- watched
//! │       └── <sessionId>.jsonl <- transcript (read for cwd)
//! └── agent2/
//!     └── sessions/
//!         └── ...
//! ```

use super::state::{OpenClawSessionsFile, TranscriptHeader};
use super::status::infer_status;
use crate::data::{AgentActivity, AgentSession, AgentType};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Default OpenClaw state directory
fn default_state_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".openclaw/agents"))
        .unwrap_or_else(|| PathBuf::from("/tmp/.openclaw/agents"))
}

/// Get the OpenClaw state directory (respects $OPENCLAW_STATE_DIR)
pub fn state_dir() -> PathBuf {
    std::env::var("OPENCLAW_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_state_dir())
}

/// Watcher that maintains current OpenClaw sessions from file system
pub struct OpenClawWatcher {
    sessions: Arc<RwLock<Vec<AgentSession>>>,
    _watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
    watch_path: PathBuf,
}

impl OpenClawWatcher {
    /// Create a new watcher using the default state directory
    #[allow(dead_code)]
    pub fn new() -> Result<Self> {
        Self::new_with_path(&state_dir())
    }

    /// Create a new watcher for a specific directory (used for testing)
    pub fn new_with_path(path: &Path) -> Result<Self> {
        let sessions = Arc::new(RwLock::new(Vec::new()));
        let watch_path = path.to_path_buf();

        // Initial load
        let initial = load_all_sessions(path);
        match sessions.write() {
            Ok(mut guard) => *guard = initial,
            Err(e) => tracing::warn!("OpenClaw sessions lock poisoned on init: {e}"),
        }

        let (tx, rx) = channel();

        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(1)),
        )?;

        // Watch recursively to catch all agent directories
        if path.exists() {
            let _ = watcher.watch(path, RecursiveMode::Recursive);
        }

        Ok(Self {
            sessions,
            _watcher: watcher,
            receiver: rx,
            watch_path,
        })
    }

    /// Poll for changes and update sessions
    ///
    /// Debounces multiple events - only reads state once even if files changed multiple times.
    pub fn poll(&self) -> bool {
        let mut has_events = false;

        loop {
            match self.receiver.try_recv() {
                Ok(Ok(_event)) => {
                    has_events = true;
                }
                Ok(Err(_)) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

        if has_events {
            let new_sessions = load_all_sessions(&self.watch_path);
            match self.sessions.write() {
                Ok(mut guard) => {
                    *guard = new_sessions;
                    return true;
                }
                Err(e) => tracing::warn!("OpenClaw sessions lock poisoned on poll: {e}"),
            }
        }

        false
    }

    /// Get a snapshot of current sessions (clones the data)
    pub fn get_sessions_snapshot(&self) -> Vec<AgentSession> {
        self.sessions.read().map(|g| g.clone()).unwrap_or_default()
    }
}

/// Load all sessions from all agent directories (one-shot, no watcher)
pub fn load_all_sessions(base_path: &Path) -> Vec<AgentSession> {
    let mut all_sessions = Vec::new();

    let entries = match fs::read_dir(base_path) {
        Ok(e) => e,
        Err(_) => return all_sessions,
    };

    for entry in entries.flatten() {
        let agent_path = entry.path();
        if agent_path.is_dir() {
            // Extract profile name from directory (e.g., "main", "personal", "work")
            let profile = agent_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());

            let sessions_dir = agent_path.join("sessions");
            if let Ok(sessions) = load_agent_sessions(&sessions_dir, profile.as_deref()) {
                all_sessions.extend(sessions);
            }
        }
    }

    all_sessions
}

/// Load sessions from a single agent's sessions directory
fn load_agent_sessions(sessions_dir: &Path, profile: Option<&str>) -> Result<Vec<AgentSession>> {
    let sessions_file = sessions_dir.join("sessions.json");

    let content = fs::read_to_string(&sessions_file)?;
    let parsed: OpenClawSessionsFile = serde_json::from_str(&content)?;

    let now = Utc::now();
    let mut sessions = Vec::new();

    // Build transcript cache for working directory lookup
    let transcripts = load_transcript_headers(sessions_dir);

    for (_key, entry) in parsed.sessions {
        let updated_at = Utc
            .timestamp_millis_opt(entry.updated_at)
            .single()
            .unwrap_or_else(Utc::now);

        let status = infer_status(updated_at, now);

        // Look up working directory from transcript
        let working_directory = transcripts.get(&entry.session_id).map(|h| h.cwd.clone());

        // Extract git branch from working directory
        let git_branch = working_directory
            .as_ref()
            .and_then(|dir| extract_git_branch(dir));

        // Extract model short name (e.g., "claude-opus-4-5" -> "opus")
        let model_short = entry.model.as_deref().and_then(extract_model_short);

        // Extract surface info from origin (webchat, discord, etc.)
        let (surface, surface_label) = entry
            .origin
            .as_ref()
            .map(|o| {
                let surface = o.surface.clone();
                // Clean up label - extract meaningful part
                let label = o.label.as_ref().map(|l| format_surface_label(l));
                (surface, label)
            })
            .unwrap_or((None, None));

        let activity = AgentActivity {
            model_short,
            surface,
            surface_label,
            profile: profile.map(|s| s.to_string()),
            ..Default::default()
        };

        sessions.push(AgentSession {
            id: entry.session_id,
            agent_type: AgentType::OpenClaw,
            status,
            working_directory,
            git_branch,
            last_output: None,
            started_at: updated_at,
            last_activity: updated_at,
            window_id: None,
            activity,
        });
    }

    Ok(sessions)
}

/// Format surface label for display
///
/// Cleans up raw labels like "openclaw-tui" or "discord:channel:123456"
fn format_surface_label(label: &str) -> String {
    if label == "openclaw-tui" {
        return "TUI".to_string();
    }
    if label.starts_with("discord:channel:") {
        return "Discord channel".to_string();
    }
    if label.contains("user id:") {
        // Extract username from "username user id:123"
        if let Some(name) = label.split(" user id:").next() {
            return format!("DM: {}", name);
        }
    }
    if label.contains("Guild #") {
        // Extract channel name from "Guild #channel-name..."
        if let Some(start) = label.find("Guild #") {
            let rest = &label[start + 7..];
            if let Some(end) = rest.find(" channel id:") {
                return format!("#{}", &rest[..end]);
            }
        }
    }
    // Return as-is if no pattern matches
    label.to_string()
}

/// Load transcript headers from .jsonl files for working directory lookup
fn load_transcript_headers(sessions_dir: &Path) -> HashMap<String, TranscriptHeader> {
    let mut headers = HashMap::new();

    let entries = match fs::read_dir(sessions_dir) {
        Ok(e) => e,
        Err(_) => return headers,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "jsonl") {
            if let Some(header) = read_transcript_header(&path) {
                headers.insert(header.id.clone(), header);
            }
        }
    }

    headers
}

/// Read the first line of a transcript file to get the header
///
/// Uses BufReader to avoid reading entire file (can be 20MB+) into memory.
fn read_transcript_header(path: &Path) -> Option<TranscriptHeader> {
    use std::io::{BufRead, BufReader};

    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    reader.read_line(&mut first_line).ok()?;
    serde_json::from_str(&first_line).ok()
}

/// Extract git branch from working directory
fn extract_git_branch(dir: &str) -> Option<String> {
    let git_head = Path::new(dir).join(".git/HEAD");
    let content = fs::read_to_string(git_head).ok()?;

    // Format: "ref: refs/heads/branch-name"
    content
        .strip_prefix("ref: refs/heads/")
        .map(|s| s.trim().to_string())
}

/// Extract short model name from full model identifier
///
/// Examples:
/// - "claude-opus-4-5" -> "opus"
/// - "claude-sonnet-4" -> "sonnet"
/// - "anthropic/claude-opus-4-5" -> "opus"
fn extract_model_short(model: &str) -> Option<String> {
    let model_lower = model.to_lowercase();
    if model_lower.contains("opus") {
        Some("opus".to_string())
    } else if model_lower.contains("sonnet") {
        Some("sonnet".to_string())
    } else if model_lower.contains("haiku") {
        Some("haiku".to_string())
    } else {
        // Return the last part after any slashes
        model.rsplit('/').next().map(|s| s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_dir_default() {
        std::env::remove_var("OPENCLAW_STATE_DIR");
        let path = state_dir();
        assert!(path.to_string_lossy().contains(".openclaw/agents"));
    }

    #[test]
    fn test_state_dir_from_env() {
        std::env::set_var("OPENCLAW_STATE_DIR", "/custom/path");
        let path = state_dir();
        assert_eq!(path, PathBuf::from("/custom/path"));
        std::env::remove_var("OPENCLAW_STATE_DIR");
    }

    #[test]
    fn test_extract_git_branch() {
        // Can't easily test without actual .git directory
        // Just verify the function signature works
        let result = extract_git_branch("/nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_model_short_opus() {
        assert_eq!(
            extract_model_short("claude-opus-4-5"),
            Some("opus".to_string())
        );
        assert_eq!(
            extract_model_short("anthropic/claude-opus-4-5"),
            Some("opus".to_string())
        );
    }

    #[test]
    fn test_extract_model_short_sonnet() {
        assert_eq!(
            extract_model_short("claude-sonnet-4"),
            Some("sonnet".to_string())
        );
    }

    #[test]
    fn test_extract_model_short_haiku() {
        assert_eq!(
            extract_model_short("claude-haiku-3"),
            Some("haiku".to_string())
        );
    }

    #[test]
    fn test_extract_model_short_unknown() {
        assert_eq!(
            extract_model_short("some/other-model"),
            Some("other-model".to_string())
        );
    }

    #[test]
    fn test_format_surface_label_tui() {
        assert_eq!(format_surface_label("openclaw-tui"), "TUI");
    }

    #[test]
    fn test_format_surface_label_discord_channel() {
        assert_eq!(
            format_surface_label("discord:channel:1234567890"),
            "Discord channel"
        );
    }

    #[test]
    fn test_format_surface_label_dm() {
        assert_eq!(
            format_surface_label("username user id:1234567890"),
            "DM: username"
        );
    }

    #[test]
    fn test_format_surface_label_guild_channel() {
        assert_eq!(
            format_surface_label("Guild #general channel id:1234567890"),
            "#general"
        );
    }

    #[test]
    fn test_format_surface_label_unknown() {
        assert_eq!(format_surface_label("something-else"), "something-else");
    }
}
