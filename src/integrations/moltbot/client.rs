//! HTTP client for Moltbot Gateway API
//!
//! Queries http://127.0.0.1:18789/api/sessions for active sessions

use crate::data::{AgentSession, AgentStatus, AgentType};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use serde::Deserialize;
use std::time::Duration;

/// Default Moltbot Gateway port
pub const DEFAULT_PORT: u16 = 18789;

/// Response from Moltbot sessions endpoint
#[derive(Debug, Deserialize)]
pub struct MoltResponse {
    pub sessions: Vec<MoltSession>,
}

/// A single Moltbot session
#[derive(Debug, Deserialize)]
pub struct MoltSession {
    pub id: String,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(rename = "startTimestamp")]
    pub start_timestamp: Option<i64>,
    #[serde(rename = "lastActivity")]
    pub last_activity: Option<i64>,
    pub status: Option<String>,
    pub workspace: Option<String>,
    #[serde(rename = "workingDirectory")]
    pub working_directory: Option<String>,
}

/// Fetch active sessions from Moltbot Gateway API
pub async fn fetch_sessions(port: Option<u16>) -> Result<Vec<AgentSession>> {
    let port = port.unwrap_or(DEFAULT_PORT);
    let url = format!("http://127.0.0.1:{}/api/sessions", port);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()?;

    let response = client
        .get(&url)
        .query(&[("active", "true")])
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("Moltbot API returned status {}", response.status());
    }

    let molt_response: MoltResponse = response.json().await?;

    let sessions = molt_response
        .sessions
        .into_iter()
        .map(|ms| {
            let id = ms.session_id.unwrap_or(ms.id);

            let status = match ms.status.as_deref() {
                Some("running") | Some("active") => AgentStatus::Running,
                Some("idle") | Some("paused") => AgentStatus::Idle,
                Some("waiting") => AgentStatus::WaitingForInput,
                Some("done") | Some("finished") => AgentStatus::Done,
                Some("error") => AgentStatus::Error,
                _ => AgentStatus::Running, // Default to running for active sessions
            };

            let working_directory = ms.working_directory.or(ms.workspace);

            let started_at = ms
                .start_timestamp
                .or(ms.last_activity)
                .and_then(|ts| Utc.timestamp_millis_opt(ts).single())
                .unwrap_or_else(Utc::now);

            AgentSession {
                id,
                agent_type: AgentType::Clawdbot,
                status,
                working_directory,
                git_branch: None, // Moltbot doesn't track git branches yet
                last_output: None,
                started_at,
                window_id: None,
            }
        })
        .collect();

    Ok(sessions)
}

/// Check if Moltbot daemon is running
#[allow(dead_code)]
pub async fn is_running(port: Option<u16>) -> bool {
    let port = port.unwrap_or(DEFAULT_PORT);
    let url = format!("http://127.0.0.1:{}/api/health", port);

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(200))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    client.get(&url).send().await.is_ok()
}
