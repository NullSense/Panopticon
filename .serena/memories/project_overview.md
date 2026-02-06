# Panopticon - Project Overview

## Purpose
Terminal dashboard (TUI) for monitoring AI agent sessions linked to Linear, GitHub, and Vercel.
Injects hooks into Claude Code's `~/.claude/settings.json` to track session lifecycle and tool usage.

## Tech Stack
- **Language**: Rust (edition 2021)
- **TUI**: ratatui + crossterm
- **Async runtime**: tokio (full features)
- **HTTP**: reqwest with rustls-tls
- **CLI**: clap (derive)
- **Config**: toml + directories/dirs crates
- **Error handling**: anyhow
- **Logging**: tracing + tracing-subscriber
- **File watching**: notify v7
- **Serialization**: serde + serde_json
- **Testing**: pretty_assertions, tempfile

## Codebase Structure
```
src/
├── main.rs              # CLI entry point (clap-based)
├── lib.rs               # Library root, re-exports all modules
├── config/              # Config loading, init wizard
├── tui/                 # Terminal UI
│   ├── app.rs           # Main app state
│   ├── input.rs         # Input handling
│   ├── search.rs        # Fuzzy search
│   ├── message.rs       # Message types
│   ├── keybindings/     # Keybinding system
│   └── ui/              # UI rendering (layout, table, modals, menus, etc.)
├── agents/              # Agent orchestration
│   ├── merger.rs        # Merging agent data
│   └── unified_watcher.rs
├── integrations/        # External service integrations
│   ├── linear.rs        # Linear API
│   ├── github.rs        # GitHub API
│   ├── vercel.rs        # Vercel API
│   ├── cache.rs         # Caching layer
│   ├── enrichment_cache.rs
│   ├── agent_cache.rs
│   ├── claude/          # Claude Code integration
│   │   ├── setup.rs     # Hook injection into ~/.claude/settings.json
│   │   ├── state.rs     # Session state management
│   │   ├── watcher.rs   # File watcher for state changes
│   │   └── hook_input.rs # Hook stdin JSON parsing
│   └── openclaw/        # OpenClaw integration
├── data/                # Data models & sorting
└── util.rs              # Utilities
tests/                   # Integration tests (25+ test files)
config.toml              # Local config (tokens, gitignored)
```

## Config
- `config.toml` at project root (gitignored) holds API tokens for Linear, GitHub
- Settings injected into `~/.claude/settings.json`

## Entry Points
- `panopticon` (default): TUI dashboard
- `panopticon internal-hook --event <event>`: Hook handler called by Claude Code
- `panopticon --init`: Config initialization wizard
- `src/bin/debug_apis.rs`: Debug binary for API testing
