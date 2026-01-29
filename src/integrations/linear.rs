use crate::config::Config;
use crate::data::{
    LinearAttachment, LinearChildRef, LinearCycle, LinearIssue, LinearLabel,
    LinearParentRef, LinearPriority, LinearStatus,
};
use crate::integrations::{LinkedLinearIssue, HTTP_CLIENT};
use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;

const LINEAR_API_URL: &str = "https://api.linear.app/graphql";

// Type-safe API response structures for Linear GraphQL API
#[derive(Debug, Deserialize)]
struct GraphQLResponse<T> {
    data: Option<T>,
    #[allow(dead_code)]
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    #[allow(dead_code)]
    message: String,
}

#[derive(Debug, Deserialize)]
struct ViewerData {
    viewer: Viewer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Viewer {
    assigned_issues: IssueConnection,
}

#[derive(Debug, Deserialize)]
struct IssueConnection {
    nodes: Vec<IssueNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssueNode {
    id: String,
    identifier: String,
    title: String,
    description: Option<String>,
    url: String,
    updated_at: String,
    created_at: String,
    priority: Option<i64>,
    estimate: Option<f64>,
    state: Option<StateNode>,
    cycle: Option<CycleNode>,
    labels: Option<LabelConnection>,
    project: Option<ProjectNode>,
    team: Option<TeamNode>,
    attachments: Option<AttachmentConnection>,
    parent: Option<ParentNode>,
    children: Option<ChildConnection>,
    branch_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StateNode {
    name: String,
    #[serde(rename = "type")]
    state_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CycleNode {
    id: String,
    name: String,
    number: i64,
    starts_at: String,
    ends_at: String,
}

#[derive(Debug, Deserialize)]
struct LabelConnection {
    nodes: Vec<LabelNode>,
}

#[derive(Debug, Deserialize)]
struct LabelNode {
    name: String,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProjectNode {
    name: String,
}

#[derive(Debug, Deserialize)]
struct TeamNode {
    name: String,
    #[allow(dead_code)]
    key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AttachmentConnection {
    nodes: Vec<AttachmentNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentNode {
    id: String,
    url: String,
    title: Option<String>,
    subtitle: Option<String>,
    source_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ParentNode {
    id: String,
    identifier: String,
    title: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChildConnection {
    nodes: Vec<ChildNode>,
}

#[derive(Debug, Deserialize)]
struct ChildNode {
    id: String,
    identifier: String,
    title: Option<String>,
    url: Option<String>,
    priority: Option<i64>,
    state: Option<StateNode>,
}

/// Fetch issues assigned to the current user
pub async fn fetch_assigned_issues(config: &Config) -> Result<Vec<LinkedLinearIssue>> {
    let client = &*HTTP_CLIENT;

    let query = r#"
        query AssignedIssues {
            viewer {
                assignedIssues(first: 100) {
                    nodes {
                        id
                        identifier
                        title
                        description
                        url
                        updatedAt
                        createdAt
                        priority
                        estimate
                        state {
                            name
                            type
                        }
                        cycle {
                            id
                            name
                            number
                            startsAt
                            endsAt
                        }
                        labels {
                            nodes {
                                name
                                color
                            }
                        }
                        project {
                            name
                        }
                        team {
                            name
                            key
                        }
                        attachments {
                            nodes {
                                id
                                url
                                title
                                subtitle
                                sourceType
                            }
                        }
                        parent {
                            id
                            identifier
                            title
                            url
                        }
                        children {
                            nodes {
                                id
                                identifier
                                title
                                url
                                priority
                                state {
                                    name
                                    type
                                }
                            }
                        }
                        branchName
                    }
                }
            }
        }
    "#;

    let response = client
        .post(LINEAR_API_URL)
        .header("Authorization", &config.tokens.linear)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await?;

    // Use typed deserialization for better error messages and type safety
    let body: GraphQLResponse<ViewerData> = response.json().await?;

    let issues = body
        .data
        .map(|d| {
            d.viewer
                .assigned_issues
                .nodes
                .into_iter()
                .filter_map(parse_issue_node)
                .collect()
        })
        .unwrap_or_default();

    Ok(issues)
}

/// Parse a typed IssueNode into LinkedLinearIssue
fn parse_issue_node(node: IssueNode) -> Option<LinkedLinearIssue> {
    let state = node.state.as_ref()?;
    let state_type = &state.state_type;
    let state_name = state.name.to_lowercase();

    let status = parse_status(state_type, &state_name);

    // Find GitHub PR URL in attachments
    let pr_url = node.attachments.as_ref().and_then(|attachments| {
        attachments.nodes.iter().find_map(|a| {
            if a.url.contains("github.com") && a.url.contains("/pull/") {
                Some(a.url.clone())
            } else {
                None
            }
        })
    });

    // Parse priority
    let priority = node.priority.map(LinearPriority::from_int).unwrap_or_default();

    // Parse cycle
    let cycle = node.cycle.and_then(|c| {
        Some(LinearCycle {
            id: c.id,
            name: c.name,
            number: c.number as i32,
            starts_at: c.starts_at.parse().ok().unwrap_or_else(Utc::now),
            ends_at: c.ends_at.parse().ok().unwrap_or_else(Utc::now),
        })
    });

    // Parse labels
    let labels = node
        .labels
        .map(|l| {
            l.nodes
                .into_iter()
                .map(|label| LinearLabel {
                    name: label.name,
                    color: label.color.unwrap_or_else(|| "#888888".to_string()),
                })
                .collect()
        })
        .unwrap_or_default();

    // Parse project and team
    let project = node.project.map(|p| p.name);
    let team = node.team.map(|t| t.name);

    // Parse estimate
    let estimate = node.estimate.map(|e| e as f32);

    // Parse attachments (filter out GitHub PR links)
    let attachments = node
        .attachments
        .map(|a| {
            a.nodes
                .into_iter()
                .filter(|att| !(att.url.contains("github.com") && att.url.contains("/pull/")))
                .map(|att| LinearAttachment {
                    id: att.id,
                    url: att.url,
                    title: att.title.unwrap_or_else(|| "Untitled".to_string()),
                    subtitle: att.subtitle,
                    source_type: att.source_type,
                })
                .collect()
        })
        .unwrap_or_default();

    // Parse parent
    let parent = node.parent.and_then(|p| {
        Some(LinearParentRef {
            id: p.id,
            identifier: p.identifier,
            title: p.title.unwrap_or_default(),
            url: p.url.unwrap_or_default(),
        })
    });

    // Parse children
    let children = node
        .children
        .map(|c| {
            c.nodes
                .into_iter()
                .filter_map(|child| {
                    let child_state = child.state.as_ref()?;
                    let child_status =
                        parse_status(&child_state.state_type, &child_state.name.to_lowercase());
                    let child_priority = child.priority.map(LinearPriority::from_int).unwrap_or_default();

                    Some(LinearChildRef {
                        id: child.id,
                        identifier: child.identifier,
                        title: child.title.unwrap_or_default(),
                        url: child.url.unwrap_or_default(),
                        status: child_status,
                        priority: child_priority,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let issue = LinearIssue {
        id: node.id,
        identifier: node.identifier,
        title: node.title,
        description: node.description,
        status,
        priority,
        url: node.url,
        created_at: node.created_at.parse().ok().unwrap_or_else(Utc::now),
        updated_at: node.updated_at.parse().ok().unwrap_or_else(Utc::now),
        cycle,
        labels,
        project,
        team,
        estimate,
        attachments,
        parent,
        children,
    };

    Some(LinkedLinearIssue {
        issue,
        linked_pr_url: pr_url,
        working_directory: node.branch_name,
    })
}

/// Parse Linear state type and name into LinearStatus
fn parse_status(state_type: &str, state_name: &str) -> LinearStatus {
    match state_type {
        "triage" => LinearStatus::Triage,
        "backlog" => LinearStatus::Backlog,
        "unstarted" => LinearStatus::Todo,
        "started" => {
            // Check state_name for "review" before defaulting to InProgress
            // Many Linear setups have "In Review" states with type "started"
            if state_name.contains("review") {
                LinearStatus::InReview
            } else {
                LinearStatus::InProgress
            }
        }
        "completed" => LinearStatus::Done,
        "canceled" => {
            if state_name.contains("duplicate") {
                LinearStatus::Duplicate
            } else {
                LinearStatus::Canceled
            }
        }
        _ => {
            // Fallback for unknown types - check state_name for hints
            if state_name.contains("review") {
                LinearStatus::InReview
            } else if state_name.contains("duplicate") {
                LinearStatus::Duplicate
            } else if state_name.contains("triage") {
                LinearStatus::Triage
            } else {
                LinearStatus::InProgress
            }
        }
    }
}

fn parse_linear_issue(node: &serde_json::Value) -> Option<LinkedLinearIssue> {
    let state_type = node["state"]["type"].as_str()?;
    let state_name = node["state"]["name"].as_str().unwrap_or("").to_lowercase();
    let status = match state_type {
        "triage" => LinearStatus::Triage,
        "backlog" => LinearStatus::Backlog,
        "unstarted" => LinearStatus::Todo,
        "started" => LinearStatus::InProgress,
        "completed" => LinearStatus::Done,
        "canceled" => {
            // Check if it's a duplicate (often marked as canceled with "duplicate" in name)
            if state_name.contains("duplicate") {
                LinearStatus::Duplicate
            } else {
                LinearStatus::Canceled
            }
        }
        _ => {
            // Check state name for special statuses
            if state_name.contains("review") {
                LinearStatus::InReview
            } else if state_name.contains("duplicate") {
                LinearStatus::Duplicate
            } else if state_name.contains("triage") {
                LinearStatus::Triage
            } else {
                LinearStatus::InProgress
            }
        }
    };

    // Find GitHub PR URL in attachments
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

    // Parse priority (0-4 integer from API)
    let priority = node["priority"]
        .as_i64()
        .map(LinearPriority::from_int)
        .unwrap_or_default();

    // Parse cycle if present
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

    // Parse project name
    let project = node["project"]["name"].as_str().map(String::from);

    // Parse team name
    let team = node["team"]["name"].as_str().map(String::from);

    // Parse estimate
    let estimate = node["estimate"].as_f64().map(|e| e as f32);

    // Parse attachments (filter out GitHub PR links)
    let attachments = node["attachments"]["nodes"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let url = a["url"].as_str()?;
                    // Skip GitHub PR attachments (we handle those separately)
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

    // Parse parent issue
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

    // Parse children (sub-issues)
    let children = node["children"]["nodes"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let child_state_type = c["state"]["type"].as_str()?;
                    let child_state_name = c["state"]["name"].as_str().unwrap_or("").to_lowercase();
                    let child_status = match child_state_type {
                        "triage" => LinearStatus::Triage,
                        "backlog" => LinearStatus::Backlog,
                        "unstarted" => LinearStatus::Todo,
                        "started" => LinearStatus::InProgress,
                        "completed" => LinearStatus::Done,
                        "canceled" => {
                            if child_state_name.contains("duplicate") {
                                LinearStatus::Duplicate
                            } else {
                                LinearStatus::Canceled
                            }
                        }
                        _ => {
                            if child_state_name.contains("review") {
                                LinearStatus::InReview
                            } else if child_state_name.contains("duplicate") {
                                LinearStatus::Duplicate
                            } else if child_state_name.contains("triage") {
                                LinearStatus::Triage
                            } else {
                                LinearStatus::InProgress
                            }
                        }
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
        project,
        team,
        estimate,
        attachments,
        parent,
        children,
    };

    Some(LinkedLinearIssue {
        issue,
        linked_pr_url: pr_url,
        working_directory: node["branchName"].as_str().map(String::from),
    })
}

/// Search all Linear issues (for full search mode)
#[allow(dead_code)]
pub async fn search_issues(config: &Config, query: &str) -> Result<Vec<LinearIssue>> {
    let client = &*HTTP_CLIENT;

    let graphql_query = r#"
        query SearchIssues($query: String!) {
            issueSearch(query: $query, first: 20) {
                nodes {
                    id
                    identifier
                    title
                    description
                    url
                    createdAt
                    updatedAt
                    priority
                    estimate
                    state {
                        name
                        type
                    }
                    cycle {
                        id
                        name
                        number
                        startsAt
                        endsAt
                    }
                    labels {
                        nodes {
                            name
                            color
                        }
                    }
                    project {
                        name
                    }
                    team {
                        name
                    }
                }
            }
        }
    "#;

    let response = client
        .post(LINEAR_API_URL)
        .header("Authorization", &config.tokens.linear)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "query": graphql_query,
            "variables": { "query": query }
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    let issues = body["data"]["issueSearch"]["nodes"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|node| {
            let state_type = node["state"]["type"].as_str()?;
            let state_name = node["state"]["name"].as_str().unwrap_or("").to_lowercase();
            let status = match state_type {
                "triage" => LinearStatus::Triage,
                "backlog" => LinearStatus::Backlog,
                "unstarted" => LinearStatus::Todo,
                "started" => LinearStatus::InProgress,
                "completed" => LinearStatus::Done,
                "canceled" => {
                    if state_name.contains("duplicate") {
                        LinearStatus::Duplicate
                    } else {
                        LinearStatus::Canceled
                    }
                }
                _ => {
                    if state_name.contains("review") {
                        LinearStatus::InReview
                    } else if state_name.contains("duplicate") {
                        LinearStatus::Duplicate
                    } else if state_name.contains("triage") {
                        LinearStatus::Triage
                    } else {
                        LinearStatus::InProgress
                    }
                }
            };

            let priority = node["priority"]
                .as_i64()
                .map(LinearPriority::from_int)
                .unwrap_or_default();

            // Parse cycle if present
            let cycle = if node["cycle"].is_object() {
                let c = &node["cycle"];
                Some(LinearCycle {
                    id: c["id"].as_str()?.to_string(),
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

            Some(LinearIssue {
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
                // Search results don't include full metadata
                attachments: Vec::new(),
                parent: None,
                children: Vec::new(),
            })
        })
        .collect();

    Ok(issues)
}
