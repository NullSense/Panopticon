# Panopticon

Terminal dashboard for monitoring AI agent sessions (Claude Code, Clawdbot) linked to Linear issues, GitHub PRs, and Vercel deployments.

## Installation

### Prerequisites

- Rust toolchain (install via [rustup](https://rustup.rs/))
- API tokens for Linear, GitHub, and optionally Vercel

### Build from source

```bash
# Clone and build
cd panopticon
cargo build --release

# The binary is at ./target/release/panopticon
# Optionally, install it to your PATH:
cargo install --path .
```

## Setup

### 1. Initialize configuration

```bash
# If installed to PATH:
panopticon --init

# Or run directly from the project:
./target/release/panopticon --init
```

This will prompt you for:
- **Linear API token** - Get it from https://linear.app/settings/api
- **GitHub token** - Create at https://github.com/settings/tokens (needs `repo` scope)
- **Vercel token** (optional) - Get from https://vercel.com/account/tokens

Config is saved to `~/.config/panopticon/config.toml`

### 2. Run the dashboard

```bash
# If installed to PATH:
panopticon

# Or run directly:
./target/release/panopticon

# Or during development:
cargo run
```

## Keybindings

| Key | Action |
|-----|--------|
| `j` / `k` | Move down / up |
| `gg` | Go to top |
| `G` | Go to bottom |
| `Ctrl+d` / `Ctrl+u` | Page down / up |
| `/` | Search active work |
| `Ctrl+/` | Search all Linear issues |
| `Enter` | Open Linear issue in browser |
| `o` | Open link menu (Linear/GitHub/Vercel/Claude) |
| `t` | Teleport to Claude session window |
| `p` | Preview Claude output |
| `r` | Refresh data |
| `?` | Show help |
| `q` | Quit |

## Configuration

Edit `~/.config/panopticon/config.toml`:

```toml
[tokens]
linear = "lin_api_..."
github = "ghp_..."
vercel = "..."  # optional

[linear]
filter = "assignee:me"  # Only show issues assigned to you

[polling]
github_interval_secs = 30
vercel_interval_secs = 30

[notifications]
enabled = true
sound = true
```

## Development

```bash
# Run with debug output
RUST_LOG=panopticon=debug cargo run

# Check for errors without building
cargo check

# Run tests
cargo test
```

## Architecture

See [SPEC.md](./SPEC.md) for full technical specification.

## License

MIT
