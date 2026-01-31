//! Enrichment caching (GitHub PR + Vercel deployment).
//!
//! This module provides:
//! - `AsyncTtlCache`: in-memory TTL cache with request coalescing (singleflight)
//! - A persisted JSON cache for warm-starts
//! - Simple backoff flags for rate limits (GitHub + Vercel)
//!
//! The UI refresh loop can be more frequent than GitHub/Vercel polling;
//! this cache ensures we don't spam external APIs while keeping status
//! reasonably fresh.

use crate::config::{config_dir, Config};
use crate::data::{GitHubPR, VercelDeployment};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify, OnceCell, RwLock};

// =============================================================================
// In-memory TTL cache with request coalescing
// =============================================================================

#[derive(Debug, Clone)]
pub enum Cached<T> {
    Ok(T),
    Err(String),
}

impl<T> Cached<T> {
    pub fn into_result(self) -> anyhow::Result<T> {
        match self {
            Self::Ok(v) => Ok(v),
            Self::Err(e) => Err(anyhow::anyhow!(e)),
        }
    }

    pub fn as_ref(&self) -> Cached<&T> {
        match self {
            Self::Ok(v) => Cached::Ok(v),
            Self::Err(e) => Cached::Err(e.clone()),
        }
    }
}

#[derive(Debug)]
enum Entry<V> {
    Ready { value: Arc<V>, expires_at: Instant },
    Loading { notify: Arc<Notify> },
}

/// An async TTL cache with request coalescing.
///
/// - If a key is fresh, returns cached value.
/// - If a key is being fetched, waits for the in-flight fetch to complete.
/// - Otherwise starts a fetch, stores the result with a caller-provided TTL.
#[derive(Debug)]
pub struct AsyncTtlCache<K, V> {
    inner: Mutex<HashMap<K, Entry<V>>>,
}

impl<K, V> Default for AsyncTtlCache<K, V>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }
}

impl<K, V> AsyncTtlCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    /// Get from cache or fetch.
    ///
    /// The fetcher returns `(value, ttl)` so callers can dynamically tune TTL.
    pub async fn get_or_try_init_with_ttl<F, Fut>(&self, key: K, fetcher: F) -> V
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = (V, Duration)>,
    {
        loop {
            // Fast path: hit fresh entry.
            let notify_to_wait = {
                let mut guard = self.inner.lock().await;
                match guard.get(&key) {
                    Some(Entry::Ready { value, expires_at }) => {
                        if Instant::now() < *expires_at {
                            return (**value).clone();
                        }
                        // Expired -> treat like miss.
                        guard.remove(&key);
                        None
                    }
                    Some(Entry::Loading { notify }) => Some(Arc::clone(notify)),
                    None => {
                        let notify = Arc::new(Notify::new());
                        guard.insert(key.clone(), Entry::Loading { notify: notify.clone() });
                        // We are the leader.
                        drop(guard);

                        let (value, ttl) = fetcher().await;
                        let expires_at = Instant::now() + ttl;

                        let mut guard = self.inner.lock().await;
                        // Extract the notify from the loading entry (if still present).
                        let notify = match guard.remove(&key) {
                            Some(Entry::Loading { notify }) => notify,
                            _ => Arc::new(Notify::new()),
                        };

                        // Store the fetched value.
                        guard.insert(
                            key.clone(),
                            Entry::Ready {
                                value: Arc::new(value.clone()),
                                expires_at,
                            },
                        );

                        // Wake all waiters.
                        notify.notify_waiters();
                        return value;
                    }
                }
            };

            // Follower path: wait for the in-flight fetch.
            if let Some(notify) = notify_to_wait {
                notify.notified().await;
                continue;
            }
        }
    }
}

// =============================================================================
// Persisted enrichment cache + rate limit backoff
// =============================================================================

const CACHE_VERSION: u32 = 1;
const CACHE_FILE_NAME: &str = "enrichment-cache.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PersistedEnrichmentCache {
    version: u32,

    github_backoff_until: Option<DateTime<Utc>>,
    vercel_backoff_until: Option<DateTime<Utc>>,
    vercel_backoff_secs: Option<u64>,

    github: HashMap<String, PersistedValue<GitHubPR>>,
    vercel: HashMap<String, PersistedValue<Option<VercelDeployment>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedValue<T> {
    fetched_at: DateTime<Utc>,
    value: T,
}

impl<T> PersistedValue<T> {
    fn is_fresh(&self, ttl_secs: u64) -> bool {
        let age = Utc::now().signed_duration_since(self.fetched_at);
        age.num_seconds() >= 0 && (age.num_seconds() as u64) < ttl_secs
    }
}

fn persisted_cache_path() -> Result<PathBuf> {
    Ok(config_dir()?.join(CACHE_FILE_NAME))
}

fn load_from_path(path: &Path) -> Result<PersistedEnrichmentCache> {
    if !path.exists() {
        return Ok(PersistedEnrichmentCache {
            version: CACHE_VERSION,
            ..Default::default()
        });
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read enrichment cache from {}", path.display()))?;

    let cache: PersistedEnrichmentCache = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse enrichment cache from {}", path.display()))?;

    if cache.version != CACHE_VERSION {
        tracing::warn!(
            "Enrichment cache version mismatch (expected {}, got {}), ignoring",
            CACHE_VERSION,
            cache.version
        );
        return Ok(PersistedEnrichmentCache {
            version: CACHE_VERSION,
            ..Default::default()
        });
    }

    Ok(cache)
}

fn save_to_path(path: &Path, cache: &PersistedEnrichmentCache) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(cache).context("Failed to serialize enrichment cache")?;
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write enrichment cache to {}", path.display()))?;
    Ok(())
}

static GLOBAL_PERSISTED: OnceCell<Arc<RwLock<PersistedEnrichmentCache>>> = OnceCell::const_new();

async fn persisted(config: &Config) -> Arc<RwLock<PersistedEnrichmentCache>> {
    GLOBAL_PERSISTED
        .get_or_init(|| async {
            let c = if config.cache.enabled {
                match persisted_cache_path().and_then(|p| load_from_path(&p)) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!("Failed to load enrichment cache: {e}");
                        PersistedEnrichmentCache {
                            version: CACHE_VERSION,
                            ..Default::default()
                        }
                    }
                }
            } else {
                PersistedEnrichmentCache {
                    version: CACHE_VERSION,
                    ..Default::default()
                }
            };
            Arc::new(RwLock::new(c))
        })
        .await
        .clone()
}

async fn persist_if_enabled(config: &Config, cache: &PersistedEnrichmentCache) {
    if !config.cache.enabled {
        return;
    }
    let Ok(path) = persisted_cache_path() else {
        return;
    };
    if let Err(e) = save_to_path(&path, cache) {
        tracing::warn!("Failed to persist enrichment cache: {e}");
    }
}

pub fn normalize_github_pr_url(pr_url: &str) -> String {
    pr_url.trim().trim_end_matches('/').to_string()
}

pub fn vercel_key(repo: &str, branch: &str) -> String {
    format!("{}:{}", repo, branch)
}

fn github_in_backoff(cache: &PersistedEnrichmentCache) -> bool {
    cache
        .github_backoff_until
        .is_some_and(|t| t > Utc::now())
}

fn vercel_in_backoff(cache: &PersistedEnrichmentCache) -> bool {
    cache
        .vercel_backoff_until
        .is_some_and(|t| t > Utc::now())
}

pub async fn github_should_backoff(config: &Config) -> bool {
    let c = persisted(config).await;
    let c = c.read().await;
    github_in_backoff(&c)
}

pub async fn vercel_should_backoff(config: &Config) -> bool {
    let c = persisted(config).await;
    let c = c.read().await;
    vercel_in_backoff(&c)
}

pub async fn get_cached_github_pr(
    config: &Config,
    key: &str,
    ttl_secs: u64,
) -> Option<(GitHubPR, bool /*fresh*/)> {
    let c = persisted(config).await;
    let c = c.read().await;
    let entry = c.github.get(key)?;
    Some((entry.value.clone(), entry.is_fresh(ttl_secs)))
}

pub async fn set_cached_github_pr(config: &Config, key: &str, pr: GitHubPR) {
    let c = persisted(config).await;
    let mut c = c.write().await;

    c.github.insert(
        key.to_string(),
        PersistedValue {
            fetched_at: Utc::now(),
            value: pr,
        },
    );

    let snapshot = c.clone();
    drop(c);
    persist_if_enabled(config, &snapshot).await;
}

pub async fn get_cached_vercel(
    config: &Config,
    key: &str,
    ttl_secs: u64,
) -> Option<(Option<VercelDeployment>, bool /*fresh*/)> {
    let c = persisted(config).await;
    let c = c.read().await;
    let entry = c.vercel.get(key)?;
    Some((entry.value.clone(), entry.is_fresh(ttl_secs)))
}

pub async fn set_cached_vercel(config: &Config, key: &str, value: Option<VercelDeployment>) {
    let c = persisted(config).await;
    let mut c = c.write().await;

    c.vercel.insert(
        key.to_string(),
        PersistedValue {
            fetched_at: Utc::now(),
            value,
        },
    );

    let snapshot = c.clone();
    drop(c);
    persist_if_enabled(config, &snapshot).await;
}

pub async fn mark_github_rate_limited(
    config: &Config,
    remaining: Option<u64>,
    reset_epoch_secs: Option<i64>,
) {
    let c = persisted(config).await;
    let mut c = c.write().await;

    // If remaining is 0, stop calling until reset.
    if remaining.is_some_and(|r| r == 0) {
        if let Some(reset) = reset_epoch_secs {
            if let Some(reset_dt) = Utc.timestamp_opt(reset, 0).single() {
                c.github_backoff_until = Some(reset_dt);
            }
        } else {
            c.github_backoff_until = Some(Utc::now() + ChronoDuration::minutes(5));
        }
    }

    // Proactive: if remaining is very low, also back off briefly.
    if remaining.is_some_and(|r| r <= 2) {
        c.github_backoff_until = Some(Utc::now() + ChronoDuration::minutes(1));
    }

    let snapshot = c.clone();
    drop(c);
    persist_if_enabled(config, &snapshot).await;
}

pub async fn mark_vercel_rate_limited(config: &Config, retry_after_secs: Option<u64>) {
    let c = persisted(config).await;
    let mut c = c.write().await;

    let next = match (c.vercel_backoff_secs, retry_after_secs) {
        (_, Some(explicit)) => explicit,
        (None, None) => 5,
        (Some(prev), None) => (prev.saturating_mul(2)).min(300),
    };

    c.vercel_backoff_secs = Some(next);
    c.vercel_backoff_until = Some(Utc::now() + ChronoDuration::seconds(next as i64));

    let snapshot = c.clone();
    drop(c);
    persist_if_enabled(config, &snapshot).await;
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn coalesces_concurrent_requests() {
        let cache: Arc<AsyncTtlCache<String, Cached<u32>>> = Arc::new(AsyncTtlCache::default());
        let calls = Arc::new(AtomicUsize::new(0));

        let mut tasks = Vec::new();
        for _ in 0..10 {
            let cache = Arc::clone(&cache);
            let calls = Arc::clone(&calls);
            tasks.push(tokio::spawn(async move {
                cache
                    .get_or_try_init_with_ttl("k".to_string(), move || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        (Cached::Ok(42), Duration::from_secs(60))
                    })
                    .await
            }));
        }

        let values: Vec<_> = futures::future::join_all(tasks)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(values.iter().all(|v| matches!(v, Cached::Ok(42))));
    }

    #[tokio::test]
    async fn expires_and_refetches() {
        let cache: AsyncTtlCache<String, u32> = AsyncTtlCache::default();
        let calls = Arc::new(AtomicUsize::new(0));

        let v1 = {
            let calls = Arc::clone(&calls);
            cache
                .get_or_try_init_with_ttl("k".to_string(), move || async move {
                    let n = calls.fetch_add(1, Ordering::SeqCst) as u32;
                    (n, Duration::from_millis(30))
                })
                .await
        };
        assert_eq!(v1, 0);

        tokio::time::sleep(Duration::from_millis(10)).await;
        let v2 = cache
            .get_or_try_init_with_ttl("k".to_string(), || async { (999, Duration::from_secs(1)) })
            .await;
        assert_eq!(v2, 0, "should still be cached before expiry");

        tokio::time::sleep(Duration::from_millis(30)).await; // now past expiry
        let v3 = {
            let calls = Arc::clone(&calls);
            cache
                .get_or_try_init_with_ttl("k".to_string(), move || async move {
                    let n = calls.fetch_add(1, Ordering::SeqCst) as u32;
                    (n, Duration::from_secs(1))
                })
                .await
        };

        assert_eq!(v3, 1, "should refetch after expiry");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
