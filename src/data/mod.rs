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
    pub priority: LinearPriority,
    pub url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub cycle: Option<LinearCycle>,
    pub labels: Vec<LinearLabel>,
    pub project: Option<String>,
    pub team: Option<String>,
    pub estimate: Option<f32>,
    pub attachments: Vec<LinearAttachment>,
    pub parent: Option<LinearParentRef>,
    pub children: Vec<LinearChildRef>,
}

/// Linear label
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearLabel {
    pub name: String,
    pub color: String,
}

/// Linear cycle (sprint)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearCycle {
    pub id: String,
    pub name: String,
    pub number: i32,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}

/// Linear attachment (document/link)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearAttachment {
    pub id: String,
    pub url: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub source_type: Option<String>,
}

/// Reference to parent issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearParentRef {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub url: String,
}

/// Reference to child/sub-issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearChildRef {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub url: String,
    pub status: LinearStatus,
    pub priority: LinearPriority,
}

/// Linear issue priority (0-4 from API)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LinearPriority {
    #[default]
    NoPriority = 0,
    Urgent = 1,
    High = 2,
    Medium = 3,
    Low = 4,
}

impl LinearPriority {
    /// Create from Linear API integer value (0-4)
    pub fn from_int(value: i64) -> Self {
        match value {
            1 => Self::Urgent,
            2 => Self::High,
            3 => Self::Medium,
            4 => Self::Low,
            _ => Self::NoPriority,
        }
    }

    /// Sort order (lower = higher priority for sorting)
    pub fn sort_order(&self) -> u8 {
        match self {
            Self::Urgent => 0,
            Self::High => 1,
            Self::Medium => 2,
            Self::Low => 3,
            Self::NoPriority => 4,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Urgent => "Urgent",
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
            Self::NoPriority => "None",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Urgent => "Highest priority (red bg)",
            Self::High => "High priority",
            Self::Medium => "Medium priority",
            Self::Low => "Low priority",
            Self::NoPriority => "No priority set",
        }
    }

    pub fn all() -> impl Iterator<Item = Self> {
        [
            Self::Urgent,
            Self::High,
            Self::Medium,
            Self::Low,
            Self::NoPriority,
        ].into_iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LinearStatus {
    Triage,
    Backlog,
    Todo,
    InProgress,
    InReview,
    Done,
    Canceled,
    Duplicate,
}

impl LinearStatus {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Triage => "Triage",
            Self::Backlog => "Backlog",
            Self::Todo => "Todo",
            Self::InProgress => "In Progress",
            Self::InReview => "In Review",
            Self::Done => "Done",
            Self::Canceled => "Canceled",
            Self::Duplicate => "Duplicate",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Triage => "Needs triage/categorization",
            Self::Backlog => "Not yet prioritized",
            Self::Todo => "Ready to start",
            Self::InProgress => "Currently being worked on",
            Self::InReview => "Awaiting review/feedback",
            Self::Done => "Completed",
            Self::Canceled => "No longer needed",
            Self::Duplicate => "Marked as duplicate",
        }
    }

    pub fn sort_order(&self) -> u8 {
        match self {
            Self::InProgress => 0,
            Self::InReview => 1,
            Self::Todo => 2,
            Self::Triage => 3,
            Self::Backlog => 4,
            Self::Done => 5,
            Self::Canceled => 6,
            Self::Duplicate => 7,
        }
    }

    /// Iterator over all status variants in display order
    pub fn all() -> impl Iterator<Item = Self> {
        [
            Self::Triage,
            Self::Backlog,
            Self::Todo,
            Self::InProgress,
            Self::InReview,
            Self::Done,
            Self::Canceled,
            Self::Duplicate,
        ].into_iter()
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
    pub fn label(&self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Open => "Open",
            Self::ReviewRequested => "Review",
            Self::ChangesRequested => "Changes",
            Self::Approved => "Approved",
            Self::Merged => "Merged",
            Self::Closed => "Closed",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Draft => "Work in progress PR",
            Self::Open => "Ready for review",
            Self::ReviewRequested => "Review requested",
            Self::ChangesRequested => "Changes requested",
            Self::Approved => "Ready to merge",
            Self::Merged => "Successfully merged",
            Self::Closed => "Closed without merging",
        }
    }

    pub fn all() -> impl Iterator<Item = Self> {
        [
            Self::Draft,
            Self::Open,
            Self::ReviewRequested,
            Self::ChangesRequested,
            Self::Approved,
            Self::Merged,
            Self::Closed,
        ].into_iter()
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
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Building => "Building",
            Self::Queued => "Queued",
            Self::Error => "Error",
            Self::Canceled => "Canceled",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Ready => "Deployed successfully",
            Self::Building => "Build in progress",
            Self::Queued => "Waiting to build",
            Self::Error => "Deployment failed",
            Self::Canceled => "Deployment canceled",
        }
    }

    pub fn all() -> impl Iterator<Item = Self> {
        [
            Self::Ready,
            Self::Building,
            Self::Queued,
            Self::Error,
            Self::Canceled,
        ].into_iter()
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
    #[allow(dead_code)]
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
    pub fn label(&self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Idle => "Idle",
            Self::WaitingForInput => "Waiting",
            Self::Done => "Done",
            Self::Error => "Error",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Running => "Agent actively working",
            Self::Idle => "Agent paused/waiting",
            Self::WaitingForInput => "Needs your input (!)",
            Self::Done => "Agent finished",
            Self::Error => "Agent encountered error",
        }
    }

    pub fn all() -> impl Iterator<Item = Self> {
        [
            Self::Running,
            Self::Idle,
            Self::WaitingForInput,
            Self::Done,
            Self::Error,
        ].into_iter()
    }
}

/// Sort mode for workstreams
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    #[default]
    ByLinearStatus,
    ByAgentStatus,
    ByVercelStatus,
    ByLastUpdated,
    ByPriority,
    ByPRActivity,
}

/// Visual item for navigation - maps exactly to what's rendered
#[derive(Debug, Clone)]
pub enum VisualItem {
    /// Section header (non-selectable, but included for offset calculation)
    SectionHeader(LinearStatus),
    /// Workstream row (selectable) - contains index into workstreams vec
    Workstream(usize),
}

impl SortMode {
    #[allow(dead_code)]
    pub fn next(&self) -> Self {
        match self {
            Self::ByLinearStatus => Self::ByAgentStatus,
            Self::ByAgentStatus => Self::ByVercelStatus,
            Self::ByVercelStatus => Self::ByLastUpdated,
            Self::ByLastUpdated => Self::ByPriority,
            Self::ByPriority => Self::ByPRActivity,
            Self::ByPRActivity => Self::ByLinearStatus,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::ByLinearStatus => "Linear Status",
            Self::ByAgentStatus => "Agent Status",
            Self::ByVercelStatus => "Vercel Status",
            Self::ByLastUpdated => "Last Updated",
            Self::ByPriority => "Priority",
            Self::ByPRActivity => "PR Activity",
        }
    }

    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            1 => Some(Self::ByAgentStatus),
            2 => Some(Self::ByVercelStatus),
            3 => Some(Self::ByLastUpdated),
            4 => Some(Self::ByPriority),
            5 => Some(Self::ByLinearStatus),
            6 => Some(Self::ByPRActivity),
            _ => None,
        }
    }
}

/// Application state
#[derive(Debug, Default)]
pub struct AppState {
    pub workstreams: Vec<Workstream>,
    pub search_query: String,
    pub search_mode: bool,
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
                SortMode::ByLinearStatus => {
                    // Default order - sort by issue identifier
                    workstreams.sort_by(|a, b| a.linear_issue.identifier.cmp(&b.linear_issue.identifier));
                }
                SortMode::ByAgentStatus => {
                    // Sort by agent status (waiting first, then running, idle, etc.)
                    workstreams.sort_by(|a, b| {
                        let a_status = a.agent_session.as_ref().map(|s| agent_sort_order(s.status)).unwrap_or(99);
                        let b_status = b.agent_session.as_ref().map(|s| agent_sort_order(s.status)).unwrap_or(99);
                        a_status.cmp(&b_status)
                    });
                }
                SortMode::ByVercelStatus => {
                    // Sort by vercel status (error first, then building, ready, etc.)
                    workstreams.sort_by(|a, b| {
                        let a_status = a.vercel_deployment.as_ref().map(|d| vercel_sort_order(d.status)).unwrap_or(99);
                        let b_status = b.vercel_deployment.as_ref().map(|d| vercel_sort_order(d.status)).unwrap_or(99);
                        a_status.cmp(&b_status)
                    });
                }
                SortMode::ByLastUpdated => {
                    // Sort by Linear issue updated_at (most recent first)
                    workstreams.sort_by(|a, b| {
                        b.linear_issue.updated_at.cmp(&a.linear_issue.updated_at)
                    });
                }
                SortMode::ByPriority => {
                    // Sort by priority (urgent first)
                    workstreams.sort_by(|a, b| {
                        a.linear_issue.priority.sort_order().cmp(&b.linear_issue.priority.sort_order())
                    });
                }
                SortMode::ByPRActivity => {
                    // Sort by PR status (changes requested first, then review, etc.)
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

    /// Build visual items list that matches exactly what's rendered
    /// This enables proper j/k navigation through the visual representation
    pub fn build_visual_items(&self, filtered_indices: &[usize]) -> Vec<VisualItem> {
        let mut items = Vec::new();
        let grouped = self.grouped_workstreams();

        for (status, workstreams) in grouped {
            // Add section header
            items.push(VisualItem::SectionHeader(status));

            // Skip items if collapsed
            if self.collapsed_sections.contains(&status) {
                continue;
            }

            // Add workstream items (only if in filtered list)
            for ws in workstreams {
                // Find index in original workstreams vec
                if let Some(idx) = self.workstreams.iter().position(|w| w.linear_issue.id == ws.linear_issue.id) {
                    if filtered_indices.contains(&idx) {
                        items.push(VisualItem::Workstream(idx));
                    }
                }
            }
        }

        items
    }
}

fn pr_sort_order(status: GitHubPRStatus) -> u8 {
    match status {
        // Prioritize items needing attention
        GitHubPRStatus::ChangesRequested => 0,
        GitHubPRStatus::ReviewRequested => 1,
        GitHubPRStatus::Approved => 2,
        GitHubPRStatus::Open => 3,
        GitHubPRStatus::Draft => 4,
        GitHubPRStatus::Merged => 5,
        GitHubPRStatus::Closed => 6,
    }
}

fn agent_sort_order(status: AgentStatus) -> u8 {
    match status {
        // Waiting for input is most urgent
        AgentStatus::WaitingForInput => 0,
        AgentStatus::Error => 1,
        AgentStatus::Running => 2,
        AgentStatus::Idle => 3,
        AgentStatus::Done => 4,
    }
}

fn vercel_sort_order(status: VercelStatus) -> u8 {
    match status {
        // Errors first
        VercelStatus::Error => 0,
        VercelStatus::Building => 1,
        VercelStatus::Queued => 2,
        VercelStatus::Ready => 3,
        VercelStatus::Canceled => 4,
    }
}
