pub mod agent_cache;
pub mod cache;
pub mod claude;
pub mod enrichment_cache;
pub mod github;
pub mod linear;
pub mod openclaw;
pub mod vercel;

use crate::config::Config;
use crate::data::{AgentSession, LinearIssue, LinearPriority, LinearStatus, Workstream};
use crate::tui::{RefreshMetadata, RefreshProgress, RefreshResult};
use anyhow::Result;
use chrono::Utc;
use futures::stream::{self, StreamExt};
use once_cell::sync::Lazy;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Shared HTTP client for all API requests to enable connection pooling
pub static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(5)
        .build()
        .expect("Failed to create HTTP client")
});

/// Fetches all workstreams by querying Linear, then enriching with GitHub/Vercel data
pub async fn fetch_workstreams(config: &Config) -> Result<Vec<Workstream>> {
    // 1. Get Linear issues assigned to user
    let issues = linear::fetch_assigned_issues(config).await?;

    // 2. Pre-load agent session cache ONCE (1 file read + 1 HTTP call total)
    let agent_cache = agent_cache::AgentSessionCache::load().await;

    // 3. For each issue, find linked PR and deployment
    let mut workstreams = Vec::new();
    for issue in issues {
        let pr = if let Some(pr_url) = &issue.linked_pr_url {
            match github::fetch_pr_from_url(config, pr_url).await {
                Ok(pr) => Some(pr),
                Err(e) => {
                    tracing::debug!("Failed to fetch PR from {}: {}", pr_url, e);
                    None
                }
            }
        } else {
            None
        };

        let deployment = if let Some(ref pr) = pr {
            match vercel::fetch_deployment_for_branch(config, &pr.repo, &pr.branch).await {
                Ok(deploy) => deploy,
                Err(e) => {
                    tracing::debug!(
                        "Failed to fetch Vercel deployment for {}/{}: {}",
                        pr.repo,
                        pr.branch,
                        e
                    );
                    None
                }
            }
        } else {
            None
        };

        // Find agent session via O(1) cache lookup by git branch
        // Linear's working_directory is the branch name, match against session's git_branch
        let agent_sessions = agent_cache.find_all_for_branch_or_identifier(
            issue.working_directory.as_deref(),
            &issue.issue.identifier,
            pr.as_ref().map(|p| p.repo.as_str()),
        );
        let agent = agent_cache.find_for_branch_or_identifier(
            issue.working_directory.as_deref(),
            &issue.issue.identifier,
            pr.as_ref().map(|p| p.repo.as_str()),
        );

        workstreams.push(Workstream {
            linear_issue: issue.issue,
            github_pr: pr,
            vercel_deployment: deployment,
            agent_sessions,
            agent_session: agent,
            stale: false,
        });
    }

    // Add unlinked sessions (sessions not matched to any issue)
    let matched_session_ids: std::collections::HashSet<String> = workstreams
        .iter()
        .flat_map(|ws| {
            if !ws.agent_sessions.is_empty() {
                ws.agent_sessions.iter().map(|s| s.id.clone()).collect()
            } else if let Some(session) = ws.agent_session.as_ref() {
                vec![session.id.clone()]
            } else {
                vec![]
            }
        })
        .collect();

    for session in agent_cache.all_sessions() {
        if !matched_session_ids.contains(&session.id) {
            workstreams.push(Workstream {
                linear_issue: create_placeholder_issue(session),
                github_pr: None,
                vercel_deployment: None,
                agent_sessions: vec![session.clone()],
                agent_session: Some(session.clone()),
                stale: false,
            });
        }
    }

    Ok(workstreams)
}

/// Fetch workstreams incrementally with progress updates (non-blocking)
/// Sends results via channel as they become available
pub async fn fetch_workstreams_incremental(
    config: &Config,
    tx: mpsc::Sender<RefreshResult>,
) -> Result<()> {
    // Step 0: Fetch metadata (projects, team members, current user)
    let (projects_res, members_res, current_user_res) = tokio::join!(
        linear::fetch_projects(config),
        linear::fetch_team_members(config),
        linear::fetch_current_user_id(config)
    );

    let metadata = RefreshMetadata {
        projects: projects_res.ok(),
        team_members: members_res.ok(),
        current_user_id: current_user_res.ok(),
    };

    crate::util::send_or_log(&tx, RefreshResult::Metadata(metadata), "metadata update").await;

    // Step 1: Fetch all Linear issues first
    if let Err(e) = tx
        .send(RefreshResult::Progress(RefreshProgress {
            total_issues: 0,
            completed: 0,
            current_stage: "Fetching Linear issues...".to_string(),
        }))
        .await
    {
        tracing::warn!("Failed to send progress update: {}", e);
    }

    let issues = linear::fetch_assigned_issues(config).await?;
    let total = issues.len();

    if let Err(e) = tx
        .send(RefreshResult::Progress(RefreshProgress {
            total_issues: total,
            completed: 0,
            current_stage: format!("Found {} issues, loading agent sessions...", total),
        }))
        .await
    {
        tracing::warn!("Failed to send progress update: {}", e);
    }

    // Step 2: Pre-load agent session cache ONCE (1 file read + 1 HTTP call total)
    // This replaces 100+ individual file reads and HTTP calls
    let agent_cache = Arc::new(agent_cache::AgentSessionCache::load().await);

    if let Err(e) = tx
        .send(RefreshResult::Progress(RefreshProgress {
            total_issues: total,
            completed: 0,
            current_stage: format!("Found {} issues, enriching...", total),
        }))
        .await
    {
        tracing::warn!("Failed to send progress update: {}", e);
    }

    // Step 3: Process issues in parallel (batch of 5 concurrent)
    // Track matched session IDs to find unlinked sessions later
    let matched_session_ids = Arc::new(tokio::sync::Mutex::new(std::collections::HashSet::new()));

    let config = config.clone();
    stream::iter(issues.into_iter().enumerate())
        .map(|(i, issue)| {
            let config = config.clone();
            let tx = tx.clone();
            let agent_cache = Arc::clone(&agent_cache);
            let matched_ids = Arc::clone(&matched_session_ids);
            async move {
                // Send progress
                if let Err(e) = tx
                    .send(RefreshResult::Progress(RefreshProgress {
                        total_issues: total,
                        completed: i,
                        current_stage: format!(
                            "Processing {} ({}/{})",
                            issue.issue.identifier,
                            i + 1,
                            total
                        ),
                    }))
                    .await
                {
                    tracing::debug!("Progress channel closed: {}", e);
                }

                // Fetch GitHub PR if linked
                let pr = if let Some(pr_url) = &issue.linked_pr_url {
                    match github::fetch_pr_from_url(&config, pr_url).await {
                        Ok(pr) => Some(pr),
                        Err(e) => {
                            tracing::debug!("Failed to fetch PR from {}: {}", pr_url, e);
                            None
                        }
                    }
                } else {
                    None
                };

                // Fetch Vercel deployment if PR exists
                let deployment = if let Some(ref pr) = pr {
                    match vercel::fetch_deployment_for_branch(&config, &pr.repo, &pr.branch).await {
                        Ok(deploy) => deploy,
                        Err(e) => {
                            tracing::debug!(
                                "Failed to fetch Vercel deployment for {}/{}: {}",
                                pr.repo,
                                pr.branch,
                                e
                            );
                            None
                        }
                    }
                } else {
                    None
                };

                // Find agent session via O(1) cache lookup by git branch
                let agent_sessions = agent_cache.find_all_for_branch_or_identifier(
                    issue.working_directory.as_deref(),
                    &issue.issue.identifier,
                    pr.as_ref().map(|p| p.repo.as_str()),
                );
                let agent = agent_cache.find_for_branch_or_identifier(
                    issue.working_directory.as_deref(),
                    &issue.issue.identifier,
                    pr.as_ref().map(|p| p.repo.as_str()),
                );

                // Track matched session IDs
                if !agent_sessions.is_empty() {
                    let mut guard = matched_ids.lock().await;
                    for session in &agent_sessions {
                        guard.insert(session.id.clone());
                    }
                } else if let Some(ref session) = agent {
                    matched_ids.lock().await.insert(session.id.clone());
                }

                let ws = Workstream {
                    linear_issue: issue.issue,
                    github_pr: pr,
                    vercel_deployment: deployment,
                    agent_sessions,
                    agent_session: agent,
                    stale: false,
                };

                crate::util::send_or_log(
                    &tx,
                    RefreshResult::Workstream(Box::new(ws)),
                    "workstream",
                )
                .await;
            }
        })
        .buffer_unordered(5) // Process 5 issues concurrently
        .collect::<Vec<_>>()
        .await;

    // Step 4: Add unlinked sessions (sessions not matched to any issue)
    let matched_ids = matched_session_ids.lock().await;
    for session in agent_cache.all_sessions() {
        if !matched_ids.contains(&session.id) {
            let ws = Workstream {
                linear_issue: create_placeholder_issue(session),
                github_pr: None,
                vercel_deployment: None,
                agent_sessions: vec![session.clone()],
                agent_session: Some(session.clone()),
                stale: false,
            };
            crate::util::send_or_log(
                &tx,
                RefreshResult::Workstream(Box::new(ws)),
                "unlinked session",
            )
            .await;
        }
    }

    crate::util::send_or_log(&tx, RefreshResult::Complete, "complete signal").await;
    Ok(())
}

/// Intermediate struct for Linear issues with extra linking info
pub struct LinkedLinearIssue {
    pub issue: crate::data::LinearIssue,
    pub linked_pr_url: Option<String>,
    pub working_directory: Option<String>,
}

/// Create a placeholder issue for unlinked agent sessions
/// These sessions appear in the Agent Sessions section but aren't linked to Linear issues
fn create_placeholder_issue(session: &AgentSession) -> LinearIssue {
    let title = if session.agent_type == crate::data::AgentType::OpenClaw {
        let via = session
            .activity
            .surface_label
            .clone()
            .or_else(|| session.activity.surface.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let profile = session
            .activity
            .profile
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let mut display = format!("Via {} / {}", via, profile);
        if let Some(branch) = &session.git_branch {
            display.push_str(&format!(" ({})", branch));
        }
        display
    } else {
        // Build title showing path + branch: ~/Projects/sandbox (main)
        let shortened_path = session.working_directory.as_ref().map(|p| {
            // Shorten path: /home/user/Projects/foo -> ~/Projects/foo
            if let Some(home) = dirs::home_dir() {
                if let Some(home_str) = home.to_str() {
                    if let Some(stripped) = p.strip_prefix(home_str) {
                        return format!("~{}", stripped);
                    }
                }
            }
            p.clone()
        });

        match (&shortened_path, &session.git_branch) {
            (Some(path), Some(branch)) => format!("{} ({})", path, branch),
            (Some(path), None) => path.clone(),
            (None, Some(branch)) => branch.clone(),
            (None, None) => "Unknown session".to_string(),
        }
    };

    let description = session.working_directory.clone();

    LinearIssue {
        id: format!("unlinked-{}", session.id),
        identifier: String::new(), // Empty = unlinked indicator
        title,
        description,
        status: LinearStatus::InProgress,
        priority: LinearPriority::NoPriority,
        url: String::new(),
        created_at: session.started_at,
        updated_at: Utc::now(),
        cycle: None,
        labels: vec![],
        project: None,
        team: None,
        assignee_id: None,
        assignee_name: None,
        estimate: None,
        attachments: vec![],
        parent: None,
        children: vec![],
    }
}
