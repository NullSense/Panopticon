use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub tokens: Tokens,
    #[serde(default)]
    pub linear: LinearConfig,
    #[serde(default)]
    pub polling: PollingConfig,
    #[serde(default)]
    pub notifications: NotificationConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tokens {
    pub linear: String,
    pub github: String,
    #[serde(default)]
    pub vercel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinearConfig {
    #[serde(default = "default_linear_filter")]
    pub filter: String,
}

fn default_linear_filter() -> String {
    "assignee:me".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingConfig {
    #[serde(default = "default_poll_interval")]
    pub github_interval_secs: u64,
    #[serde(default = "default_poll_interval")]
    pub vercel_interval_secs: u64,
}

fn default_poll_interval() -> u64 {
    30
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            github_interval_secs: default_poll_interval(),
            vercel_interval_secs: default_poll_interval(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub sound: bool,
}

fn default_true() -> bool {
    true
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sound: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiConfig {
    #[serde(default)]
    pub theme: String,
}

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

pub async fn init_wizard() -> Result<()> {
    use std::io::{self, Write};

    println!("Panopticon Configuration Wizard");
    println!("================================\n");

    let config_path = default_config_path()?;
    if config_path.exists() {
        print!("Config already exists at {}. Overwrite? [y/N] ", config_path.display());
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
        polling: PollingConfig::default(),
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
