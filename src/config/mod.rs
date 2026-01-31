use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// =============================================================================
// Main Config Structure
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub tokens: Tokens,
    #[serde(default)]
    pub linear: LinearConfig,
    #[serde(default)]
    pub github: GithubConfig,
    #[serde(default)]
    pub vercel: VercelConfig,
    #[serde(default)]
    pub polling: PollingConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub notifications: NotificationConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

// =============================================================================
// API Tokens
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tokens {
    pub linear: String,
    pub github: String,
    #[serde(default)]
    pub vercel: Option<String>,
}

// =============================================================================
// Linear Configuration
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearConfig {
    /// Filter query for Linear issues (e.g., "assignee:me")
    #[serde(default = "default_linear_filter")]
    pub filter: String,

    /// Maximum issues to fetch per API call
    #[serde(default = "default_linear_fetch_limit")]
    pub fetch_limit: usize,

    /// Enable incremental sync (only fetch updated issues)
    #[serde(default = "default_true")]
    pub incremental_sync: bool,
}

impl Default for LinearConfig {
    fn default() -> Self {
        Self {
            filter: default_linear_filter(),
            fetch_limit: default_linear_fetch_limit(),
            incremental_sync: true,
        }
    }
}

fn default_linear_filter() -> String {
    "assignee:me".to_string()
}

fn default_linear_fetch_limit() -> usize {
    150
}

// =============================================================================
// GitHub Configuration
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GithubConfig {
    /// GitHub username (for filtering PRs)
    #[serde(default)]
    pub username: Option<String>,

    /// Organizations to include
    #[serde(default)]
    pub organizations: Vec<String>,
}

// =============================================================================
// Vercel Configuration
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VercelConfig {
    /// Team ID or slug
    #[serde(default)]
    pub team_id: Option<String>,

    /// Project IDs to monitor
    #[serde(default)]
    pub project_ids: Vec<String>,
}

// =============================================================================
// Polling Configuration
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingConfig {
    /// Linear refresh interval in seconds
    #[serde(default = "default_linear_interval")]
    pub linear_interval_secs: u64,

    /// GitHub refresh interval in seconds
    #[serde(default = "default_github_interval")]
    pub github_interval_secs: u64,

    /// Vercel refresh interval in seconds
    #[serde(default = "default_vercel_interval")]
    pub vercel_interval_secs: u64,

    /// User action refresh cooldown in seconds
    #[serde(default = "default_user_action_cooldown")]
    pub user_action_cooldown_secs: u64,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            linear_interval_secs: default_linear_interval(),
            github_interval_secs: default_github_interval(),
            vercel_interval_secs: default_vercel_interval(),
            user_action_cooldown_secs: default_user_action_cooldown(),
        }
    }
}

fn default_linear_interval() -> u64 {
    15 // 15 seconds
}

fn default_github_interval() -> u64 {
    30
}

fn default_vercel_interval() -> u64 {
    30
}

fn default_user_action_cooldown() -> u64 {
    10
}

// =============================================================================
// Cache Configuration
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable local cache for offline/fast startup
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Cache file path (relative to config dir, or absolute)
    #[serde(default = "default_cache_file")]
    pub file: String,

    /// Maximum age of cache before full refresh (in hours)
    #[serde(default = "default_cache_max_age_hours")]
    pub max_age_hours: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            file: default_cache_file(),
            max_age_hours: default_cache_max_age_hours(),
        }
    }
}

fn default_cache_file() -> String {
    "cache.json".to_string()
}

fn default_cache_max_age_hours() -> u64 {
    24
}

// =============================================================================
// Notification Configuration
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_true")]
    pub sound: bool,

    /// Notify on PR review requests
    #[serde(default = "default_true")]
    pub on_review_request: bool,

    /// Notify on PR approvals
    #[serde(default = "default_true")]
    pub on_approval: bool,

    /// Notify on deployment failures
    #[serde(default = "default_true")]
    pub on_deploy_failure: bool,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sound: true,
            on_review_request: true,
            on_approval: true,
            on_deploy_failure: true,
        }
    }
}

fn default_true() -> bool {
    true
}

// =============================================================================
// UI Configuration
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Color theme
    #[serde(default)]
    pub theme: String,

    /// Default sort mode
    #[serde(default = "default_sort_mode")]
    pub default_sort: String,

    /// Show sub-issues by default
    #[serde(default = "default_true")]
    pub show_sub_issues: bool,

    /// Show completed issues by default
    #[serde(default)]
    pub show_completed: bool,

    /// Show canceled issues by default
    #[serde(default)]
    pub show_canceled: bool,

    /// Show preview panel by default
    #[serde(default)]
    pub show_preview: bool,

    /// Column widths [status, priority, id, title, pr, agent, vercel, time]
    #[serde(default = "default_column_widths")]
    pub column_widths: [usize; 8],
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: String::new(),
            default_sort: default_sort_mode(),
            show_sub_issues: true,
            show_completed: false,
            show_canceled: false,
            show_preview: false,
            column_widths: default_column_widths(),
        }
    }
}

fn default_sort_mode() -> String {
    "priority".to_string()
}

fn default_column_widths() -> [usize; 8] {
    // Status, Priority, ID, Title, PR, Agent, Vercel, Time
    [1, 3, 10, 26, 12, 20, 3, 6]
}

// =============================================================================
// Path Utilities
// =============================================================================

pub fn config_dir() -> Result<PathBuf> {
    let dir = directories::ProjectDirs::from("", "", "panopticon")
        .context("Could not determine config directory")?
        .config_dir()
        .to_path_buf();
    Ok(dir)
}

pub fn default_config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

pub fn cache_path(config: &Config) -> Result<PathBuf> {
    let cache_file = &config.cache.file;
    if cache_file == "~" || cache_file.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            let suffix = cache_file.trim_start_matches("~/");
            return Ok(home.join(suffix));
        }
    }
    if Path::new(cache_file).is_absolute() {
        Ok(PathBuf::from(cache_file))
    } else {
        Ok(config_dir()?.join(cache_file))
    }
}

// =============================================================================
// Load/Save
// =============================================================================

pub fn load(path: Option<&Path>) -> Result<Config> {
    let path = match path {
        Some(p) => p.to_path_buf(),
        None => default_config_path()?,
    };

    if !path.exists() {
        anyhow::bail!(
            "Config file not found at {}. Run `panopticon --init` to create one.",
            path.display()
        );
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config from {}", path.display()))?;

    let config: Config = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config from {}", path.display()))?;

    Ok(config)
}

/// Generate example config with all options documented
pub fn generate_example_config() -> String {
    r#"# Panopticon Configuration
# =========================

# API Tokens (required)
[tokens]
linear = "lin_api_xxxxx"
github = "ghp_xxxxx"
vercel = "xxxxx"  # Optional

# Linear Settings
[linear]
filter = "assignee:me"    # Linear search filter
fetch_limit = 150         # Max issues per API call
incremental_sync = true   # Only fetch updated issues

# GitHub Settings
[github]
username = "your-username"
organizations = ["org1", "org2"]

# Vercel Settings
[vercel]
team_id = "team_xxxxx"
project_ids = ["prj_xxxxx"]

# Polling Intervals (seconds)
[polling]
linear_interval_secs = 15
github_interval_secs = 30
vercel_interval_secs = 30
user_action_cooldown_secs = 10

# Local Cache
[cache]
enabled = true
file = "cache.json"       # Relative to config dir
max_age_hours = 24        # Full refresh after this

# Notifications
[notifications]
enabled = true
sound = true
on_review_request = true
on_approval = true
on_deploy_failure = true

# UI Preferences
[ui]
theme = ""                # Future: light/dark/custom
default_sort = "priority" # priority, status, updated, agent, pr, vercel
show_sub_issues = true
show_completed = false
show_canceled = false
show_preview = false
column_widths = [1, 3, 10, 26, 12, 10, 3, 6]
"#
    .to_string()
}

// =============================================================================
// Init Wizard
// =============================================================================

pub async fn init_wizard() -> Result<()> {
    use std::io::{self, Write};

    println!("Panopticon Configuration Wizard");
    println!("================================\n");

    let config_path = default_config_path()?;
    if config_path.exists() {
        print!(
            "Config already exists at {}. Overwrite? [y/N] ",
            config_path.display()
        );
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    println!("Enter your API tokens:\n");

    print!("Linear API token (https://linear.app/settings/api): ");
    io::stdout().flush()?;
    let mut linear_token = String::new();
    io::stdin().read_line(&mut linear_token)?;

    print!("GitHub token (https://github.com/settings/tokens): ");
    io::stdout().flush()?;
    let mut github_token = String::new();
    io::stdin().read_line(&mut github_token)?;

    print!("Vercel token (optional, press Enter to skip): ");
    io::stdout().flush()?;
    let mut vercel_token = String::new();
    io::stdin().read_line(&mut vercel_token)?;

    let config = Config {
        tokens: Tokens {
            linear: linear_token.trim().to_string(),
            github: github_token.trim().to_string(),
            vercel: if vercel_token.trim().is_empty() {
                None
            } else {
                Some(vercel_token.trim().to_string())
            },
        },
        linear: LinearConfig::default(),
        github: GithubConfig::default(),
        vercel: VercelConfig::default(),
        polling: PollingConfig::default(),
        cache: CacheConfig::default(),
        notifications: NotificationConfig::default(),
        ui: UiConfig::default(),
    };

    // Create config directory
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Write config with restricted permissions
    let content = toml::to_string_pretty(&config)?;
    std::fs::write(&config_path, content)?;

    // Set file permissions to 0600 (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600))?;
    }

    println!("\nConfig saved to {}", config_path.display());
    println!("Run `panopticon` to start the dashboard.");

    Ok(())
}
