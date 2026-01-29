pub mod agent_cache;
pub mod cache;
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
                    tracing::debug!("Failed to fetch Vercel deployment for {}/{}: {}", pr.repo, pr.branch, e);
                    None
                }
            }
        } else {
            None
        };

        // Find agent session via O(1) cache lookup (instead of file read + HTTP call)
        let agent = agent_cache.find_for_directory(issue.working_directory.as_deref());

        workstreams.push(Workstream {
            linear_issue: issue.issue,
            github_pr: pr,
            vercel_deployment: deployment,
            agent_session: agent,
            stale: false,
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
    let config = config.clone();
    stream::iter(issues.into_iter().enumerate())
        .map(|(i, issue)| {
            let config = config.clone();
            let tx = tx.clone();
            let agent_cache = Arc::clone(&agent_cache);
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
                            tracing::debug!("Failed to fetch Vercel deployment for {}/{}: {}", pr.repo, pr.branch, e);
                            None
                        }
                    }
                } else {
                    None
                };

                // Find agent session via O(1) cache lookup (instead of file read + HTTP call)
                let agent = agent_cache.find_for_directory(issue.working_directory.as_deref());

                let ws = Workstream {
                    linear_issue: issue.issue,
                    github_pr: pr,
                    vercel_deployment: deployment,
                    agent_session: agent,
                    stale: false,
                };

                if let Err(e) = tx.send(RefreshResult::Workstream(ws)).await {
                    tracing::debug!("Workstream channel closed: {}", e);
                }
            }
        })
        .buffer_unordered(5) // Process 5 issues concurrently
        .collect::<Vec<_>>()
        .await;

    if let Err(e) = tx.send(RefreshResult::Complete).await {
        tracing::debug!("Complete channel closed: {}", e);
    }
    Ok(())
}

/// Intermediate struct for Linear issues with extra linking info
pub struct LinkedLinearIssue {
    pub issue: crate::data::LinearIssue,
    pub linked_pr_url: Option<String>,
    pub working_directory: Option<String>,
}
