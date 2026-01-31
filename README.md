# Panopticon

Terminal dashboard for monitoring AI agent sessions (Claude Code, Clawdbot) linked to Linear issues, GitHub PRs, and Vercel deployments.

![Panopticon Dashboard](panopticon.png)

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
| `s` | Sort options |
| `f` | Filter options (cycle, priority, project, assignee) |
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
filter = "assignee:me"      # Only show issues assigned to you
fetch_limit = 150           # Max issues to fetch per request
incremental_sync = true     # Only fetch updated issues

[github]
username = "your-github-username"
organizations = ["your-org"] # optional

[vercel]
team_id = "team_..."        # optional
project_ids = ["prj_..."]   # optional

[polling]
linear_interval_secs = 15
github_interval_secs = 30
vercel_interval_secs = 30
user_action_cooldown_secs = 10  # Min time between user-triggered refreshes

[cache]
enabled = true
file = "cache.json"         # Relative to config dir or absolute path
max_age_hours = 24          # Show stale indicator after this

[notifications]
enabled = true
sound = true
on_review_request = true
on_approval = true
on_deploy_failure = true

[ui]
theme = ""
default_sort = "priority"
show_sub_issues = true      # Show child issues under parents
show_completed = false      # Hide completed issues by default
show_canceled = false       # Hide canceled/duplicate issues
show_preview = false
column_widths = [1, 3, 10, 26, 12, 10, 3, 6]
```

## Claude Code Integration

Panopticon can track active Claude Code sessions by integrating with Claude Code's hooks system. This lets you see which issues have agents actively working on them.

### Installation for Hooks

The `panopticon` binary must be in your PATH for hooks to work:

```bash
# Install to ~/.cargo/bin (recommended)
cargo install --path .

# Verify it's accessible
which panopticon
```

> **Note:** After making changes to panopticon, re-run `cargo install --path .` to update the installed binary.

### Configure Claude Code Hooks

Add the following to your `~/.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "panopticon internal-hook --event start"
          }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "panopticon internal-hook --event stop"
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "panopticon internal-hook --event active"
          }
        ]
      }
    ]
  }
}
```

### How It Works

- **SessionStart**: Registers a new Claude session with the working directory
- **UserPromptSubmit**: Updates session status to "active" (agent is working)
- **Stop**: Marks the session as ended

Panopticon matches sessions to Linear issues by looking for issue identifiers (e.g., `DRE-174`) in the working directory path or git branch name.

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
