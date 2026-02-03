//! OpenClaw session state file parsing
//!
//! Parses `~/.openclaw/agents/<agentId>/sessions/sessions.json` files
//! and transcript headers from `<sessionId>.jsonl` files.
//!
//! # File Formats
//!
//! ## sessions.json
//! ```json
//! {
//!   "agent:default:main": {
//!     "sessionId": "abc123",
//!     "updatedAt": 1706745600000,
//!     "totalTokens": 4700
//!   }
//! }
//! ```
//!
//! ## Transcript header (first line of .jsonl)
//! ```json
//! {"type":"session","id":"abc123","cwd":"/home/user/project"}
//! ```

use serde::Deserialize;
use std::collections::HashMap;

/// Represents the entire sessions.json file content.
///
/// Keys are session identifiers like "agent:default:main".
#[derive(Debug, Deserialize)]
pub struct OpenClawSessionsFile {
    #[serde(flatten)]
    pub sessions: HashMap<String, SessionEntry>,
}

/// A single session entry from sessions.json.
#[derive(Debug, Deserialize)]
pub struct SessionEntry {
    #[serde(rename = "sessionId")]
    pub session_id: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: i64,

    #[serde(rename = "chatType")]
    pub chat_type: Option<String>,

    #[serde(rename = "inputTokens")]
    pub input_tokens: Option<u64>,

    #[serde(rename = "outputTokens")]
    pub output_tokens: Option<u64>,

    #[serde(rename = "totalTokens")]
    pub total_tokens: Option<u64>,

    #[serde(rename = "modelOverride")]
    pub model_override: Option<String>,

    /// Model being used (e.g., "claude-opus-4-5")
    pub model: Option<String>,

    /// Origin information (where the session is running)
    pub origin: Option<SessionOrigin>,

    /// Last channel used (webchat, discord, etc.)
    #[serde(rename = "lastChannel")]
    pub last_channel: Option<String>,
}

/// Origin information for a session - where it's running
#[derive(Debug, Deserialize, Clone)]
pub struct SessionOrigin {
    /// Provider type (webchat, discord, etc.)
    pub provider: Option<String>,

    /// Surface/interface (webchat, discord, slack, etc.)
    pub surface: Option<String>,

    /// Label with more detail (e.g., "openclaw-tui", "discord:channel:123")
    pub label: Option<String>,

    /// Chat type (direct, channel, etc.)
    #[serde(rename = "chatType")]
    pub chat_type: Option<String>,
}

/// Transcript file header (first line of .jsonl files).
///
/// Contains session metadata including the working directory.
#[derive(Debug, Deserialize)]
pub struct TranscriptHeader {
    #[serde(rename = "type")]
    pub entry_type: String,

    pub id: String,

    pub cwd: String,

    /// Timestamp can be either ISO string or Unix ms - we ignore it
    #[serde(default)]
    pub timestamp: serde_json::Value,
}
