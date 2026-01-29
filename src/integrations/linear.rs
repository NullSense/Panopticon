use crate::config::Config;
use crate::data::{LinearIssue, LinearStatus};
use crate::integrations::LinkedLinearIssue;
use anyhow::Result;
use chrono::Utc;

const LINEAR_API_URL: &str = "https://api.linear.app/graphql";

/// Fetch issues assigned to the current user
pub async fn fetch_assigned_issues(config: &Config) -> Result<Vec<LinkedLinearIssue>> {
    let client = reqwest::Client::new();

    let query = r#"
        query AssignedIssues {
            viewer {
                assignedIssues(first: 50, filter: { state: { type: { nin: ["canceled", "completed"] } } }) {
                    nodes {
                        id
                        identifier
                        title
                        description
                        url
                        updatedAt
                        state {
                            name
                            type
                        }
                        attachments {
                            nodes {
                                url
                                title
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

    let body: serde_json::Value = response.json().await?;

    let issues = body["data"]["viewer"]["assignedIssues"]["nodes"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|node| parse_linear_issue(node))
        .collect();

    Ok(issues)
}

fn parse_linear_issue(node: &serde_json::Value) -> Option<LinkedLinearIssue> {
    let state_type = node["state"]["type"].as_str()?;
    let status = match state_type {
        "backlog" => LinearStatus::Backlog,
        "unstarted" => LinearStatus::Todo,
        "started" => LinearStatus::InProgress,
        "completed" => LinearStatus::Done,
        "canceled" => LinearStatus::Canceled,
        _ => {
            // Check state name for "review" states
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

    let issue = LinearIssue {
        id: node["id"].as_str()?.to_string(),
        identifier: node["identifier"].as_str()?.to_string(),
        title: node["title"].as_str()?.to_string(),
        description: node["description"].as_str().map(String::from),
        status,
        url: node["url"].as_str()?.to_string(),
        updated_at: node["updatedAt"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
    };

    Some(LinkedLinearIssue {
        issue,
        linked_pr_url: pr_url,
        working_directory: node["branchName"].as_str().map(String::from),
    })
}

/// Search all Linear issues (for full search mode)
pub async fn search_issues(config: &Config, query: &str) -> Result<Vec<LinearIssue>> {
    let client = reqwest::Client::new();

    let graphql_query = r#"
        query SearchIssues($query: String!) {
            issueSearch(query: $query, first: 20) {
                nodes {
                    id
                    identifier
                    title
                    description
                    url
                    updatedAt
                    state {
                        name
                        type
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
            let status = match state_type {
                "backlog" => LinearStatus::Backlog,
                "unstarted" => LinearStatus::Todo,
                "started" => LinearStatus::InProgress,
                "completed" => LinearStatus::Done,
                "canceled" => LinearStatus::Canceled,
                _ => LinearStatus::InProgress,
            };

            Some(LinearIssue {
                id: node["id"].as_str()?.to_string(),
                identifier: node["identifier"].as_str()?.to_string(),
                title: node["title"].as_str()?.to_string(),
                description: node["description"].as_str().map(String::from),
                status,
                url: node["url"].as_str()?.to_string(),
                updated_at: node["updatedAt"]
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(Utc::now),
            })
        })
        .collect();

    Ok(issues)
}
