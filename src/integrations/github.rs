use crate::config::Config;
use crate::data::{GitHubPR, GitHubPRStatus};
use anyhow::{Context, Result};

const GITHUB_API_URL: &str = "https://api.github.com";

/// Fetch PR details from a GitHub PR URL
pub async fn fetch_pr_from_url(config: &Config, pr_url: &str) -> Result<GitHubPR> {
    // Parse URL: https://github.com/owner/repo/pull/123
    let parts: Vec<&str> = pr_url.trim_end_matches('/').split('/').collect();
    let pr_number: u64 = parts
        .last()
        .context("Invalid PR URL")?
        .parse()
        .context("Invalid PR number")?;

    let repo_idx = parts.iter().position(|&s| s == "github.com").unwrap_or(0) + 1;
    let owner = parts.get(repo_idx).context("Missing owner")?;
    let repo = parts.get(repo_idx + 1).context("Missing repo")?;

    fetch_pr(config, owner, repo, pr_number).await
}

/// Fetch PR details from GitHub API
pub async fn fetch_pr(config: &Config, owner: &str, repo: &str, number: u64) -> Result<GitHubPR> {
    let client = reqwest::Client::new();

    let url = format!("{}/repos/{}/{}/pulls/{}", GITHUB_API_URL, owner, repo, number);

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", config.tokens.github))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "panopticon")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("GitHub API error: {}", response.status());
    }

    let pr: serde_json::Value = response.json().await?;

    // Fetch reviews to determine approval status
    let reviews_url = format!(
        "{}/repos/{}/{}/pulls/{}/reviews",
        GITHUB_API_URL, owner, repo, number
    );
    let reviews_response = client
        .get(&reviews_url)
        .header("Authorization", format!("Bearer {}", config.tokens.github))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "panopticon")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?;

    let reviews: Vec<serde_json::Value> = if reviews_response.status().is_success() {
        reviews_response.json().await.unwrap_or_default()
    } else {
        vec![]
    };

    let status = determine_pr_status(&pr, &reviews);

    Ok(GitHubPR {
        number,
        title: pr["title"].as_str().unwrap_or("").to_string(),
        url: pr["html_url"].as_str().unwrap_or("").to_string(),
        status,
        branch: pr["head"]["ref"].as_str().unwrap_or("").to_string(),
        repo: format!("{}/{}", owner, repo),
    })
}

fn determine_pr_status(pr: &serde_json::Value, reviews: &[serde_json::Value]) -> GitHubPRStatus {
    // Check if merged or closed
    if pr["merged"].as_bool().unwrap_or(false) {
        return GitHubPRStatus::Merged;
    }
    if pr["state"].as_str() == Some("closed") {
        return GitHubPRStatus::Closed;
    }

    // Check if draft
    if pr["draft"].as_bool().unwrap_or(false) {
        return GitHubPRStatus::Draft;
    }

    // Analyze reviews
    let mut has_approval = false;
    let mut has_changes_requested = false;

    for review in reviews {
        match review["state"].as_str() {
            Some("APPROVED") => has_approval = true,
            Some("CHANGES_REQUESTED") => has_changes_requested = true,
            _ => {}
        }
    }

    if has_changes_requested {
        GitHubPRStatus::ChangesRequested
    } else if has_approval {
        GitHubPRStatus::Approved
    } else if pr["requested_reviewers"]
        .as_array()
        .map(|a| !a.is_empty())
        .unwrap_or(false)
    {
        GitHubPRStatus::ReviewRequested
    } else {
        GitHubPRStatus::Open
    }
}
