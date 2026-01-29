use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A workstream represents a Linear issue and all its linked resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workstream {
    pub linear_issue: LinearIssue,
    pub github_pr: Option<GitHubPR>,
    pub vercel_deployment: Option<VercelDeployment>,
    pub agent_session: Option<AgentSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearIssue {
    pub id: String,
    pub identifier: String, // e.g., "LIN-123"
    pub title: String,
    pub description: Option<String>,
    pub status: LinearStatus,
    pub url: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LinearStatus {
    Backlog,
    Todo,
    InProgress,
    InReview,
    Done,
    Canceled,
}

impl LinearStatus {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Backlog => "Backlog",
            Self::Todo => "Todo",
            Self::InProgress => "In Progress",
            Self::InReview => "In Review",
            Self::Done => "Done",
            Self::Canceled => "Canceled",
        }
    }

    pub fn sort_order(&self) -> u8 {
        match self {
            Self::InProgress => 0,
            Self::InReview => 1,
            Self::Todo => 2,
            Self::Backlog => 3,
            Self::Done => 4,
            Self::Canceled => 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubPR {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub status: GitHubPRStatus,
    pub branch: String,
    pub repo: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitHubPRStatus {
    Draft,
    Open,
    ReviewRequested,
    ChangesRequested,
    Approved,
    Merged,
    Closed,
}

impl GitHubPRStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Draft => "🔵",
            Self::Open => "🟡",
            Self::ReviewRequested => "🟡",
            Self::ChangesRequested => "🟠",
            Self::Approved => "🟢",
            Self::Merged => "🟣",
            Self::Closed => "⚫",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Open => "open",
            Self::ReviewRequested => "review",
            Self::ChangesRequested => "changes",
            Self::Approved => "approved",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VercelDeployment {
    pub id: String,
    pub url: String,
    pub status: VercelStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VercelStatus {
    Queued,
    Building,
    Ready,
    Error,
    Canceled,
}

impl VercelStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Queued => "⏳",
            Self::Building => "🔄",
            Self::Ready => "✅",
            Self::Error => "❌",
            Self::Canceled => "⚫",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub agent_type: AgentType,
    pub status: AgentStatus,
    pub working_directory: Option<String>,
    pub last_output: Option<String>,
    pub started_at: DateTime<Utc>,
    pub window_id: Option<String>, // For teleporting
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentType {
    ClaudeCode,
    Clawdbot,
}

impl AgentType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude",
            Self::Clawdbot => "Clawdbot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Running,
    Idle,
    WaitingForInput,
    Done,
    Error,
}

impl AgentStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Running => "🟢",
            Self::Idle => "🟡",
            Self::WaitingForInput => "🔴",
            Self::Done => "⚪",
            Self::Error => "❌",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Idle => "idle",
            Self::WaitingForInput => "waiting",
            Self::Done => "done",
            Self::Error => "error",
        }
    }
}

/// Sort mode for workstreams
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    #[default]
    ByStatus,
    ByElapsedTime,
    ByPRActivity,
}

impl SortMode {
    pub fn next(&self) -> Self {
        match self {
            Self::ByStatus => Self::ByElapsedTime,
            Self::ByElapsedTime => Self::ByPRActivity,
            Self::ByPRActivity => Self::ByStatus,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::ByStatus => "Status",
            Self::ByElapsedTime => "Elapsed",
            Self::ByPRActivity => "PR Activity",
        }
    }
}

/// Application state
#[derive(Debug, Default)]
pub struct AppState {
    pub workstreams: Vec<Workstream>,
    pub search_query: String,
    pub search_mode: bool,
    pub selected_index: usize,
    pub last_refresh: Option<DateTime<Utc>>,
    pub collapsed_sections: HashSet<LinearStatus>,
    pub sort_mode: SortMode,
}

impl AppState {
    pub fn grouped_workstreams(&self) -> Vec<(LinearStatus, Vec<&Workstream>)> {
        let mut groups: std::collections::HashMap<LinearStatus, Vec<&Workstream>> =
            std::collections::HashMap::new();

        for ws in &self.workstreams {
            groups
                .entry(ws.linear_issue.status)
                .or_default()
                .push(ws);
        }

        // Sort within each group based on sort mode
        for workstreams in groups.values_mut() {
            match self.sort_mode {
                SortMode::ByStatus => {
                    // Default order - sort by issue identifier
                    workstreams.sort_by(|a, b| a.linear_issue.identifier.cmp(&b.linear_issue.identifier));
                }
                SortMode::ByElapsedTime => {
                    // Sort by agent session start time (most recent first)
                    workstreams.sort_by(|a, b| {
                        let a_time = a.agent_session.as_ref().map(|s| s.started_at);
                        let b_time = b.agent_session.as_ref().map(|s| s.started_at);
                        b_time.cmp(&a_time) // Reverse for most recent first
                    });
                }
                SortMode::ByPRActivity => {
                    // Sort by PR status (merged first, then approved, etc.)
                    workstreams.sort_by(|a, b| {
                        let a_pr = a.github_pr.as_ref().map(|p| pr_sort_order(p.status)).unwrap_or(99);
                        let b_pr = b.github_pr.as_ref().map(|p| pr_sort_order(p.status)).unwrap_or(99);
                        a_pr.cmp(&b_pr)
                    });
                }
            }
        }

        let mut result: Vec<_> = groups.into_iter().collect();
        result.sort_by_key(|(status, _)| status.sort_order());
        result
    }
}

fn pr_sort_order(status: GitHubPRStatus) -> u8 {
    match status {
        GitHubPRStatus::Merged => 0,
        GitHubPRStatus::Approved => 1,
        GitHubPRStatus::ChangesRequested => 2,
        GitHubPRStatus::ReviewRequested => 3,
        GitHubPRStatus::Open => 4,
        GitHubPRStatus::Draft => 5,
        GitHubPRStatus::Closed => 6,
    }
}
