use anyhow::Result;
use clap::{Parser, Subcommand};
use panopticon::{config, integrations, tui};

#[derive(Parser, Debug)]
#[command(name = "panopticon")]
#[command(about = "Terminal dashboard for monitoring AI agent sessions")]
#[command(version)]
struct Args {
    /// Run as background daemon
    #[arg(long)]
    daemon: bool,

    /// Initialize configuration
    #[arg(long)]
    init: bool,

    /// Path to config file
    #[arg(long, short)]
    config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Internal hook handler (called by Claude Code hooks)
    #[command(hide = true)]
    InternalHook {
        /// Event type: start, active, stop
        #[arg(long)]
        event: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Handle internal-hook command first (before logging setup for speed)
    if let Some(Command::InternalHook { event }) = &args.command {
        // Get session info from environment (Claude passes this)
        let session_id = std::env::var("CLAUDE_SESSION_ID")
            .or_else(|_| std::env::var("SESSION_ID"))
            .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());

        let cwd = std::env::var("PWD")
            .or_else(|_| std::env::current_dir().map(|p| p.to_string_lossy().to_string()))
            .unwrap_or_default();

        // Quick write to state file and exit
        integrations::claude::handle_hook(event, &session_id, &cwd)?;
        return Ok(());
    }

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("panopticon=info".parse()?),
        )
        .init();

    if args.init {
        config::init_wizard().await?;
        return Ok(());
    }

    let config = config::load(args.config.as_deref())?;

    // Initialize Claude integration (inject hooks on first run)
    if let Err(e) = integrations::claude::init() {
        tracing::warn!("Failed to initialize Claude hooks: {}", e);
    }

    if args.daemon {
        anyhow::bail!(
            "Daemon mode is not yet implemented. Run without --daemon flag to use the TUI."
        );
    }

    // Run TUI
    tui::run(config).await
}
