//! Local cache for workstreams to enable fast startup and offline viewing.
//!
//! The cache stores workstreams as JSON and tracks the last sync time.
//! On boot, cached data is loaded and marked as "stale" until refreshed.

use crate::config::{cache_path, Config};
use crate::data::Workstream;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

// =============================================================================
// Cache Data Structure
// =============================================================================

/// Cache file format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkstreamCache {
    /// Schema version for forward compatibility
    pub version: u32,

    /// Last successful sync timestamp
    pub last_sync: DateTime<Utc>,

    /// Cached workstreams
    pub workstreams: Vec<Workstream>,
}

impl Default for WorkstreamCache {
    fn default() -> Self {
        Self {
            version: 1,
            last_sync: DateTime::UNIX_EPOCH,
            workstreams: Vec::new(),
        }
    }
}

impl WorkstreamCache {
    /// Create a new cache with the given workstreams
    pub fn new(workstreams: Vec<Workstream>) -> Self {
        Self {
            version: 1,
            last_sync: Utc::now(),
            workstreams,
        }
    }

    /// Check if the cache is expired based on max age
    pub fn is_expired(&self, max_age_hours: u64) -> bool {
        let age = Utc::now().signed_duration_since(self.last_sync);
        let hours = age.num_hours().max(0) as u64;
        hours > max_age_hours
    }

    /// Check if the cache has any data
    pub fn is_empty(&self) -> bool {
        self.workstreams.is_empty()
    }
}

// =============================================================================
// Cache Operations
// =============================================================================

/// Load cache from disk
pub fn load_cache(config: &Config) -> Result<Option<WorkstreamCache>> {
    if !config.cache.enabled {
        return Ok(None);
    }

    let path = cache_path(config)?;

    if !path.exists() {
        return Ok(None);
    }

    load_cache_from_path(&path)
}

/// Load cache for a specific view (uses view-scoped cache file)
pub fn load_cache_for_view(config: &Config, view_name: &str) -> Result<Option<WorkstreamCache>> {
    if !config.cache.enabled {
        return Ok(None);
    }

    let path = cache_path_for_view(config, view_name)?;

    if !path.exists() {
        return Ok(None);
    }

    load_cache_from_path(&path)
}

/// Load cache from a specific path (for testing)
pub fn load_cache_from_path(path: &Path) -> Result<Option<WorkstreamCache>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read cache from {}", path.display()))?;

    let cache: WorkstreamCache = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse cache from {}", path.display()))?;

    // Check schema version
    if cache.version != 1 {
        tracing::warn!(
            "Cache version mismatch (expected 1, got {}), ignoring cache",
            cache.version
        );
        return Ok(None);
    }

    Ok(Some(cache))
}

/// Save cache to disk
pub fn save_cache(config: &Config, cache: &WorkstreamCache) -> Result<()> {
    if !config.cache.enabled {
        return Ok(());
    }

    let path = cache_path(config)?;
    save_cache_to_path(&path, cache)
}

/// Save cache for a specific view (uses view-scoped cache file)
pub fn save_cache_for_view(
    config: &Config,
    view_name: &str,
    cache: &WorkstreamCache,
) -> Result<()> {
    if !config.cache.enabled {
        return Ok(());
    }

    let path = cache_path_for_view(config, view_name)?;
    save_cache_to_path(&path, cache)
}

/// Get cache file path for a specific view.
/// Sanitizes the view name for use as a filename.
fn cache_path_for_view(_config: &Config, view_name: &str) -> Result<std::path::PathBuf> {
    let sanitized: String = view_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .to_lowercase();
    let filename = format!("workstreams_{}.json", sanitized);
    Ok(crate::config::config_dir()?.join(filename))
}

/// Save cache to a specific path (for testing)
pub fn save_cache_to_path(path: &Path, cache: &WorkstreamCache) -> Result<()> {
    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(cache).context("Failed to serialize cache")?;

    std::fs::write(path, content)
        .with_context(|| format!("Failed to write cache to {}", path.display()))?;

    Ok(())
}

/// Delete the cache file
pub fn clear_cache(config: &Config) -> Result<()> {
    let path = cache_path(config)?;

    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("Failed to delete cache at {}", path.display()))?;
    }

    Ok(())
}

// =============================================================================
// Incremental Sync Helpers
// =============================================================================

/// Merge updated workstreams into existing cache
///
/// Updates existing entries and adds new ones.
/// Does NOT remove entries (handles separately via full sync).
pub fn merge_workstreams(existing: &mut Vec<Workstream>, updates: Vec<Workstream>) {
    for update in updates {
        // Find existing entry by issue ID
        if let Some(pos) = existing
            .iter()
            .position(|w| w.linear_issue.id == update.linear_issue.id)
        {
            // Replace with updated version
            existing[pos] = update;
        } else {
            // Add new entry
            existing.push(update);
        }
    }
}

/// Remove workstreams that are no longer assigned
///
/// Call this after a full sync to clean up stale entries.
pub fn remove_unassigned(
    existing: &mut Vec<Workstream>,
    current_ids: &std::collections::HashSet<String>,
) {
    existing.retain(|w| current_ids.contains(&w.linear_issue.id));
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{LinearIssue, LinearPriority, LinearStatus};
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn make_workstream(id: &str, identifier: &str) -> Workstream {
        Workstream {
            linear_issue: LinearIssue {
                id: id.to_string(),
                identifier: identifier.to_string(),
                title: format!("Issue {}", identifier),
                description: None,
                url: format!("https://linear.app/test/issue/{}", identifier),
                status: LinearStatus::InProgress,
                priority: LinearPriority::Medium,
                cycle: None,
                labels: Vec::new(),
                project: None,
                team: None,
                assignee_id: None,
                assignee_name: None,
                estimate: None,
                created_at: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
                updated_at: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
                attachments: Vec::new(),
                parent: None,
                children: Vec::new(),
            },
            github_pr: None,
            vercel_deployment: None,
            agent_sessions: vec![],
            agent_session: None,
            stale: false,
        }
    }

    #[test]
    fn test_cache_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("cache.json");

        let workstreams = vec![
            make_workstream("1", "TEST-1"),
            make_workstream("2", "TEST-2"),
        ];

        let cache = WorkstreamCache::new(workstreams);
        save_cache_to_path(&cache_path, &cache).unwrap();

        let loaded = load_cache_from_path(&cache_path).unwrap().unwrap();
        assert_eq!(loaded.workstreams.len(), 2);
        assert_eq!(loaded.workstreams[0].linear_issue.id, "1");
    }

    #[test]
    fn test_cache_expired() {
        let cache = WorkstreamCache {
            last_sync: Utc::now() - chrono::Duration::hours(25),
            ..Default::default()
        };

        assert!(cache.is_expired(24));
        assert!(!cache.is_expired(48));
    }

    #[test]
    fn test_merge_workstreams_update() {
        let mut existing = vec![
            make_workstream("1", "TEST-1"),
            make_workstream("2", "TEST-2"),
        ];

        let mut updated = make_workstream("1", "TEST-1");
        updated.linear_issue.title = "Updated Title".to_string();

        merge_workstreams(&mut existing, vec![updated]);

        assert_eq!(existing.len(), 2);
        assert_eq!(existing[0].linear_issue.title, "Updated Title");
    }

    #[test]
    fn test_merge_workstreams_add_new() {
        let mut existing = vec![make_workstream("1", "TEST-1")];

        let new_ws = make_workstream("2", "TEST-2");
        merge_workstreams(&mut existing, vec![new_ws]);

        assert_eq!(existing.len(), 2);
    }

    #[test]
    fn test_remove_unassigned() {
        let mut existing = vec![
            make_workstream("1", "TEST-1"),
            make_workstream("2", "TEST-2"),
            make_workstream("3", "TEST-3"),
        ];

        let current_ids: std::collections::HashSet<String> =
            ["1".to_string(), "3".to_string()].into_iter().collect();

        remove_unassigned(&mut existing, &current_ids);

        assert_eq!(existing.len(), 2);
        assert!(existing.iter().all(|w| w.linear_issue.id != "2"));
    }

    #[test]
    fn test_load_missing_cache() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("nonexistent.json");

        let result = load_cache_from_path(&cache_path).unwrap();
        assert!(result.is_none());
    }
}
