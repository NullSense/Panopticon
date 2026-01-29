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

Uses **Elm Architecture (TEA)** pattern for the event loop:
- `Message` enum defines all possible user actions
- `App::update(msg)` processes messages and mutates state
- `input::dispatch()` maps key events to messages
- Unidirectional data flow: Key → Message → Update → Render

```
src/
├── main.rs              # Entry point, CLI args, initialization
├── config/              # Config loading, init wizard
├── tui/                 # TUI components (TEA pattern)
│   ├── mod.rs           # TUI runner, event loop
│   ├── app.rs           # App state + update() method
│   ├── message.rs       # Message enum (all user actions)
│   ├── input.rs         # Key dispatch + chord state machine
│   ├── ui.rs            # Rendering (view layer)
│   └── search.rs        # Fuzzy search with nucleo
├── integrations/        # External service integrations
│   ├── linear.rs        # Linear API (GraphQL)
│   ├── github.rs        # GitHub API
│   ├── vercel.rs        # Vercel API
│   ├── claude/          # Claude Code session detection
│   │   ├── mod.rs       # Re-exports
│   │   ├── state.rs     # Session state types
│   │   ├── watcher.rs   # File system watcher
│   │   └── setup.rs     # Setup utilities
│   └── moltbot/         # Clawdbot integration
└── data/                # Data models
```

### Key Patterns
- **ModalState enum**: Single enum replaces multiple boolean flags for modal state
- **Non-blocking refresh**: Background data fetch with progress tracking via channels
- **InputState**: State machine for key chords (gg, d1-d9, c1-c9) with timeout

## Configuration
Config stored at `~/.config/panopticon/config.toml` with API tokens for:
- Linear
- GitHub  
- Vercel (optional)
