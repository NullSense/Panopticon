# Panopticon - Project Overview

## Purpose
Terminal dashboard (TUI) for monitoring AI agent sessions (Claude Code, Clawdbot) linked to:
- Linear issues
- GitHub PRs
- Vercel deployments

## Problem Solved
Provides unified visibility into multiple concurrent AI coding agents, showing which are running, waiting for input, or blocked, along with their related PRs and deployments.

## Tech Stack
| Component | Choice |
|-----------|--------|
| Language | Rust (2021 edition) |
| TUI | Ratatui + Crossterm |
| Async | Tokio |
| HTTP | Reqwest |
| Search | Nucleo (fuzzy matching) |
| Error handling | anyhow + thiserror |
| CLI | Clap (derive) |
| Config | TOML |
| Logging | tracing + tracing-subscriber |

## Architecture
```
src/
├── main.rs              # Entry point, CLI args, initialization
├── config/              # Config loading, init wizard
├── tui/                 # TUI components
│   ├── app.rs           # Main App state and logic
│   ├── ui.rs            # Rendering
│   ├── search.rs        # Fuzzy search
│   └── mod.rs           # TUI runner
├── integrations/        # External service integrations
│   ├── linear.rs        # Linear API (GraphQL)
│   ├── github.rs        # GitHub API
│   ├── vercel.rs        # Vercel API
│   ├── claude/          # Claude Code session detection
│   └── moltbot/         # Clawdbot integration
└── data/                # Data models
```

## Configuration
Config stored at `~/.config/panopticon/config.toml` with API tokens for:
- Linear
- GitHub  
- Vercel (optional)
