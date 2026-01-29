use crate::config::Config;
use crate::data::{VercelDeployment, VercelStatus};
use crate::integrations::HTTP_CLIENT;
use anyhow::Result;
use chrono::Utc;

const VERCEL_API_URL: &str = "https://api.vercel.com";

/// Fetch the latest deployment for a given branch
pub async fn fetch_deployment_for_branch(
    config: &Config,
    _repo: &str,
    branch: &str,
) -> Result<Option<VercelDeployment>> {
    let token = match &config.tokens.vercel {
        Some(t) => t,
        None => return Ok(None),
    };

    let client = &*HTTP_CLIENT;

    // Query deployments filtered by branch (meta.githubCommitRef)
    let url = format!(
        "{}/v6/deployments?limit=1&meta-githubCommitRef={}",
        VERCEL_API_URL, branch
    );

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if !response.status().is_success() {
        tracing::warn!("Vercel API error: {}", response.status());
        return Ok(None);
    }

    let body: serde_json::Value = response.json().await?;

    let deployment = body["deployments"]
        .as_array()
        .and_then(|deps| deps.first())
        .map(|d| {
            let state = d["readyState"].as_str().unwrap_or("QUEUED");
            let status = match state {
                "QUEUED" => VercelStatus::Queued,
                "BUILDING" => VercelStatus::Building,
                "READY" => VercelStatus::Ready,
                "ERROR" => VercelStatus::Error,
                "CANCELED" => VercelStatus::Canceled,
                _ => VercelStatus::Queued,
            };

            VercelDeployment {
                id: d["uid"].as_str().unwrap_or("").to_string(),
                url: d["url"]
                    .as_str()
                    .map(|u| format!("https://{}", u))
                    .unwrap_or_default(),
                status,
                created_at: d["createdAt"]
                    .as_i64()
                    .map(|ts| {
                        chrono::DateTime::from_timestamp_millis(ts)
                            .unwrap_or_else(Utc::now)
                    })
                    .unwrap_or_else(Utc::now),
            }
        });

    Ok(deployment)
}

/// Alternative: Get deployment status from GitHub commit statuses
/// This works even without Vercel token if Vercel GitHub integration is set up
#[allow(dead_code)]
pub async fn fetch_deployment_from_github_status(
    config: &Config,
    owner: &str,
    repo: &str,
    commit_sha: &str,
) -> Result<Option<VercelDeployment>> {
    let client = &*HTTP_CLIENT;

    let url = format!(
        "https://api.github.com/repos/{}/{}/commits/{}/statuses",
        owner, repo, commit_sha
    );

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", config.tokens.github))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "panopticon")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let statuses: Vec<serde_json::Value> = response.json().await?;

    // Find Vercel deployment status
    let vercel_status = statuses.iter().find(|s| {
        s["context"]
            .as_str()
            .map(|c| c.contains("vercel"))
            .unwrap_or(false)
    });

    let deployment = vercel_status.map(|s| {
        let state = s["state"].as_str().unwrap_or("pending");
        let status = match state {
            "pending" => VercelStatus::Building,
            "success" => VercelStatus::Ready,
            "failure" | "error" => VercelStatus::Error,
            _ => VercelStatus::Queued,
        };

        VercelDeployment {
            id: s["id"].to_string(),
            url: s["target_url"].as_str().unwrap_or("").to_string(),
            status,
            created_at: s["created_at"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(Utc::now),
        }
    });

    Ok(deployment)
}
