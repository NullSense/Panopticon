mod config;
mod data;
mod integrations;
mod tui;

use anyhow::Result;
use clap::Parser;

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
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

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

    if args.daemon {
        tracing::info!("Starting panopticon daemon...");
        // TODO: Run as daemon
        todo!("Daemon mode not yet implemented");
    }

    // Run TUI
    tui::run(config).await
}
