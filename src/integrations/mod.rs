pub mod claude;
pub mod github;
pub mod linear;
pub mod moltbot;
pub mod vercel;

use crate::config::Config;
use crate::data::Workstream;
use crate::tui::{RefreshProgress, RefreshResult};
use anyhow::Result;
use futures::stream::{self, StreamExt};
use tokio::sync::mpsc;

/// Fetches all workstreams by querying Linear, then enriching with GitHub/Vercel data
pub async fn fetch_workstreams(config: &Config) -> Result<Vec<Workstream>> {
    // 1. Get Linear issues assigned to user
    let issues = linear::fetch_assigned_issues(config).await?;

    // 2. For each issue, find linked PR and deployment
    let mut workstreams = Vec::new();
    for issue in issues {
        let pr = if let Some(pr_url) = &issue.linked_pr_url {
            github::fetch_pr_from_url(config, pr_url).await.ok()
        } else {
            None
        };

        let deployment = if let Some(ref pr) = pr {
            vercel::fetch_deployment_for_branch(config, &pr.repo, &pr.branch)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        // Try to find agent session - check Claude first, then Clawdbot
        let agent = find_agent_session(issue.working_directory.as_deref()).await;

        workstreams.push(Workstream {
            linear_issue: issue.issue,
            github_pr: pr,
            vercel_deployment: deployment,
            agent_session: agent,
        });
    }

    Ok(workstreams)
}

/// Fetch workstreams incrementally with progress updates (non-blocking)
/// Sends results via channel as they become available
pub async fn fetch_workstreams_incremental(
    config: &Config,
    tx: mpsc::Sender<RefreshResult>,
) -> Result<()> {
    // Step 1: Fetch all Linear issues first
    let _ = tx
        .send(RefreshResult::Progress(RefreshProgress {
            total_issues: 0,
            completed: 0,
            current_stage: "Fetching Linear issues...".to_string(),
        }))
        .await;

    let issues = linear::fetch_assigned_issues(config).await?;
    let total = issues.len();

    let _ = tx
        .send(RefreshResult::Progress(RefreshProgress {
            total_issues: total,
            completed: 0,
            current_stage: format!("Found {} issues, enriching...", total),
        }))
        .await;

    // Step 2: Process issues in parallel (batch of 5 concurrent)
    let config = config.clone();
    stream::iter(issues.into_iter().enumerate())
        .map(|(i, issue)| {
            let config = config.clone();
            let tx = tx.clone();
            async move {
                // Send progress
                let _ = tx
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
                    .await;

                // Fetch GitHub PR if linked
                let pr = if let Some(pr_url) = &issue.linked_pr_url {
                    github::fetch_pr_from_url(&config, pr_url).await.ok()
                } else {
                    None
                };

                // Fetch Vercel deployment if PR exists
                let deployment = if let Some(ref pr) = pr {
                    vercel::fetch_deployment_for_branch(&config, &pr.repo, &pr.branch)
                        .await
                        .ok()
                        .flatten()
                } else {
                    None
                };

                // Find agent session
                let agent = find_agent_session(issue.working_directory.as_deref()).await;

                let ws = Workstream {
                    linear_issue: issue.issue,
                    github_pr: pr,
                    vercel_deployment: deployment,
                    agent_session: agent,
                };

                let _ = tx.send(RefreshResult::Workstream(ws)).await;
            }
        })
        .buffer_unordered(5) // Process 5 issues concurrently
        .collect::<Vec<_>>()
        .await;

    let _ = tx.send(RefreshResult::Complete).await;
    Ok(())
}

/// Find an agent session for a working directory, checking both Claude and Moltbot
async fn find_agent_session(dir: Option<&str>) -> Option<crate::data::AgentSession> {
    // Try Claude Code first
    if let Some(session) = claude::find_session_for_directory(dir).await {
        return Some(session);
    }

    // Fall back to Moltbot
    moltbot::find_session_for_directory(dir).await
}

/// Intermediate struct for Linear issues with extra linking info
pub struct LinkedLinearIssue {
    pub issue: crate::data::LinearIssue,
    pub linked_pr_url: Option<String>,
    pub working_directory: Option<String>,
}
