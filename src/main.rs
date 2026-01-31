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
        // Parse rich input from stdin (Claude Code sends JSON)
        let hook_input = integrations::claude::hook_input::HookInput::from_stdin();

        // Get session info from environment or stdin
        let session_id = hook_input
            .as_ref()
            .and_then(|i| i.session_id.clone())
            .or_else(|| std::env::var("CLAUDE_SESSION_ID").ok())
            .or_else(|| std::env::var("SESSION_ID").ok())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let cwd = hook_input
            .as_ref()
            .and_then(|i| i.cwd.clone())
            .or_else(|| std::env::var("PWD").ok())
            .or_else(|| std::env::current_dir().map(|p| p.to_string_lossy().to_string()).ok())
            .unwrap_or_default();

        // Quick write to state file and exit
        integrations::claude::handle_hook(event, &session_id, &cwd, hook_input.as_ref())?;
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
