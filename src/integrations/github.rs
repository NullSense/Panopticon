use crate::config::Config;
use crate::data::{GitHubPR, GitHubPRStatus};
use crate::integrations::enrichment_cache::{self, AsyncTtlCache, Cached};
use crate::integrations::HTTP_CLIENT;
use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use std::time::Duration;

const GITHUB_API_URL: &str = "https://api.github.com";

static PR_CACHE: Lazy<AsyncTtlCache<String, Cached<GitHubPR>>> = Lazy::new(AsyncTtlCache::default);

/// Fetch PR details from a GitHub PR URL.
///
/// Uses an in-memory TTL cache + request coalescing to drastically reduce
/// GitHub API usage during refresh.
pub async fn fetch_pr_from_url(config: &Config, pr_url: &str) -> Result<GitHubPR> {
    let key = enrichment_cache::normalize_github_pr_url(pr_url);

    // Persisted cache: if fresh, return immediately (prevents re-fetch every Linear refresh).
    if let Some((pr, true)) =
        enrichment_cache::get_cached_github_pr(config, &key, config.polling.github_interval_secs)
            .await
    {
        return Ok(pr);
    }

    // If we are in backoff, serve stale cached value if available.
    if enrichment_cache::github_should_backoff(config).await {
        if let Some((pr, _fresh)) =
            enrichment_cache::get_cached_github_pr(config, &key, config.polling.github_interval_secs)
                .await
        {
            return Ok(pr);
        }
    }

    let cached = PR_CACHE
        .get_or_try_init_with_ttl(key.clone(), || async {
            let outcome = fetch_pr_from_url_uncached(config, pr_url).await;
            outcome_to_cached(outcome)
        })
        .await;

    let pr = cached.into_result()?;
    enrichment_cache::set_cached_github_pr(config, &key, pr.clone()).await;
    Ok(pr)
}

/// Fetch PR details from GitHub API.
///
/// Also cached.
pub async fn fetch_pr(config: &Config, owner: &str, repo: &str, number: u64) -> Result<GitHubPR> {
    let key = format!("{}/{}/pull/{}", owner, repo, number);

    let cached = PR_CACHE
        .get_or_try_init_with_ttl(key, || async {
            let outcome = fetch_pr_uncached(config, owner, repo, number).await;
            outcome_to_cached(outcome)
        })
        .await;

    cached.into_result()
}

struct FetchError {
    msg: String,
    backoff: Duration,
}

type FetchOutcome<T> = std::result::Result<T, FetchError>;

fn outcome_to_cached(outcome: FetchOutcome<GitHubPR>) -> (Cached<GitHubPR>, Duration) {
    match outcome {
        Ok(pr) => {
            let ttl = ttl_for_pr_status(pr.status);
            (Cached::Ok(pr), ttl)
        }
        Err(e) => (Cached::Err(e.msg), e.backoff),
    }
}

fn ttl_for_pr_status(status: GitHubPRStatus) -> Duration {
    match status {
        GitHubPRStatus::Merged | GitHubPRStatus::Closed => Duration::from_secs(30 * 60),
        GitHubPRStatus::Approved
        | GitHubPRStatus::ChangesRequested
        | GitHubPRStatus::ReviewRequested => Duration::from_secs(2 * 60),
        GitHubPRStatus::Open | GitHubPRStatus::Draft => Duration::from_secs(60),
    }
}

fn backoff_from_rate_limit_headers(response: &reqwest::Response) -> Option<Duration> {
    let remaining = response
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    if remaining != Some(0) {
        return None;
    }

    let reset_epoch = response
        .headers()
        .get("x-ratelimit-reset")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let Some(reset_epoch) = reset_epoch else {
        return Some(Duration::from_secs(60));
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    let wait = reset_epoch.saturating_sub(now);
    Some(Duration::from_secs(wait.clamp(10, 10 * 60)))
}

async fn fetch_pr_from_url_uncached(config: &Config, pr_url: &str) -> FetchOutcome<GitHubPR> {
    // Parse URL: https://github.com/owner/repo/pull/123
    let parts: Vec<&str> = pr_url.trim_end_matches('/').split('/').collect();
    let pr_number: u64 = parts
        .last()
        .context("Invalid PR URL")
        .and_then(|s| s.parse().context("Invalid PR number"))
        .map_err(anyhow_to_fetch_error)?;

    let repo_idx = parts.iter().position(|&s| s == "github.com").unwrap_or(0) + 1;
    let owner = parts.get(repo_idx).context("Missing owner").map_err(anyhow_to_fetch_error)?;
    let repo = parts
        .get(repo_idx + 1)
        .context("Missing repo")
        .map_err(anyhow_to_fetch_error)?;

    fetch_pr_uncached(config, owner, repo, pr_number).await
}

async fn fetch_pr_uncached(
    config: &Config,
    owner: &str,
    repo: &str,
    number: u64,
) -> FetchOutcome<GitHubPR> {
    // Prefer GraphQL (1 call instead of pulls+reviews).
    // If GraphQL fails, fall back to REST.
    match fetch_pr_graphql(config, owner, repo, number).await {
        Ok(pr) => Ok(pr),
        Err(e) => {
            // If it was a rate-limit, don't hammer the fallback.
            if e.msg.contains("rate limited") {
                return Err(e);
            }
            tracing::debug!("GitHub GraphQL failed ({}), falling back to REST", e.msg);
            fetch_pr_rest(config, owner, repo, number).await
        }
    }
}

async fn fetch_pr_graphql(
    config: &Config,
    owner: &str,
    repo: &str,
    number: u64,
) -> FetchOutcome<GitHubPR> {
    let client = &*HTTP_CLIENT;

    // reviewDecision provides overall review state without calling /reviews.
    let query = r#"
      query($owner:String!, $repo:String!, $number:Int!) {
        repository(owner:$owner, name:$repo) {
          pullRequest(number:$number) {
            number
            title
            url
            state
            isDraft
            merged
            headRefName
            reviewDecision
          }
        }
      }
    "#;

    let variables = serde_json::json!({
        "owner": owner,
        "repo": repo,
        "number": number as i64,
    });

    let response = client
        .post(format!("{}/graphql", GITHUB_API_URL))
        .header("Authorization", format!("Bearer {}", config.tokens.github))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "panopticon")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&serde_json::json!({"query": query, "variables": variables}))
        .send()
        .await
        .map_err(|e| anyhow_to_fetch_error(e.into()))?;

    if response.status().as_u16() == 403 || response.status().as_u16() == 429 {
        let backoff = backoff_from_rate_limit_headers(&response).unwrap_or(Duration::from_secs(60));
        return Err(FetchError {
            msg: format!("GitHub API rate limited: {}", response.status()),
            backoff,
        });
    }

    if !response.status().is_success() {
        return Err(FetchError {
            msg: format!("GitHub API error: {}", response.status()),
            backoff: Duration::from_secs(60),
        });
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| anyhow_to_fetch_error(e.into()))?;

    if body
        .get("errors")
        .is_some_and(|e| e.is_array() && !e.as_array().unwrap().is_empty())
    {
        return Err(FetchError {
            msg: format!("GitHub GraphQL returned errors: {}", body["errors"]),
            backoff: Duration::from_secs(60),
        });
    }

    let pr = &body["data"]["repository"]["pullRequest"];

    let state = pr["state"].as_str().unwrap_or("OPEN");
    let merged = pr["merged"].as_bool().unwrap_or(false);
    let is_draft = pr["isDraft"].as_bool().unwrap_or(false);

    let status = if merged {
        GitHubPRStatus::Merged
    } else if state == "CLOSED" {
        GitHubPRStatus::Closed
    } else if is_draft {
        GitHubPRStatus::Draft
    } else {
        match pr["reviewDecision"].as_str() {
            Some("CHANGES_REQUESTED") => GitHubPRStatus::ChangesRequested,
            Some("APPROVED") => GitHubPRStatus::Approved,
            Some("REVIEW_REQUIRED") => GitHubPRStatus::ReviewRequested,
            _ => GitHubPRStatus::Open,
        }
    };

    Ok(GitHubPR {
        number: pr["number"].as_u64().unwrap_or(number),
        title: pr["title"].as_str().unwrap_or("").to_string(),
        url: pr["url"].as_str().unwrap_or("").to_string(),
        status,
        branch: pr["headRefName"].as_str().unwrap_or("").to_string(),
        repo: format!("{}/{}", owner, repo),
    })
}

/// Fetch PR details from GitHub REST API.
///
/// Fallback path; makes 1-2 calls depending on PR state.
async fn fetch_pr_rest(
    config: &Config,
    owner: &str,
    repo: &str,
    number: u64,
) -> FetchOutcome<GitHubPR> {
    let client = &*HTTP_CLIENT;

    let url = format!(
        "{}/repos/{}/{}/pulls/{}",
        GITHUB_API_URL, owner, repo, number
    );

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", config.tokens.github))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "panopticon")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| anyhow_to_fetch_error(e.into()))?;

    if response.status().as_u16() == 403 || response.status().as_u16() == 429 {
        let backoff = backoff_from_rate_limit_headers(&response).unwrap_or(Duration::from_secs(60));
        return Err(FetchError {
            msg: format!("GitHub API rate limited: {}", response.status()),
            backoff,
        });
    }

    if !response.status().is_success() {
        return Err(FetchError {
            msg: format!("GitHub API error: {}", response.status()),
            backoff: Duration::from_secs(60),
        });
    }

    let pr: serde_json::Value = response
        .json()
        .await
        .map_err(|e| anyhow_to_fetch_error(e.into()))?;

    // For merged/closed/draft we don't need a reviews call.
    if pr["merged"].as_bool().unwrap_or(false)
        || pr["state"].as_str() == Some("closed")
        || pr["draft"].as_bool().unwrap_or(false)
    {
        let status = determine_pr_status(&pr, &[]);
        return Ok(GitHubPR {
            number,
            title: pr["title"].as_str().unwrap_or("").to_string(),
            url: pr["html_url"].as_str().unwrap_or("").to_string(),
            status,
            branch: pr["head"]["ref"].as_str().unwrap_or("").to_string(),
            repo: format!("{}/{}", owner, repo),
        });
    }

    // Fetch reviews to determine approval status (fallback-only path).
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
        .await
        .map_err(|e| anyhow_to_fetch_error(e.into()))?;

    if reviews_response.status().as_u16() == 403 || reviews_response.status().as_u16() == 429 {
        let status = determine_pr_status(&pr, &[]);
        // Rate limit on reviews: return PR basics and cache with a moderate TTL.
        return Ok(GitHubPR {
            number,
            title: pr["title"].as_str().unwrap_or("").to_string(),
            url: pr["html_url"].as_str().unwrap_or("").to_string(),
            status,
            branch: pr["head"]["ref"].as_str().unwrap_or("").to_string(),
            repo: format!("{}/{}", owner, repo),
        });
    }

    let reviews: Vec<serde_json::Value> = if reviews_response.status().is_success() {
        reviews_response
            .json()
            .await
            .unwrap_or_default()
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

fn anyhow_to_fetch_error(e: anyhow::Error) -> FetchError {
    FetchError {
        msg: e.to_string(),
        backoff: Duration::from_secs(60),
    }
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

    // Track latest review per reviewer to avoid "sticky CHANGES_REQUESTED" bug
    // A reviewer may request changes, then later approve - only the latest matters
    use std::collections::HashMap;
    let mut latest_by_reviewer: HashMap<&str, (&str, &str)> = HashMap::new();

    for review in reviews {
        let Some(reviewer) = review["user"]["login"].as_str() else {
            continue;
        };
        let Some(state) = review["state"].as_str() else {
            continue;
        };
        let submitted_at = review["submitted_at"].as_str().unwrap_or("");

        // Keep only the latest review from each reviewer
        // ISO 8601 timestamps can be compared lexicographically
        latest_by_reviewer
            .entry(reviewer)
            .and_modify(|(current_state, current_time)| {
                if submitted_at > *current_time {
                    *current_state = state;
                    *current_time = submitted_at;
                }
            })
            .or_insert((state, submitted_at));
    }

    // Aggregate latest reviews from all reviewers
    let mut has_approval = false;
    let mut has_changes_requested = false;

    for (state, _) in latest_by_reviewer.values() {
        match *state {
            "APPROVED" => has_approval = true,
            "CHANGES_REQUESTED" => has_changes_requested = true,
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
        || pr["requested_teams"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false)
    {
        // Check both individual reviewers AND team review requests
        GitHubPRStatus::ReviewRequested
    } else {
        GitHubPRStatus::Open
    }
}
