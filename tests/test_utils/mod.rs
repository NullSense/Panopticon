//! Test utilities and fixtures for panopticon tests

use serde_json::{json, Value};

/// Minimal valid issue JSON that should always parse successfully
pub fn minimal_issue_json() -> Value {
    json!({
        "id": "issue-123",
        "identifier": "TEST-1",
        "title": "Test Issue",
        "description": "A test description",
        "url": "https://linear.app/test/issue/TEST-1",
        "createdAt": "2024-01-01T00:00:00Z",
        "updatedAt": "2024-01-02T00:00:00Z",
        "priority": 2,
        "estimate": null,
        "state": {
            "name": "In Progress",
            "type": "started"
        },
        "cycle": null,
        "labels": { "nodes": [] },
        "project": null,
        "team": { "name": "Test Team", "key": "TEST" },
        "attachments": { "nodes": [] },
        "parent": null,
        "children": { "nodes": [] },
        "branchName": "test-branch"
    })
}

/// Wrapper around the internal parse function for testing
/// This is a simplified version that mimics the parsing logic
pub fn parse_issue(node: &Value) -> Option<ParsedIssue> {
    use chrono::Utc;

    let state_type = node["state"]["type"].as_str()?;
    let status = match state_type {
        "backlog" => LinearStatus::Backlog,
        "unstarted" => LinearStatus::Todo,
        "started" => LinearStatus::InProgress,
        "completed" => LinearStatus::Done,
        "canceled" => LinearStatus::Canceled,
        _ => {
            if node["state"]["name"]
                .as_str()
                .map(|s| s.to_lowercase().contains("review"))
                .unwrap_or(false)
            {
                LinearStatus::InReview
            } else {
                LinearStatus::InProgress
            }
        }
    };

    // Parse priority
    let priority = node["priority"]
        .as_i64()
        .map(LinearPriority::from_int)
        .unwrap_or_default();

    // Parse cycle (safely - no ? operator)
    let cycle = if node["cycle"].is_object() {
        let c = &node["cycle"];
        c["id"].as_str().map(|id| LinearCycle {
            id: id.to_string(),
            name: c["name"].as_str().unwrap_or("Unnamed").to_string(),
            number: c["number"].as_i64().unwrap_or(0) as i32,
            starts_at: c["startsAt"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(Utc::now),
            ends_at: c["endsAt"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(Utc::now),
        })
    } else {
        None
    };

    // Parse labels
    let labels = node["labels"]["nodes"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|l| {
                    Some(LinearLabel {
                        name: l["name"].as_str()?.to_string(),
                        color: l["color"].as_str().unwrap_or("#888888").to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Parse attachments
    let pr_url = node["attachments"]["nodes"]
        .as_array()
        .and_then(|attachments| {
            attachments.iter().find_map(|a| {
                let url = a["url"].as_str()?;
                if url.contains("github.com") && url.contains("/pull/") {
                    Some(url.to_string())
                } else {
                    None
                }
            })
        });

    let attachments = node["attachments"]["nodes"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let url = a["url"].as_str()?;
                    if url.contains("github.com") && url.contains("/pull/") {
                        return None;
                    }
                    Some(LinearAttachment {
                        id: a["id"].as_str().unwrap_or("").to_string(),
                        url: url.to_string(),
                        title: a["title"].as_str().unwrap_or("Untitled").to_string(),
                        subtitle: a["subtitle"].as_str().map(String::from),
                        source_type: a["sourceType"].as_str().map(String::from),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Parse parent (safely)
    let parent = if node["parent"].is_object() {
        let p = &node["parent"];
        match (p["id"].as_str(), p["identifier"].as_str()) {
            (Some(id), Some(identifier)) => Some(LinearParentRef {
                id: id.to_string(),
                identifier: identifier.to_string(),
                title: p["title"].as_str().unwrap_or("").to_string(),
                url: p["url"].as_str().unwrap_or("").to_string(),
            }),
            _ => None,
        }
    } else {
        None
    };

    // Parse children
    let children = node["children"]["nodes"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let child_state_type = c["state"]["type"].as_str()?;
                    let child_status = match child_state_type {
                        "backlog" => LinearStatus::Backlog,
                        "unstarted" => LinearStatus::Todo,
                        "started" => LinearStatus::InProgress,
                        "completed" => LinearStatus::Done,
                        "canceled" => LinearStatus::Canceled,
                        _ => LinearStatus::InProgress,
                    };
                    let child_priority = c["priority"]
                        .as_i64()
                        .map(LinearPriority::from_int)
                        .unwrap_or_default();

                    Some(LinearChildRef {
                        id: c["id"].as_str()?.to_string(),
                        identifier: c["identifier"].as_str()?.to_string(),
                        title: c["title"].as_str().unwrap_or("").to_string(),
                        url: c["url"].as_str().unwrap_or("").to_string(),
                        status: child_status,
                        priority: child_priority,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let issue = LinearIssue {
        id: node["id"].as_str()?.to_string(),
        identifier: node["identifier"].as_str()?.to_string(),
        title: node["title"].as_str()?.to_string(),
        description: node["description"].as_str().map(String::from),
        status,
        priority,
        url: node["url"].as_str()?.to_string(),
        created_at: node["createdAt"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
        updated_at: node["updatedAt"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
        cycle,
        labels,
        project: node["project"]["name"].as_str().map(String::from),
        team: node["team"]["name"].as_str().map(String::from),
        estimate: node["estimate"].as_f64().map(|e| e as f32),
        attachments,
        parent,
        children,
    };

    Some(ParsedIssue {
        issue,
        linked_pr_url: pr_url,
        working_directory: node["branchName"].as_str().map(String::from),
    })
}

// Re-create the data types for testing (to avoid depending on internal modules)
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    pub fn from_int(value: i64) -> Self {
        match value {
            1 => Self::Urgent,
            2 => Self::High,
            3 => Self::Medium,
            4 => Self::Low,
            _ => Self::NoPriority,
        }
    }
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearCycle {
    pub id: String,
    pub name: String,
    pub number: i32,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearLabel {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearAttachment {
    pub id: String,
    pub url: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub source_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearParentRef {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearChildRef {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub url: String,
    pub status: LinearStatus,
    pub priority: LinearPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearIssue {
    pub id: String,
    pub identifier: String,
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

#[derive(Debug, Clone)]
pub struct ParsedIssue {
    pub issue: LinearIssue,
    pub linked_pr_url: Option<String>,
    pub working_directory: Option<String>,
}
