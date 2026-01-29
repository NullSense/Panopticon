pub mod claude;
pub mod github;
pub mod linear;
pub mod moltbot;
pub mod vercel;

use crate::config::Config;
use crate::data::Workstream;
use anyhow::Result;

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
