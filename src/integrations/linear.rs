use crate::config::Config;
use crate::data::{
    LinearAttachment, LinearChildRef, LinearCycle, LinearIssue, LinearLabel,
    LinearParentRef, LinearPriority, LinearStatus,
};
use crate::integrations::{LinkedLinearIssue, HTTP_CLIENT};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;

const LINEAR_API_URL: &str = "https://api.linear.app/graphql";

// =============================================================================
// GraphQL Response Types
// =============================================================================

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

// =============================================================================
// Issue Response Types
// =============================================================================

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
#[serde(rename_all = "camelCase")]
struct IssueConnection {
    nodes: Vec<IssueNode>,
    page_info: PageInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
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
    assignee: Option<UserNode>,
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
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct TeamNode {
    name: String,
    #[allow(dead_code)]
    key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserNode {
    id: String,
    name: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
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

// =============================================================================
// Project/Team Response Types
// =============================================================================

#[derive(Debug, Deserialize)]
struct ProjectsData {
    projects: ProjectConnection,
}

#[derive(Debug, Deserialize)]
struct ProjectConnection {
    nodes: Vec<ProjectInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct TeamMembersData {
    users: UserConnection,
}

#[derive(Debug, Deserialize)]
struct UserConnection {
    nodes: Vec<TeamMemberInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TeamMemberInfo {
    pub id: String,
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub email: Option<String>,
}

// =============================================================================
// GraphQL Query Fragments
// =============================================================================

const ISSUE_FIELDS: &str = r#"
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
        id
        name
    }
    team {
        name
        key
    }
    assignee {
        id
        name
        displayName
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
"#;

// =============================================================================
// Public API: Issue Fetching
// =============================================================================

/// Fetch all assigned issues with pagination
pub async fn fetch_assigned_issues(config: &Config) -> Result<Vec<LinkedLinearIssue>> {
    let fetch_limit = config.linear.fetch_limit;
    fetch_issues_paginated(config, fetch_limit, None).await
}

/// Fetch issues updated since a given timestamp (incremental sync)
pub async fn fetch_issues_since(
    config: &Config,
    since: DateTime<Utc>,
) -> Result<Vec<LinkedLinearIssue>> {
    let fetch_limit = config.linear.fetch_limit;
    fetch_issues_paginated(config, fetch_limit, Some(since)).await
}

/// Fetch issues with pagination support
async fn fetch_issues_paginated(
    config: &Config,
    limit: usize,
    updated_since: Option<DateTime<Utc>>,
) -> Result<Vec<LinkedLinearIssue>> {
    let client = &*HTTP_CLIENT;
    let mut all_issues = Vec::new();
    let mut cursor: Option<String> = None;
    let page_size = limit.min(100); // Linear API max is 100 per page

    loop {
        let query = build_issues_query(page_size, cursor.as_deref(), updated_since);

        let response = client
            .post(LINEAR_API_URL)
            .header("Authorization", &config.tokens.linear)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await?;

        let body: GraphQLResponse<ViewerData> = response.json().await?;

        if let Some(data) = body.data {
            let connection = data.viewer.assigned_issues;

            for node in connection.nodes {
                if let Some(issue) = parse_issue_node(node) {
                    all_issues.push(issue);
                }
            }

            // Check if we should continue pagination
            if connection.page_info.has_next_page && all_issues.len() < limit {
                cursor = connection.page_info.end_cursor;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    // Respect the limit
    all_issues.truncate(limit);
    Ok(all_issues)
}

/// Build the GraphQL query for fetching issues
fn build_issues_query(limit: usize, cursor: Option<&str>, updated_since: Option<DateTime<Utc>>) -> String {
    let after_clause = cursor
        .map(|c| format!(r#", after: "{}""#, c))
        .unwrap_or_default();

    let filter_clause = updated_since
        .map(|ts| format!(r#", filter: {{ updatedAt: {{ gte: "{}" }} }}"#, ts.to_rfc3339()))
        .unwrap_or_default();

    format!(
        r#"
        query AssignedIssues {{
            viewer {{
                assignedIssues(first: {}{}{}) {{
                    pageInfo {{
                        hasNextPage
                        endCursor
                    }}
                    nodes {{
                        {}
                    }}
                }}
            }}
        }}
        "#,
        limit, after_clause, filter_clause, ISSUE_FIELDS
    )
}

// =============================================================================
// Public API: Projects and Team Members
// =============================================================================

/// Fetch all projects accessible to the user
pub async fn fetch_projects(config: &Config) -> Result<Vec<ProjectInfo>> {
    let client = &*HTTP_CLIENT;

    let query = r#"
        query Projects {
            projects(first: 100) {
                nodes {
                    id
                    name
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

    let body: GraphQLResponse<ProjectsData> = response.json().await?;

    let projects = body
        .data
        .map(|d| d.projects.nodes)
        .unwrap_or_default();

    Ok(projects)
}

/// Fetch team members (users in the organization)
pub async fn fetch_team_members(config: &Config) -> Result<Vec<TeamMemberInfo>> {
    let client = &*HTTP_CLIENT;

    let query = r#"
        query TeamMembers {
            users(first: 100) {
                nodes {
                    id
                    name
                    displayName
                    email
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

    let body: GraphQLResponse<TeamMembersData> = response.json().await?;

    let members = body
        .data
        .map(|d| d.users.nodes)
        .unwrap_or_default();

    Ok(members)
}

/// Get the current user's ID
pub async fn fetch_current_user_id(config: &Config) -> Result<String> {
    let client = &*HTTP_CLIENT;

    let query = r#"
        query CurrentUser {
            viewer {
                id
            }
        }
    "#;

    #[derive(Deserialize)]
    struct ViewerIdData {
        viewer: ViewerId,
    }

    #[derive(Deserialize)]
    struct ViewerId {
        id: String,
    }

    let response = client
        .post(LINEAR_API_URL)
        .header("Authorization", &config.tokens.linear)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await?;

    let body: GraphQLResponse<ViewerIdData> = response.json().await?;

    body.data
        .map(|d| d.viewer.id)
        .ok_or_else(|| anyhow::anyhow!("Failed to get current user ID"))
}

// =============================================================================
// Public API: Search
// =============================================================================

/// Search all Linear issues (for full search mode)
#[allow(dead_code)]
pub async fn search_issues(config: &Config, query: &str) -> Result<Vec<LinearIssue>> {
    let client = &*HTTP_CLIENT;

    let graphql_query = format!(
        r#"
        query SearchIssues($query: String!) {{
            issueSearch(query: $query, first: 20) {{
                nodes {{
                    {}
                }}
            }}
        }}
        "#,
        ISSUE_FIELDS
    );

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

    #[derive(Deserialize)]
    struct SearchData {
        #[serde(rename = "issueSearch")]
        issue_search: IssueConnection,
    }

    let body: GraphQLResponse<SearchData> = response.json().await?;

    let issues = body
        .data
        .map(|d| {
            d.issue_search
                .nodes
                .into_iter()
                .filter_map(|node| parse_issue_node(node).map(|li| li.issue))
                .collect()
        })
        .unwrap_or_default();

    Ok(issues)
}

// =============================================================================
// Parsing Helpers
// =============================================================================

/// Parse a typed IssueNode into LinkedLinearIssue
fn parse_issue_node(node: IssueNode) -> Option<LinkedLinearIssue> {
    let state = node.state.as_ref()?;
    let status = parse_status(&state.state_type, &state.name);

    let issue = LinearIssue {
        id: node.id,
        identifier: node.identifier,
        title: node.title,
        description: node.description,
        status,
        priority: parse_priority(node.priority),
        url: node.url,
        created_at: parse_datetime(&node.created_at),
        updated_at: parse_datetime(&node.updated_at),
        cycle: parse_cycle(node.cycle),
        labels: parse_labels(node.labels),
        project: node.project.map(|p| p.name),
        team: node.team.map(|t| t.name),
        estimate: node.estimate.map(|e| e as f32),
        attachments: parse_attachments(&node.attachments),
        parent: parse_parent(node.parent),
        children: parse_children(node.children),
    };

    let pr_url = find_github_pr_url(&node.attachments);

    Some(LinkedLinearIssue {
        issue,
        linked_pr_url: pr_url,
        working_directory: node.branch_name,
    })
}

/// Parse Linear state type and name into LinearStatus
fn parse_status(state_type: &str, state_name: &str) -> LinearStatus {
    let name_lower = state_name.to_lowercase();

    match state_type {
        "triage" => LinearStatus::Triage,
        "backlog" => LinearStatus::Backlog,
        "unstarted" => LinearStatus::Todo,
        "started" => {
            if name_lower.contains("review") {
                LinearStatus::InReview
            } else {
                LinearStatus::InProgress
            }
        }
        "completed" => LinearStatus::Done,
        "canceled" => {
            if name_lower.contains("duplicate") {
                LinearStatus::Duplicate
            } else {
                LinearStatus::Canceled
            }
        }
        _ => infer_status_from_name(&name_lower),
    }
}

/// Infer status from state name when type is unknown
fn infer_status_from_name(name: &str) -> LinearStatus {
    if name.contains("review") {
        LinearStatus::InReview
    } else if name.contains("duplicate") {
        LinearStatus::Duplicate
    } else if name.contains("triage") {
        LinearStatus::Triage
    } else {
        LinearStatus::InProgress
    }
}

fn parse_priority(priority: Option<i64>) -> LinearPriority {
    priority.map(LinearPriority::from_int).unwrap_or_default()
}

fn parse_datetime(s: &str) -> DateTime<Utc> {
    s.parse().unwrap_or_else(|_| Utc::now())
}

fn parse_cycle(cycle: Option<CycleNode>) -> Option<LinearCycle> {
    cycle.map(|c| LinearCycle {
        id: c.id,
        name: c.name,
        number: c.number as i32,
        starts_at: parse_datetime(&c.starts_at),
        ends_at: parse_datetime(&c.ends_at),
    })
}

fn parse_labels(labels: Option<LabelConnection>) -> Vec<LinearLabel> {
    labels
        .map(|l| {
            l.nodes
                .into_iter()
                .map(|label| LinearLabel {
                    name: label.name,
                    color: label.color.unwrap_or_else(|| "#888888".to_string()),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_attachments(attachments: &Option<AttachmentConnection>) -> Vec<LinearAttachment> {
    attachments
        .as_ref()
        .map(|a| {
            a.nodes
                .iter()
                .filter(|att| !is_github_pr_url(&att.url))
                .map(|att| LinearAttachment {
                    id: att.id.clone(),
                    url: att.url.clone(),
                    title: att.title.clone().unwrap_or_else(|| "Untitled".to_string()),
                    subtitle: att.subtitle.clone(),
                    source_type: att.source_type.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_parent(parent: Option<ParentNode>) -> Option<LinearParentRef> {
    parent.map(|p| LinearParentRef {
        id: p.id,
        identifier: p.identifier,
        title: p.title.unwrap_or_default(),
        url: p.url.unwrap_or_default(),
    })
}

fn parse_children(children: Option<ChildConnection>) -> Vec<LinearChildRef> {
    children
        .map(|c| {
            c.nodes
                .into_iter()
                .filter_map(|child| {
                    let state = child.state.as_ref()?;
                    let status = parse_status(&state.state_type, &state.name);

                    Some(LinearChildRef {
                        id: child.id,
                        identifier: child.identifier,
                        title: child.title.unwrap_or_default(),
                        url: child.url.unwrap_or_default(),
                        status,
                        priority: parse_priority(child.priority),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn find_github_pr_url(attachments: &Option<AttachmentConnection>) -> Option<String> {
    attachments.as_ref().and_then(|a| {
        a.nodes
            .iter()
            .find(|att| is_github_pr_url(&att.url))
            .map(|att| att.url.clone())
    })
}

fn is_github_pr_url(url: &str) -> bool {
    url.contains("github.com") && url.contains("/pull/")
}
