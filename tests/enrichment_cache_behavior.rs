use panopticon::config::{
    CacheConfig, Config, GithubConfig, LinearConfig, NotificationConfig, PollingConfig, Tokens,
    UiConfig, VercelConfig,
};
use panopticon::data::{GitHubPR, GitHubPRStatus, VercelDeployment, VercelStatus};
use panopticon::integrations::enrichment_cache;

fn test_config() -> Config {
    Config {
        tokens: Tokens {
            linear: "lin_test".to_string(),
            github: "gh_test".to_string(),
            vercel: Some("vercel_test".to_string()),
        },
        linear: LinearConfig::default(),
        github: GithubConfig::default(),
        vercel: VercelConfig::default(),
        polling: PollingConfig {
            github_interval_secs: 60,
            vercel_interval_secs: 60,
            ..PollingConfig::default()
        },
        cache: CacheConfig {
            enabled: false,
            ..CacheConfig::default()
        },
        notifications: NotificationConfig::default(),
        ui: UiConfig::default(),
        views: vec![],
    }
}

#[tokio::test]
async fn github_cache_roundtrip_in_memory() {
    let config = test_config();

    let pr = GitHubPR {
        number: 123,
        title: "Test".to_string(),
        url: "https://github.com/o/r/pull/123".to_string(),
        status: GitHubPRStatus::Open,
        branch: "feat".to_string(),
        repo: "o/r".to_string(),
    };

    let key = enrichment_cache::normalize_github_pr_url(&pr.url);
    enrichment_cache::set_cached_github_pr(&config, &key, pr.clone()).await;

    let (cached, fresh) = enrichment_cache::get_cached_github_pr(&config, &key, 60)
        .await
        .expect("expected cached");

    assert!(fresh);
    assert_eq!(cached.number, 123);
    assert_eq!(cached.branch, "feat");
}

#[tokio::test]
async fn github_backoff_is_set_when_remaining_zero() {
    let config = test_config();

    let reset = chrono::Utc::now().timestamp() + 600;
    enrichment_cache::mark_github_rate_limited(&config, Some(0), Some(reset)).await;

    assert!(enrichment_cache::github_should_backoff(&config).await);
}

#[tokio::test]
async fn vercel_cache_and_backoff() {
    let config = test_config();

    let dep = VercelDeployment {
        id: "d1".to_string(),
        url: "https://example.vercel.app".to_string(),
        status: VercelStatus::Ready,
        created_at: chrono::Utc::now(),
    };

    let key = enrichment_cache::vercel_key("o/r", "feat");
    enrichment_cache::set_cached_vercel(&config, &key, Some(dep.clone())).await;

    let (cached, fresh) = enrichment_cache::get_cached_vercel(&config, &key, 60)
        .await
        .expect("expected cached");

    assert!(fresh);
    assert_eq!(cached.unwrap().id, "d1");

    enrichment_cache::mark_vercel_rate_limited(&config, Some(10)).await;
    assert!(enrichment_cache::vercel_should_backoff(&config).await);
}
