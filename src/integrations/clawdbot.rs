use crate::data::{AgentSession, AgentStatus, AgentType};
use anyhow::Result;
use chrono::{DateTime, Utc};

/// Find Clawdbot/Moltbot sessions using the CLI
///
/// Primary method: `moltbot sessions --json --active 300`
/// Fallback: Read ~/.clawdbot/agents/*/sessions/sessions.json
pub async fn find_all_sessions() -> Result<Vec<AgentSession>> {
    // Try CLI first (most reliable)
    if let Ok(sessions) = find_sessions_via_cli().await {
        if !sessions.is_empty() {
            return Ok(sessions);
        }
    }

    // Fallback to reading session files directly
    find_sessions_via_files().await
}

async fn find_sessions_via_cli() -> Result<Vec<AgentSession>> {
    // Run: moltbot sessions --json --active 300
    let output = tokio::process::Command::new("moltbot")
        .args(["sessions", "--json", "--active", "300"])
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!("moltbot CLI failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sessions_json: serde_json::Value = serde_json::from_str(&stdout)?;

    let mut sessions = Vec::new();

    // Parse the JSON response
    if let Some(arr) = sessions_json.as_array() {
        for session_data in arr {
            if let Some(session) = parse_cli_session(session_data) {
                sessions.push(session);
            }
        }
    }

    Ok(sessions)
}

fn parse_cli_session(data: &serde_json::Value) -> Option<AgentSession> {
    let session_id = data["sessionId"]
        .as_str()
        .or_else(|| data["id"].as_str())?
        .to_string();

    // Get agent/workspace info
    let workspace = data["workspace"]
        .as_str()
        .or_else(|| data["agent"]["workspace"].as_str())
        .map(String::from);

    // Determine status from activity
    let updated_at = data["updatedAt"]
        .as_i64()
        .or_else(|| data["lastActivity"].as_i64())
        .unwrap_or(0);

    let updated_time = DateTime::from_timestamp_millis(updated_at)
        .unwrap_or_else(Utc::now);

    let seconds_since_update = Utc::now()
        .signed_duration_since(updated_time)
        .num_seconds();

    let status = if seconds_since_update < 60 {
        AgentStatus::Running
    } else if seconds_since_update < 300 {
        AgentStatus::Idle
    } else {
        AgentStatus::Done
    };

    Some(AgentSession {
        id: session_id,
        agent_type: AgentType::Clawdbot,
        status,
        working_directory: workspace,
        last_output: data["lastMessage"].as_str().map(|s| {
            if s.len() > 200 {
                format!("…{}", &s[s.len().saturating_sub(200)..])
            } else {
                s.to_string()
            }
        }),
        started_at: updated_time,
        window_id: None,
    })
}

async fn find_sessions_via_files() -> Result<Vec<AgentSession>> {
    let clawdbot_dir = dirs::home_dir()
        .map(|h| h.join(".clawdbot"))
        .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

    if !clawdbot_dir.exists() {
        return Ok(vec![]);
    }

    let mut sessions = Vec::new();

    // Read clawdbot.json for agent list
    let config_path = clawdbot_dir.join("clawdbot.json");
    if !config_path.exists() {
        return Ok(vec![]);
    }

    let config_content = std::fs::read_to_string(&config_path)?;
    let config: serde_json::Value = serde_json::from_str(&config_content)?;

    // Get agents list
    let agents = config["agents"]["list"]
        .as_array()
        .or_else(|| config["agents"].as_array());

    if let Some(agent_list) = agents {
        for agent in agent_list {
            let agent_id = match agent["id"].as_str() {
                Some(id) => id,
                None => continue,
            };
            let workspace = agent["workspace"].as_str();

            // Read sessions for this agent
            let sessions_file = clawdbot_dir
                .join("agents")
                .join(agent_id)
                .join("sessions")
                .join("sessions.json");

            if sessions_file.exists() {
                if let Ok(agent_sessions) = parse_sessions_file(&sessions_file, workspace).await {
                    sessions.extend(agent_sessions);
                }
            }
        }
    }

    Ok(sessions)
}

async fn parse_sessions_file(
    sessions_file: &std::path::PathBuf,
    workspace: Option<&str>,
) -> Result<Vec<AgentSession>> {
    let content = std::fs::read_to_string(sessions_file)?;
    let sessions_json: serde_json::Value = serde_json::from_str(&content)?;

    let mut sessions = Vec::new();

    // sessions.json is an object with session keys
    if let Some(obj) = sessions_json.as_object() {
        for (_key, session_data) in obj {
            let session_id = session_data["sessionId"]
                .as_str()
                .unwrap_or("")
                .to_string();

            if session_id.is_empty() {
                continue;
            }

            let updated_at = session_data["updatedAt"].as_i64().unwrap_or(0);
            let updated_time = DateTime::from_timestamp_millis(updated_at)
                .unwrap_or_else(Utc::now);

            let seconds_since_update = Utc::now()
                .signed_duration_since(updated_time)
                .num_seconds();

            let status = if seconds_since_update < 60 {
                AgentStatus::Running
            } else if seconds_since_update < 300 {
                AgentStatus::Idle
            } else {
                AgentStatus::Done
            };

            sessions.push(AgentSession {
                id: session_id,
                agent_type: AgentType::Clawdbot,
                status,
                working_directory: workspace.map(String::from),
                last_output: None,
                started_at: updated_time,
                window_id: None,
            });
        }
    }

    Ok(sessions)
}

/// Find a Clawdbot session for a given working directory
pub async fn find_session_for_directory(dir: Option<&str>) -> Option<AgentSession> {
    let dir = dir?;
    let sessions = find_all_sessions().await.ok()?;

    sessions
        .into_iter()
        .find(|s| s.working_directory.as_deref() == Some(dir))
}
