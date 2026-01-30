# Panopticon - Agent Monitoring TUI

Terminal-based dashboard for monitoring concurrent AI agent sessions linked to project management (Linear), version control (GitHub), and deployments (Vercel).

## Problem

When working with multiple AI coding agents (Claude Code CLI, Clawdbot) across different Linear issues, GitHub PRs, and Vercel deployments, it's hard to:
- Know which agents are running, waiting for input, or blocked
- See the status of related PRs and deployments at a glance
- Jump between different agent sessions quickly
- Search across all active work

## Solution

A k9s-style TUI that aggregates all this information in one view with:
- Real-time status updates
- Fuzzy search across Linear issues
- One-key teleport to any Claude session
- Clickable links to open Linear/GitHub/Vercel in browser

## Tech Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | Performance, single binary, low memory |
| TUI | Ratatui | Best Rust TUI library, active development |
| Async | Tokio | Standard async runtime |
| Search | nucleo | Fast fuzzy matching (same as Helix editor) |

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Panopticon                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │ Linear       │  │ GitHub       │  │ Vercel       │              │
│  │ Subscriptions│  │ Polling      │  │ Polling      │              │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘              │
│         │                 │                 │                       │
│         └─────────────────┼─────────────────┘                       │
│                           ▼                                         │
│                    ┌─────────────┐                                  │
│                    │  State      │                                  │
│                    │  Manager    │                                  │
│                    └──────┬──────┘                                  │
│                           │                                         │
│         ┌─────────────────┼─────────────────┐                       │
│         ▼                 ▼                 ▼                       │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │ Claude Code  │  │ Clawdbot    │  │ File        │              │
│  │ Hooks (IPC)  │  │ Gateway     │  │ Watchers    │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│                                                                     │
│                           ▼                                         │
│                    ┌─────────────┐                                  │
│                    │  TUI        │                                  │
│                    │  (Ratatui)  │                                  │
│                    └─────────────┘                                  │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### Linear (Real-time via GraphQL Subscriptions)
- Connect to Linear's WebSocket endpoint
- Subscribe to issue updates for assigned issues
- Get linked PR URLs from attachments

### GitHub (Polling ~30s)
- Query PR status, reviews, merge state
- Get deployment statuses (Vercel populates these)
- Rate limit: 5000 req/hour authenticated

### Vercel (Polling ~30s or via GitHub)
- Deployment URLs from GitHub commit statuses
- Or direct Vercel API for build logs

### Claude Code (Local Hooks)
- Register hooks in `~/.claude/settings.json`
- Hooks send events to Unix socket `/tmp/panopticon.sock`
- Events: session start, end, permission prompt, completion

### Clawdbot (Gateway API)
- Query Gateway at `http://127.0.0.1:18789`
- Watch `~/.clawdbot/` for state changes

## UI Layout

```
┌───────────────────────────────────────────────────────────────────────┐
│ 🔍 Search: ___________                             [5 active] 12:34:56│
├───────────────────────────────────────────────────────────────────────┤
│ ▼ IN PROGRESS (3)                                                     │
│   LIN-123 Fix auth bug    │ 🟢 PR#45 merged  │ 🔴 Claude │ ✅  │ 12:34│
│   LIN-456 Add dashboard   │ 🟡 PR#67 review  │ 🟢 Claude │ 🔄  │ 03:21│
│   LIN-789 Refactor API    │ 🔵 PR#89 draft   │ 🟢 Claude │ ⏳  │ 00:45│
│ ▼ IN REVIEW (1)                                                       │
│   LIN-234 Update tests    │ 🟢 PR#34 approved│ ⚪ Done   │ ✅  │(done)│
│ ▶ DONE (1) [collapsed]                                                │
└───────────────────────────────────────────────────────────────────────┘

Columns:
1. Linear ID + title (truncated)
2. GitHub PR status (draft/open/review/approved/merged)
3. Claude/Clawdbot status (running/waiting/done)
4. Vercel status (queued/building/ready/error)
5. Elapsed time
```

### Status Indicators

**Linear Issue Status** (from Linear's status field):
- Grouped by status: In Progress, In Review, Done, etc.

**GitHub PR Status**:
- 🔵 Draft
- 🟡 Open (review requested)
- 🟠 Changes requested
- 🟢 Approved
- 🟣 Merged
- ⚫ Closed

**Claude Session Status**:
- 🟢 Running (active output)
- 🟡 Idle (no output 30s+)
- 🔴 Waiting (permission prompt)
- ⚪ Done (session ended)

**Vercel Deployment**:
- ⏳ Queued
- 🔄 Building
- ✅ Ready
- ❌ Error

## Keybindings

### Navigation
| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `h` / `←` | Collapse section |
| `l` / `→` | Expand section |
| `gg` | Go to top |
| `G` | Go to bottom |
| `Ctrl+d` | Half page down |
| `Ctrl+u` | Half page up |

### Search
| Key | Action |
|-----|--------|
| `/` | Search active work (fzf mode) |
| `Ctrl+/` | Search all Linear issues |
| `Esc` | Exit search |
| `Enter` | Select result |

### Actions
| Key | Action |
|-----|--------|
| `Enter` | Open primary link (Linear issue) |
| `o` | Open submenu: Linear \| GitHub \| Vercel \| Claude |
| `t` | Teleport to Claude session (focus window) |
| `p` | Preview Claude output in TUI |
| `r` | Refresh all data |
| `?` | Help |
| `q` | Quit |

## Session Teleport

### Focus Existing Window (WSL + Alacritty)
```powershell
# PowerShell command to focus window by title
$windows = Get-Process | Where-Object {$_.MainWindowTitle -like "*claude*"}
[void][System.Reflection.Assembly]::LoadWithPartialName('Microsoft.VisualBasic')
[Microsoft.VisualBasic.Interaction]::AppActivate($windows.MainWindowTitle)
```

### Preview Mode
- Read-only view of recent Claude output
- Scroll through history
- Press `Esc` to return to main view

### reptyr Attachment (Future)
- For orphaned sessions: `reptyr <pid>`
- Attach process to TUI's embedded PTY

## Configuration

```toml
# ~/.config/panopticon/config.toml

[tokens]
linear = "lin_api_..."
github = "ghp_..."
vercel = "..."  # optional

[linear]
filter = "assignee:me"       # Only show issues assigned to you
fetch_limit = 150            # Max issues per request
incremental_sync = true      # Only fetch updated issues

[github]
username = "your-github-username"
organizations = ["your-org"] # optional

[vercel]
team_id = "team_..."         # optional
project_ids = ["prj_..."]    # optional

[polling]
linear_interval_secs = 15
github_interval_secs = 30
vercel_interval_secs = 30
user_action_cooldown_secs = 10

[cache]
enabled = true
file = "cache.json"
max_age_hours = 24           # Stale indicator threshold

[notifications]
enabled = true
sound = true
on_review_request = true
on_approval = true
on_deploy_failure = true

[ui]
theme = ""
default_sort = "priority"
show_sub_issues = true       # Show child issues under parents
show_completed = false       # Hide completed issues
show_canceled = false        # Hide canceled/duplicate issues
show_preview = false
column_widths = [1, 3, 10, 26, 12, 10, 3, 6]
```

## Files & Directories

```
~/.config/panopticon/
├── config.toml          # Main configuration
└── sessions.json        # Cached session state

~/.cache/panopticon/
└── workstreams.json     # Cached workstream data (for quick startup)

~/.claude/settings.json  # Claude Code hooks (modified)

/tmp/panopticon.sock     # Unix socket for IPC
```

## MVP Scope (v0.1)

### Must Have
- [ ] Dashboard with Linear issues grouped by status
- [ ] GitHub PR status indicators
- [ ] Vercel deployment status
- [ ] Claude Code session detection (file-based)
- [ ] fzf-style search
- [ ] vim-style navigation
- [ ] Click to open links (Linear, GitHub, Vercel)
- [ ] Focus Claude session window

### Nice to Have (MVP)
- [ ] Notifications on attention-needed
- [ ] Read-only Claude output preview
- [ ] Elapsed time tracking

### V2 Features
- [ ] Bootstrap new work (create branch + start Claude)
- [ ] Full webhooks (instant updates)
- [ ] Clawdbot deep integration
- [ ] Custom themes
- [ ] Multi-workspace support

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Linear API down | Show cached data, indicate staleness |
| GitHub rate limited | Back off, show warning |
| Token invalid | Prompt to re-authenticate |
| No issues found | Show empty state with hint |
| Claude session not found | Remove from list, don't crash |

## Security

- Tokens stored in `~/.config/panopticon/config.toml` with `0600` permissions
- No tokens logged or displayed
- Unix socket restricted to current user
- No network exposure (all outbound connections)
