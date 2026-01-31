# Panopticon Agent Orchestration Spec

> **Status**: Draft  
> **Issue**: DRE-380  
> **Author**: Beans 🦝  
> **Date**: 2026-01-31

## Overview

Panopticon becomes the unified entry point for managing AI coding agents. Users can:

1. **See** all running agents (Claude Code, OpenClaw sessions)
2. **Spawn** new agents from Linear issues
3. **Teleport** into any agent session
4. **Track** agent status and completion

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Panopticon TUI                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │ Linear View │  │ Agent View  │  │  Spawn UI   │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Session Registry                              │
│              ~/.local/share/panopticon/sessions.json             │
└────────────────────────────┬────────────────────────────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        ▼                    ▼                    ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│ tmux Sessions │    │ Claude Hooks │    │ OpenClaw API │
│  (spawned)    │    │  (updates)   │    │  (read-only) │
└──────────────┘    └──────────────┘    └──────────────┘
```

---

## 1. Session Registry

### Location
```
~/.local/share/panopticon/sessions.json
```

### Schema

```rust
/// Root registry structure
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionRegistry {
    /// Schema version for migrations
    pub version: u32,  // Start at 1
    
    /// All tracked sessions
    pub sessions: HashMap<String, TrackedSession>,
}

/// A tracked agent session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedSession {
    /// Unique session identifier (tmux session name)
    pub id: String,
    
    /// Agent type
    pub agent_type: AgentType,
    
    /// Current status
    pub status: AgentStatus,
    
    /// tmux session name (for teleporting)
    pub tmux_session: Option<String>,
    
    /// tmux socket path (if using custom socket)
    pub tmux_socket: Option<String>,
    
    /// Working directory
    pub working_directory: String,
    
    /// Git branch (primary identity for linking to issues)
    pub git_branch: Option<String>,
    
    /// Linear issue ID (fallback identity)
    pub linear_issue_id: Option<String>,
    
    /// Linear issue identifier (e.g., "DRE-380")
    pub linear_issue_identifier: Option<String>,
    
    /// Task/prompt given to agent
    pub task: Option<String>,
    
    /// When session was created
    pub created_at: DateTime<Utc>,
    
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
    
    /// Last known output snippet (for status display)
    pub last_output: Option<String>,
    
    /// Source of this session
    pub source: SessionSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionSource {
    /// Spawned by Panopticon
    Panopticon,
    /// Detected via Claude Code hooks
    ClaudeHook,
    /// Detected via OpenClaw API
    OpenClaw,
    /// Manually registered
    Manual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentType {
    ClaudeCode,
    Codex,
    OpenClaw,  // Renamed from Clawdbot
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentStatus {
    /// Agent is actively working
    Running,
    /// Agent is at prompt, waiting for input
    Idle,
    /// Agent asked a question, needs user response
    WaitingForInput,
    /// Agent completed task
    Done,
    /// Agent encountered an error
    Error,
    /// Session exists but status unknown
    Unknown,
}
```

### Example JSON

```json
{
  "version": 1,
  "sessions": {
    "claude-dre-380": {
      "id": "claude-dre-380",
      "agent_type": "ClaudeCode",
      "status": "Running",
      "tmux_session": "claude-dre-380",
      "tmux_socket": null,
      "working_directory": "/home/user/Programming/panopticon",
      "git_branch": "feat/dre-380-unified-orchestration",
      "linear_issue_id": "af11902e-0065-413b-a4fc-f147d6cac130",
      "linear_issue_identifier": "DRE-380",
      "task": "Implement session registry for agent orchestration",
      "created_at": "2026-01-31T10:30:00Z",
      "last_activity": "2026-01-31T10:35:00Z",
      "last_output": "Reading src/integrations/mod.rs...",
      "source": "Panopticon"
    }
  }
}
```

### File Locking

Use `fs2` crate for cross-platform file locking (already in dependencies):

```rust
use fs2::FileExt;

fn read_registry() -> Result<SessionRegistry> {
    let path = registry_path()?;
    let file = File::open(&path)?;
    file.lock_shared()?;  // Shared lock for reading
    let registry: SessionRegistry = serde_json::from_reader(&file)?;
    file.unlock()?;
    Ok(registry)
}

fn write_registry(registry: &SessionRegistry) -> Result<()> {
    let path = registry_path()?;
    let file = File::create(&path)?;
    file.lock_exclusive()?;  // Exclusive lock for writing
    serde_json::to_writer_pretty(&file, registry)?;
    file.unlock()?;
    Ok(())
}
```

---

## 2. Spawning Agents

### User Flow

1. User presses `S` on a Linear issue
2. Spawn modal appears with options:
   - Agent type: Claude Code (default), Codex
   - Working directory: auto-detected from git branch or manual
   - Task: auto-generated from issue or custom
3. User confirms with Enter
4. Panopticon:
   - Creates tmux session
   - Writes to registry
   - Launches agent with task
   - Returns to main view

### Spawn Modal UI

```
┌─ Spawn Agent ─────────────────────────────────────────┐
│                                                       │
│  Issue: DRE-380 - Unified Agent Orchestration        │
│                                                       │
│  Agent:     [Claude Code ▾]                          │
│  Directory: ~/Programming/panopticon                  │
│  Branch:    feat/dre-380-unified-orchestration       │
│                                                       │
│  Task:                                                │
│  ┌───────────────────────────────────────────────┐   │
│  │ Implement the session registry as specified   │   │
│  │ in SPEC-AGENT-ORCHESTRATION.md. Start with   │   │
│  │ the data structures and file I/O.            │   │
│  └───────────────────────────────────────────────┘   │
│                                                       │
│  [Enter] Spawn   [Esc] Cancel   [Tab] Next field     │
└───────────────────────────────────────────────────────┘
```

### Session Naming Convention

```
claude-{issue_identifier}
codex-{issue_identifier}

Examples:
  claude-dre-380
  codex-dre-380
  claude-dre-380-2  (if first exists)
```

### tmux Session Creation

```rust
pub struct SpawnConfig {
    pub agent_type: AgentType,
    pub working_directory: PathBuf,
    pub git_branch: Option<String>,
    pub linear_issue: Option<LinearIssueRef>,
    pub task: String,
}

pub struct LinearIssueRef {
    pub id: String,
    pub identifier: String,
    pub title: String,
}

/// Spawn a new agent session
pub fn spawn_agent(config: SpawnConfig) -> Result<TrackedSession> {
    // 1. Generate session name
    let session_name = generate_session_name(&config)?;
    
    // 2. Verify session doesn't exist
    if tmux_session_exists(&session_name)? {
        anyhow::bail!("Session '{}' already exists", session_name);
    }
    
    // 3. Create tmux session
    create_tmux_session(&session_name, &config.working_directory)?;
    
    // 4. Build agent command
    let command = build_agent_command(&config)?;
    
    // 5. Send command to tmux
    tmux_send_keys(&session_name, &command)?;
    
    // 6. Create tracked session
    let session = TrackedSession {
        id: session_name.clone(),
        agent_type: config.agent_type,
        status: AgentStatus::Running,
        tmux_session: Some(session_name),
        tmux_socket: None,  // Use default socket
        working_directory: config.working_directory.to_string_lossy().to_string(),
        git_branch: config.git_branch,
        linear_issue_id: config.linear_issue.as_ref().map(|i| i.id.clone()),
        linear_issue_identifier: config.linear_issue.as_ref().map(|i| i.identifier.clone()),
        task: Some(config.task),
        created_at: Utc::now(),
        last_activity: Utc::now(),
        last_output: None,
        source: SessionSource::Panopticon,
    };
    
    // 7. Write to registry
    add_session_to_registry(&session)?;
    
    Ok(session)
}

fn generate_session_name(config: &SpawnConfig) -> Result<String> {
    let prefix = match config.agent_type {
        AgentType::ClaudeCode => "claude",
        AgentType::Codex => "codex",
        _ => "agent",
    };
    
    let base = match &config.linear_issue {
        Some(issue) => format!("{}-{}", prefix, issue.identifier.to_lowercase()),
        None => format!("{}-{}", prefix, Utc::now().timestamp()),
    };
    
    // Check for conflicts and add suffix if needed
    let mut name = base.clone();
    let mut suffix = 2;
    while tmux_session_exists(&name)? {
        name = format!("{}-{}", base, suffix);
        suffix += 1;
    }
    
    Ok(name)
}

fn build_agent_command(config: &SpawnConfig) -> Result<String> {
    let task_escaped = config.task.replace("'", "'\\''");
    
    match config.agent_type {
        AgentType::ClaudeCode => {
            // Use --dangerously-skip-permissions for autonomous operation
            Ok(format!("claude --dangerously-skip-permissions '{}'", task_escaped))
        }
        AgentType::Codex => {
            Ok(format!("codex --full-auto exec '{}'", task_escaped))
        }
        _ => anyhow::bail!("Unsupported agent type for spawning"),
    }
}
```

### tmux Commands

```rust
/// Check if a tmux session exists
fn tmux_session_exists(session: &str) -> Result<bool> {
    let output = Command::new("tmux")
        .args(["has-session", "-t", session])
        .output()?;
    Ok(output.status.success())
}

/// Create a new detached tmux session
fn create_tmux_session(session: &str, working_dir: &Path) -> Result<()> {
    let status = Command::new("tmux")
        .args([
            "new-session",
            "-d",                           // Detached
            "-s", session,                  // Session name
            "-c", &working_dir.to_string_lossy(),  // Working directory
            "-x", "200",                    // Width
            "-y", "50",                     // Height
        ])
        .status()?;
    
    if !status.success() {
        anyhow::bail!("Failed to create tmux session");
    }
    Ok(())
}

/// Send keys to a tmux session
fn tmux_send_keys(session: &str, keys: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["send-keys", "-t", session, keys, "Enter"])
        .status()?;
    
    if !status.success() {
        anyhow::bail!("Failed to send keys to tmux session");
    }
    Ok(())
}

/// Capture pane content from a tmux session
fn tmux_capture_pane(session: &str, lines: i32) -> Result<String> {
    let output = Command::new("tmux")
        .args([
            "capture-pane",
            "-t", session,
            "-p",                           // Print to stdout
            "-S", &format!("-{}", lines),   // Start N lines back
        ])
        .output()?;
    
    if !output.status.success() {
        anyhow::bail!("Failed to capture tmux pane");
    }
    
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// List all tmux sessions
fn tmux_list_sessions() -> Result<Vec<String>> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()?;
    
    if !output.status.success() {
        // No sessions is not an error
        return Ok(vec![]);
    }
    
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect())
}
```

---

## 3. Teleporting to Sessions

### Platform Detection

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,   // WSL
    MacOS,
    Linux,
}

pub fn detect_platform() -> Platform {
    if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::MacOS
    } else {
        // Detect WSL
        if std::fs::read_to_string("/proc/version")
            .map(|v| v.contains("microsoft") || v.contains("WSL"))
            .unwrap_or(false)
        {
            Platform::Windows  // WSL counts as Windows for terminal spawning
        } else {
            Platform::Linux
        }
    }
}
```

### Terminal Emulator Selection

Priority order:
1. **`$TERMINAL`** — user's explicitly configured preference
2. **Platform default** — standard system mechanism
3. **Detection chain** — probe for known terminals

```rust
#[derive(Debug, Clone)]
pub struct TerminalConfig {
    /// Terminal command
    pub command: String,
    /// Arguments template (use {cmd} for the command to run)
    pub args: Vec<String>,
    /// Fallback terminal if primary not found
    pub fallback: Option<Box<TerminalConfig>>,
}

impl TerminalConfig {
    /// Resolve terminal for current platform with priority:
    /// 1. $TERMINAL env var
    /// 2. Platform default (x-terminal-emulator, open, wt)
    /// 3. Detection chain
    pub fn resolve_for_platform(platform: Platform) -> Result<Self> {
        // 1. Check $TERMINAL first
        if let Ok(terminal) = std::env::var("TERMINAL") {
            if which::which(&terminal).is_ok() {
                return Ok(Self {
                    command: terminal,
                    args: vec!["-e".to_string(), "{cmd}".to_string()],
                    fallback: None,
                });
            }
        }
        
        // 2. Try platform default
        if let Some(default) = Self::platform_default(platform) {
            if default.is_available() {
                return Ok(default);
            }
        }
        
        // 3. Fall back to detection chain
        let chain = Self::detection_chain(platform);
        chain.resolve()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No terminal emulator found"))
    }
    
    /// Platform-specific default terminal mechanism
    fn platform_default(platform: Platform) -> Option<Self> {
        match platform {
            Platform::Linux => {
                // x-terminal-emulator is the Debian/Ubuntu standard
                if which::which("x-terminal-emulator").is_ok() {
                    return Some(Self {
                        command: "x-terminal-emulator".to_string(),
                        args: vec!["-e".to_string(), "{cmd}".to_string()],
                        fallback: None,
                    });
                }
                None
            }
            Platform::MacOS => {
                // open -a Terminal.app always works on macOS
                Some(Self {
                    command: "open".to_string(),
                    args: vec![
                        "-a".to_string(),
                        "Terminal".to_string(),
                        // Create temp script to run command
                        // (open -a Terminal doesn't support direct command execution)
                    ],
                    fallback: None,
                })
            }
            Platform::Windows => {
                // Windows Terminal is the modern default
                if which::which("wt.exe").is_ok() {
                    return Some(Self {
                        command: "wt.exe".to_string(),
                        args: vec!["wsl".to_string(), "{cmd}".to_string()],
                        fallback: None,
                    });
                }
                None
            }
        }
    }
    
    /// Detection chain as last resort
    fn detection_chain(platform: Platform) -> Self {
        match platform {
            Platform::Windows => Self::windows_chain(),
            Platform::MacOS => Self::macos_chain(),
            Platform::Linux => Self::linux_chain(),
        }
    }
    
    /// Windows/WSL: Alacritty → Windows Terminal → cmd.exe
    fn windows_chain() -> Self {
        Self {
            command: "alacritty.exe".to_string(),
            args: vec!["-e".to_string(), "wsl".to_string(), "{cmd}".to_string()],
            fallback: Some(Box::new(Self {
                command: "wt.exe".to_string(),
                args: vec!["wsl".to_string(), "{cmd}".to_string()],
                fallback: Some(Box::new(Self {
                    command: "cmd.exe".to_string(),
                    args: vec!["/c".to_string(), "wsl".to_string(), "{cmd}".to_string()],
                    fallback: None,
                })),
            })),
        }
    }
    
    /// macOS: Alacritty → iTerm2 → Terminal.app
    fn macos_chain() -> Self {
        Self {
            command: "alacritty".to_string(),
            args: vec!["-e".to_string(), "{cmd}".to_string()],
            fallback: Some(Box::new(Self {
                // iTerm2 via osascript
                command: "osascript".to_string(),
                args: vec![
                    "-e".to_string(),
                    "tell application \"iTerm2\" to create window with default profile command \"{cmd}\"".to_string(),
                ],
                fallback: Some(Box::new(Self {
                    // Terminal.app via osascript
                    command: "osascript".to_string(),
                    args: vec![
                        "-e".to_string(),
                        "tell application \"Terminal\" to do script \"{cmd}\"".to_string(),
                    ],
                    fallback: None,
                })),
            })),
        }
    }
    
    /// Linux: Alacritty → Kitty → GNOME Terminal → Konsole → xterm
    fn linux_chain() -> Self {
        Self {
            command: "alacritty".to_string(),
            args: vec!["-e".to_string(), "{cmd}".to_string()],
            fallback: Some(Box::new(Self {
                command: "kitty".to_string(),
                args: vec!["{cmd}".to_string()],
                fallback: Some(Box::new(Self {
                    command: "gnome-terminal".to_string(),
                    args: vec!["--".to_string(), "{cmd}".to_string()],
                    fallback: Some(Box::new(Self {
                        command: "konsole".to_string(),
                        args: vec!["-e".to_string(), "{cmd}".to_string()],
                        fallback: Some(Box::new(Self {
                            command: "xterm".to_string(),
                            args: vec!["-e".to_string(), "{cmd}".to_string()],
                            fallback: None,
                        })),
                    })),
                })),
            })),
        }
    }
    
    /// Check if this terminal is available
    pub fn is_available(&self) -> bool {
        which::which(&self.command).is_ok()
    }
    
    /// Get the first available terminal in the chain
    pub fn resolve(&self) -> Option<&TerminalConfig> {
        if self.is_available() {
            Some(self)
        } else if let Some(ref fallback) = self.fallback {
            fallback.resolve()
        } else {
            None
        }
    }
}
```

### Teleport Implementation

```rust
/// Teleport to a session (open in new terminal window)
pub fn teleport_to_session(session: &TrackedSession) -> Result<()> {
    let tmux_session = session.tmux_session.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Session has no tmux session"))?;
    
    // Build the tmux attach command
    let attach_cmd = match &session.tmux_socket {
        Some(socket) => format!("tmux -S {} attach -t {}", socket, tmux_session),
        None => format!("tmux attach -t {}", tmux_session),
    };
    
    // Get terminal config for platform
    let platform = detect_platform();
    let terminal_config = TerminalConfig::for_platform(platform);
    
    let terminal = terminal_config.resolve()
        .ok_or_else(|| anyhow::anyhow!("No terminal emulator found"))?;
    
    // Build arguments with command substitution
    let args: Vec<String> = terminal.args.iter()
        .map(|arg| arg.replace("{cmd}", &attach_cmd))
        .collect();
    
    // Spawn terminal process
    Command::new(&terminal.command)
        .args(&args)
        .spawn()
        .context("Failed to spawn terminal")?;
    
    Ok(())
}
```

### Alternative: Focus Existing Window (WSL/Windows)

For sessions that are already visible in a terminal window, try to focus that window first:

```rust
/// Try to focus an existing window, fall back to teleport
pub async fn focus_or_teleport(session: &TrackedSession) -> Result<()> {
    let platform = detect_platform();
    
    // On Windows/WSL, try to focus existing window first
    if platform == Platform::Windows {
        if let Some(ref dir) = session.working_directory {
            // Try to find window by directory name in title
            if focus_window_by_title(dir).await.is_ok() {
                return Ok(());
            }
        }
    }
    
    // Fall back to teleport (new terminal window)
    teleport_to_session(session)
}

/// Focus a window by title match (Windows/WSL only)
async fn focus_window_by_title(search: &str) -> Result<()> {
    // Extract last path component for matching
    let search_term = Path::new(search)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(search);
    
    let script = format!(
        r#"
        Add-Type @"
        using System;
        using System.Runtime.InteropServices;
        public class Win32 {{
            [DllImport("user32.dll")]
            public static extern bool SetForegroundWindow(IntPtr hWnd);
        }}
"@
        $procs = Get-Process | Where-Object {{ $_.MainWindowTitle -like "*{}*" }}
        if ($procs) {{
            [Win32]::SetForegroundWindow($procs[0].MainWindowHandle)
            exit 0
        }}
        exit 1
        "#,
        search_term
    );

    let output = tokio::process::Command::new("powershell.exe")
        .args(["-Command", &script])
        .output()
        .await?;

    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!("Window not found")
    }
}
```

---

## 4. Status Tracking

### Polling Strategy

```rust
pub struct StatusPoller {
    /// How often to poll tmux for status (seconds)
    poll_interval: Duration,
    
    /// How many lines of pane history to capture
    capture_lines: i32,
    
    /// Patterns indicating agent is working
    working_patterns: Vec<Regex>,
    
    /// Patterns indicating agent is idle/waiting
    idle_patterns: Vec<Regex>,
    
    /// Patterns indicating agent is done
    done_patterns: Vec<Regex>,
    
    /// Patterns indicating error
    error_patterns: Vec<Regex>,
}

impl Default for StatusPoller {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            capture_lines: 30,
            working_patterns: vec![
                Regex::new(r"(?i)(thinking|working|reading|writing|searching)").unwrap(),
                Regex::new(r"[✻✶✽⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]").unwrap(),  // Spinners
            ],
            idle_patterns: vec![
                Regex::new(r"^❯\s*$").unwrap(),           // Claude Code prompt
                Regex::new(r"^\$\s*$").unwrap(),          // Shell prompt
                Regex::new(r"(?i)INSERT|NORMAL").unwrap(), // Vim mode indicators
            ],
            done_patterns: vec![
                Regex::new(r"(?i)task complete|finished|done").unwrap(),
            ],
            error_patterns: vec![
                Regex::new(r"(?i)error|failed|exception|panic").unwrap(),
            ],
        }
    }
}

impl StatusPoller {
    /// Detect status from pane content
    pub fn detect_status(&self, content: &str) -> AgentStatus {
        // Check last few lines for current state
        let recent_lines: Vec<&str> = content.lines().rev().take(5).collect();
        let recent = recent_lines.join("\n");
        
        // Check patterns in priority order
        for pattern in &self.error_patterns {
            if pattern.is_match(&recent) {
                return AgentStatus::Error;
            }
        }
        
        for pattern in &self.done_patterns {
            if pattern.is_match(&recent) {
                return AgentStatus::Done;
            }
        }
        
        for pattern in &self.working_patterns {
            if pattern.is_match(&recent) {
                return AgentStatus::Running;
            }
        }
        
        for pattern in &self.idle_patterns {
            if pattern.is_match(&recent) {
                return AgentStatus::Idle;
            }
        }
        
        AgentStatus::Unknown
    }
    
    /// Extract last meaningful output line
    pub fn extract_last_output(&self, content: &str) -> Option<String> {
        content.lines()
            .rev()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() 
                    && !trimmed.starts_with('❯')
                    && !trimmed.starts_with('$')
                    && trimmed.len() > 3
            })
            .next()
            .map(|s| {
                // Truncate to reasonable length
                if s.len() > 100 {
                    format!("{}...", &s[..97])
                } else {
                    s.to_string()
                }
            })
    }
}
```

### Background Polling Loop

```rust
/// Poll all tracked sessions and update registry
pub async fn poll_sessions(registry: &mut SessionRegistry) -> Result<()> {
    let poller = StatusPoller::default();
    let active_tmux_sessions = tmux_list_sessions()?;
    
    for (id, session) in registry.sessions.iter_mut() {
        // Skip non-tmux sessions
        let tmux_session = match &session.tmux_session {
            Some(s) => s.clone(),
            None => continue,
        };
        
        // Check if session still exists
        if !active_tmux_sessions.contains(&tmux_session) {
            session.status = AgentStatus::Done;
            session.last_activity = Utc::now();
            continue;
        }
        
        // Capture pane and detect status
        match tmux_capture_pane(&tmux_session, poller.capture_lines) {
            Ok(content) => {
                let new_status = poller.detect_status(&content);
                let new_output = poller.extract_last_output(&content);
                
                // Only update if changed
                if session.status != new_status {
                    session.status = new_status;
                    session.last_activity = Utc::now();
                }
                
                if new_output.is_some() && new_output != session.last_output {
                    session.last_output = new_output;
                    session.last_activity = Utc::now();
                }
            }
            Err(e) => {
                tracing::warn!("Failed to capture pane for {}: {}", tmux_session, e);
            }
        }
    }
    
    Ok(())
}
```

### Merging External Sources

```rust
/// Merge sessions from all sources into registry
pub async fn sync_all_sessions(registry: &mut SessionRegistry) -> Result<()> {
    // 1. Poll existing tmux sessions
    poll_sessions(registry).await?;
    
    // 2. Load Claude Code hook sessions
    if let Ok(claude_sessions) = load_claude_hook_sessions().await {
        for claude_session in claude_sessions {
            // Only add if not already tracked
            if !registry.sessions.contains_key(&claude_session.id) {
                registry.sessions.insert(claude_session.id.clone(), claude_session);
            }
        }
    }
    
    // 3. Load OpenClaw sessions (read-only, don't persist)
    // These are displayed but not written to registry
    
    Ok(())
}

/// Load sessions from Claude Code hooks
async fn load_claude_hook_sessions() -> Result<Vec<TrackedSession>> {
    let state = crate::integrations::claude::state::read_state()?;
    
    let sessions = state.sessions.into_iter()
        .map(|(id, s)| TrackedSession {
            id: id.clone(),
            agent_type: AgentType::ClaudeCode,
            status: match s.status.as_str() {
                "running" | "active" => AgentStatus::Running,
                "idle" => AgentStatus::Idle,
                "done" | "stop" => AgentStatus::Done,
                _ => AgentStatus::Unknown,
            },
            tmux_session: None,  // Claude hooks don't track tmux
            tmux_socket: None,
            working_directory: s.path,
            git_branch: s.git_branch,
            linear_issue_id: None,
            linear_issue_identifier: None,
            task: None,
            created_at: Utc.timestamp_opt(s.last_active, 0).single().unwrap_or_else(Utc::now),
            last_activity: Utc.timestamp_opt(s.last_active, 0).single().unwrap_or_else(Utc::now),
            last_output: None,
            source: SessionSource::ClaudeHook,
        })
        .collect();
    
    Ok(sessions)
}
```

---

## 5. UI Integration

### New Messages

```rust
// Add to src/tui/message.rs

pub enum Message {
    // ... existing messages ...
    
    // ─────────────────────────────────────────────────────────────────────────
    // Agent spawning
    // ─────────────────────────────────────────────────────────────────────────
    /// Open spawn modal for current issue
    OpenSpawnModal,
    /// Close spawn modal
    CloseSpawnModal,
    /// Change agent type in spawn modal
    SetSpawnAgentType(AgentType),
    /// Change working directory in spawn modal
    SetSpawnDirectory(PathBuf),
    /// Update task text in spawn modal
    SetSpawnTask(String),
    /// Confirm and execute spawn
    ConfirmSpawn,
    
    // ─────────────────────────────────────────────────────────────────────────
    // Agent management
    // ─────────────────────────────────────────────────────────────────────────
    /// Kill an agent session
    KillSession(String),
    /// Refresh agent sessions
    RefreshAgents,
}
```

### New Keybindings

| Key | Context | Action |
|-----|---------|--------|
| `S` | Main view (on issue) | Open spawn modal |
| `t` | Main view / Link menu | Teleport to session |
| `K` | Main view (on agent row) | Kill session |
| `Enter` | Spawn modal | Confirm spawn |
| `Esc` | Spawn modal | Cancel |
| `Tab` | Spawn modal | Next field |
| `Shift+Tab` | Spawn modal | Previous field |

### Modal State

```rust
// Add to modal state enum

pub enum ModalState {
    // ... existing variants ...
    
    /// Spawn agent modal
    Spawn(SpawnModalState),
}

pub struct SpawnModalState {
    /// Selected agent type
    pub agent_type: AgentType,
    /// Working directory
    pub directory: String,
    /// Task/prompt
    pub task: String,
    /// Which field is focused
    pub focused_field: SpawnField,
    /// The issue being spawned for
    pub issue: LinearIssue,
}

pub enum SpawnField {
    AgentType,
    Directory,
    Task,
}
```

### Agent Status Display

In the main table, show agent status in a dedicated column:

```
│ ID       │ Title                        │ Status │ Agent    │
├──────────┼──────────────────────────────┼────────┼──────────┤
│ DRE-380  │ Unified Agent Orchestration  │ Todo   │ 🤖 Running│
│ DRE-379  │ Fix login bug                │ Done   │          │
│ DRE-378  │ Add caching                  │ In Pr  │ 🟢 Idle   │
```

Agent status indicators:
- `🤖 Running` — agent actively working
- `🟢 Idle` — agent at prompt
- `🟡 Waiting` — agent waiting for input
- `✅ Done` — agent completed
- `🔴 Error` — agent errored

---

## 6. Configuration

### New Config Section

```rust
// Add to src/config/mod.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Default agent type for spawning
    #[serde(default = "default_agent_type")]
    pub default_agent: String,
    
    /// Status polling interval in seconds
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    
    /// Custom terminal command (overrides auto-detection)
    #[serde(default)]
    pub terminal_command: Option<String>,
    
    /// Custom terminal arguments (use {cmd} for command substitution)
    #[serde(default)]
    pub terminal_args: Option<Vec<String>>,
    
    /// Directory mappings: issue identifier prefix → directory
    /// e.g., "DRE" → "~/Programming/panopticon"
    #[serde(default)]
    pub directory_mappings: HashMap<String, String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            default_agent: "claude".to_string(),
            poll_interval_secs: 5,
            terminal_command: None,
            terminal_args: None,
            directory_mappings: HashMap::new(),
        }
    }
}

fn default_agent_type() -> String {
    "claude".to_string()
}

fn default_poll_interval() -> u64 {
    5
}
```

### Example Config

```toml
[agent]
default_agent = "claude"
poll_interval_secs = 5

# Optional: override terminal detection
# terminal_command = "wezterm"
# terminal_args = ["start", "--", "{cmd}"]

# Map issue prefixes to directories
[agent.directory_mappings]
DRE = "~/Programming/panopticon"
TEN = "~/Programming/Tensil"
```

---

## 7. File Structure

New files to create:

```
src/
├── agents/
│   ├── mod.rs           # Module exports
│   ├── registry.rs      # Session registry I/O
│   ├── spawn.rs         # Spawning logic
│   ├── teleport.rs      # Teleport/focus logic
│   ├── status.rs        # Status polling
│   └── terminal.rs      # Platform terminal detection
└── tui/
    └── ui/
        └── spawn_modal.rs  # Spawn modal rendering
```

---

## 8. Migration Path

### Phase 1: Registry + Data Structures
1. Create `src/agents/` module
2. Implement `SessionRegistry` with file I/O
3. Add `TrackedSession` to data model
4. Migrate existing `AgentSession` usage

### Phase 2: Spawning
1. Implement tmux session creation
2. Add spawn modal UI
3. Wire up `S` keybinding
4. Write sessions to registry on spawn

### Phase 3: Teleporting
1. Implement platform detection
2. Implement terminal emulator chain
3. Add `tmux attach` teleport
4. Keep existing window focus as fallback

### Phase 4: Status Tracking
1. Implement pane content polling
2. Add pattern-based status detection
3. Background sync loop
4. Merge with existing Claude hooks + OpenClaw API

### Phase 5: Polish
1. Session cleanup (prune old/dead sessions)
2. Kill session functionality
3. Config options
4. Error handling + user feedback

---

## 9. Open Questions

1. **Task generation**: Should we auto-generate task from issue description, or always require user input?

2. **Multiple agents per issue**: Allow spawning multiple agents for the same issue? (Currently: suffix with `-2`, `-3`, etc.)

3. **Session persistence**: How long to keep completed sessions in registry? (Proposal: 7 days, configurable)

4. **OpenClaw integration**: Should Panopticon be able to spawn OpenClaw sessions too, or keep that separate?

5. **Notifications**: Send desktop notification when agent completes? (Platform-specific)

---

## 10. Clean Architecture & Testability

### Core Principles (Uncle Bob)

1. **Dependency Inversion**: All I/O behind traits, injected at construction
2. **Single Responsibility**: One reason to change per module
3. **Pure Core**: Business logic is pure functions, side effects at edges
4. **Ports & Adapters**: Core knows nothing about tmux, filesystem, terminals

### Architectural Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                        UI Layer (TUI)                           │
│              Handles input, renders output                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Application Layer                            │
│         Orchestrates use cases, coordinates services            │
│   SpawnAgentUseCase, TeleportUseCase, PollStatusUseCase        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Domain Layer                               │
│        Pure business logic, entities, value objects             │
│   TrackedSession, SessionRegistry, StatusDetector              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Infrastructure Layer                           │
│            Implementations of port traits                       │
│   TmuxAdapter, FileSystemAdapter, TerminalAdapter, ClockAdapter│
└─────────────────────────────────────────────────────────────────┘
```

### Port Traits (Abstractions)

All external dependencies are abstracted behind traits. This enables:
- Unit testing with fakes/mocks
- Swapping implementations
- Clear boundaries

```rust
// src/agents/ports.rs

use async_trait::async_trait;

/// Clock abstraction for testable time
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// Filesystem abstraction
#[async_trait]
pub trait FileSystem: Send + Sync {
    async fn read_to_string(&self, path: &Path) -> Result<String>;
    async fn write(&self, path: &Path, contents: &str) -> Result<()>;
    async fn exists(&self, path: &Path) -> bool;
    async fn create_dir_all(&self, path: &Path) -> Result<()>;
}

/// tmux operations abstraction
#[async_trait]
pub trait TmuxClient: Send + Sync {
    async fn session_exists(&self, session: &str) -> Result<bool>;
    async fn create_session(&self, config: &TmuxSessionConfig) -> Result<()>;
    async fn kill_session(&self, session: &str) -> Result<()>;
    async fn send_keys(&self, session: &str, keys: &str) -> Result<()>;
    async fn capture_pane(&self, session: &str, lines: i32) -> Result<String>;
    async fn list_sessions(&self) -> Result<Vec<String>>;
}

#[derive(Debug, Clone)]
pub struct TmuxSessionConfig {
    pub name: String,
    pub working_dir: PathBuf,
    pub width: u16,
    pub height: u16,
}

/// Terminal launcher abstraction
#[async_trait]
pub trait TerminalLauncher: Send + Sync {
    async fn launch(&self, command: &str) -> Result<()>;
    fn is_available(&self) -> bool;
}

/// Command execution abstraction (for `which`, process spawning)
#[async_trait]
pub trait CommandRunner: Send + Sync {
    fn which(&self, binary: &str) -> Option<PathBuf>;
    async fn spawn(&self, command: &str, args: &[&str]) -> Result<()>;
    async fn output(&self, command: &str, args: &[&str]) -> Result<CommandOutput>;
}

#[derive(Debug)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

/// Environment variable access
pub trait Environment: Send + Sync {
    fn var(&self, key: &str) -> Option<String>;
    fn home_dir(&self) -> Option<PathBuf>;
}

/// Platform detection
pub trait PlatformDetector: Send + Sync {
    fn detect(&self) -> Platform;
}
```

### Domain Layer (Pure)

Business logic with no I/O dependencies:

```rust
// src/agents/domain/session.rs

/// Pure domain entity - no I/O
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackedSession {
    // ... fields as before ...
}

impl TrackedSession {
    /// Pure constructor with validation
    pub fn new(
        id: String,
        agent_type: AgentType,
        working_directory: String,
        created_at: DateTime<Utc>,
    ) -> Result<Self, ValidationError> {
        if id.is_empty() {
            return Err(ValidationError::EmptySessionId);
        }
        if working_directory.is_empty() {
            return Err(ValidationError::EmptyWorkingDirectory);
        }
        
        Ok(Self {
            id,
            agent_type,
            status: AgentStatus::Running,
            tmux_session: None,
            tmux_socket: None,
            working_directory,
            git_branch: None,
            linear_issue_id: None,
            linear_issue_identifier: None,
            task: None,
            created_at,
            last_activity: created_at,
            last_output: None,
            source: SessionSource::Panopticon,
        })
    }
    
    /// Pure state transition
    pub fn update_status(&mut self, status: AgentStatus, now: DateTime<Utc>) {
        if self.status != status {
            self.status = status;
            self.last_activity = now;
        }
    }
    
    /// Pure: check if session is stale
    pub fn is_stale(&self, now: DateTime<Utc>, threshold: Duration) -> bool {
        now.signed_duration_since(self.last_activity) > threshold
    }
    
    /// Pure: link to issue
    pub fn link_to_issue(&mut self, id: String, identifier: String) {
        self.linear_issue_id = Some(id);
        self.linear_issue_identifier = Some(identifier);
    }
}

// src/agents/domain/registry.rs

/// Pure registry operations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionRegistry {
    pub version: u32,
    pub sessions: HashMap<String, TrackedSession>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            version: 1,
            sessions: HashMap::new(),
        }
    }
    
    /// Pure: add session
    pub fn add(&mut self, session: TrackedSession) -> Result<(), RegistryError> {
        if self.sessions.contains_key(&session.id) {
            return Err(RegistryError::SessionExists(session.id));
        }
        self.sessions.insert(session.id.clone(), session);
        Ok(())
    }
    
    /// Pure: remove session
    pub fn remove(&mut self, id: &str) -> Option<TrackedSession> {
        self.sessions.remove(id)
    }
    
    /// Pure: find by git branch
    pub fn find_by_branch(&self, branch: &str) -> Option<&TrackedSession> {
        self.sessions.values()
            .find(|s| s.git_branch.as_deref() == Some(branch))
    }
    
    /// Pure: find by issue identifier
    pub fn find_by_issue(&self, identifier: &str) -> Option<&TrackedSession> {
        self.sessions.values()
            .find(|s| s.linear_issue_identifier.as_deref() == Some(identifier))
    }
    
    /// Pure: prune old sessions
    pub fn prune(&mut self, now: DateTime<Utc>, max_age: Duration) -> Vec<TrackedSession> {
        let stale_ids: Vec<String> = self.sessions.iter()
            .filter(|(_, s)| s.is_stale(now, max_age) && s.status == AgentStatus::Done)
            .map(|(id, _)| id.clone())
            .collect();
        
        stale_ids.iter()
            .filter_map(|id| self.sessions.remove(id))
            .collect()
    }
}

// src/agents/domain/status.rs

/// Pure status detection from pane content
pub struct StatusDetector {
    working_patterns: Vec<Regex>,
    idle_patterns: Vec<Regex>,
    done_patterns: Vec<Regex>,
    error_patterns: Vec<Regex>,
}

impl StatusDetector {
    pub fn new() -> Self {
        Self {
            working_patterns: vec![
                Regex::new(r"(?i)(thinking|working|reading|writing|searching)").unwrap(),
                Regex::new(r"[✻✶✽⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]").unwrap(),
            ],
            idle_patterns: vec![
                Regex::new(r"^❯\s*$").unwrap(),
                Regex::new(r"^\$\s*$").unwrap(),
                Regex::new(r"(?i)INSERT|NORMAL").unwrap(),
            ],
            done_patterns: vec![
                Regex::new(r"(?i)task complete|finished|done").unwrap(),
            ],
            error_patterns: vec![
                Regex::new(r"(?i)error|failed|exception|panic").unwrap(),
            ],
        }
    }
    
    /// Pure function: content in, status out
    pub fn detect(&self, content: &str) -> AgentStatus {
        let recent_lines: Vec<&str> = content.lines().rev().take(5).collect();
        let recent = recent_lines.join("\n");
        
        if self.matches_any(&self.error_patterns, &recent) {
            AgentStatus::Error
        } else if self.matches_any(&self.done_patterns, &recent) {
            AgentStatus::Done
        } else if self.matches_any(&self.working_patterns, &recent) {
            AgentStatus::Running
        } else if self.matches_any(&self.idle_patterns, &recent) {
            AgentStatus::Idle
        } else {
            AgentStatus::Unknown
        }
    }
    
    fn matches_any(&self, patterns: &[Regex], text: &str) -> bool {
        patterns.iter().any(|p| p.is_match(text))
    }
    
    /// Pure function: extract last meaningful output
    pub fn extract_last_output(&self, content: &str) -> Option<String> {
        content.lines()
            .rev()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() 
                    && !trimmed.starts_with('❯')
                    && !trimmed.starts_with('$')
                    && trimmed.len() > 3
            })
            .next()
            .map(|s| truncate(s, 100))
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max - 3])
    } else {
        s.to_string()
    }
}

// src/agents/domain/naming.rs

/// Pure session name generation
pub struct SessionNameGenerator;

impl SessionNameGenerator {
    /// Pure function: generate base name from inputs
    pub fn generate_base(agent_type: AgentType, issue: Option<&str>) -> String {
        let prefix = match agent_type {
            AgentType::ClaudeCode => "claude",
            AgentType::Codex => "codex",
            AgentType::OpenClaw => "openclaw",
            AgentType::Other => "agent",
        };
        
        match issue {
            Some(id) => format!("{}-{}", prefix, id.to_lowercase()),
            None => prefix.to_string(),
        }
    }
    
    /// Pure function: add suffix to avoid collision
    pub fn with_suffix(base: &str, existing: &[String]) -> String {
        if !existing.contains(&base.to_string()) {
            return base.to_string();
        }
        
        for i in 2..100 {
            let candidate = format!("{}-{}", base, i);
            if !existing.contains(&candidate) {
                return candidate;
            }
        }
        
        // Fallback: use timestamp
        format!("{}-{}", base, chrono::Utc::now().timestamp())
    }
}
```

### Application Layer (Use Cases)

Orchestrates domain logic with injected dependencies:

```rust
// src/agents/usecases/spawn.rs

pub struct SpawnAgentUseCase<T, F, C>
where
    T: TmuxClient,
    F: FileSystem,
    C: Clock,
{
    tmux: T,
    fs: F,
    clock: C,
    registry_path: PathBuf,
}

impl<T, F, C> SpawnAgentUseCase<T, F, C>
where
    T: TmuxClient,
    F: FileSystem,
    C: Clock,
{
    pub fn new(tmux: T, fs: F, clock: C, registry_path: PathBuf) -> Self {
        Self { tmux, fs, clock, registry_path }
    }
    
    pub async fn execute(&self, request: SpawnRequest) -> Result<TrackedSession> {
        // 1. Load registry (via injected fs)
        let mut registry = self.load_registry().await?;
        
        // 2. Generate session name (pure)
        let existing: Vec<String> = self.tmux.list_sessions().await?;
        let base_name = SessionNameGenerator::generate_base(
            request.agent_type,
            request.issue.as_ref().map(|i| i.identifier.as_str()),
        );
        let session_name = SessionNameGenerator::with_suffix(&base_name, &existing);
        
        // 3. Create tmux session (via injected tmux client)
        self.tmux.create_session(&TmuxSessionConfig {
            name: session_name.clone(),
            working_dir: request.working_directory.clone(),
            width: 200,
            height: 50,
        }).await?;
        
        // 4. Build and send command (pure command building)
        let command = build_agent_command(request.agent_type, &request.task);
        self.tmux.send_keys(&session_name, &command).await?;
        
        // 5. Create session entity (pure)
        let now = self.clock.now();
        let mut session = TrackedSession::new(
            session_name.clone(),
            request.agent_type,
            request.working_directory.to_string_lossy().to_string(),
            now,
        )?;
        session.tmux_session = Some(session_name);
        session.task = Some(request.task);
        session.git_branch = request.git_branch;
        if let Some(issue) = request.issue {
            session.link_to_issue(issue.id, issue.identifier);
        }
        
        // 6. Update registry (pure) and persist (via injected fs)
        registry.add(session.clone())?;
        self.save_registry(&registry).await?;
        
        Ok(session)
    }
    
    async fn load_registry(&self) -> Result<SessionRegistry> {
        if !self.fs.exists(&self.registry_path).await {
            return Ok(SessionRegistry::new());
        }
        let content = self.fs.read_to_string(&self.registry_path).await?;
        Ok(serde_json::from_str(&content)?)
    }
    
    async fn save_registry(&self, registry: &SessionRegistry) -> Result<()> {
        let content = serde_json::to_string_pretty(registry)?;
        self.fs.write(&self.registry_path, &content).await
    }
}

/// Pure function: build agent command string
fn build_agent_command(agent_type: AgentType, task: &str) -> String {
    let escaped = task.replace("'", "'\\''");
    match agent_type {
        AgentType::ClaudeCode => format!("claude --dangerously-skip-permissions '{}'", escaped),
        AgentType::Codex => format!("codex --full-auto exec '{}'", escaped),
        _ => format!("echo 'Unknown agent type'"),
    }
}

// src/agents/usecases/teleport.rs

pub struct TeleportUseCase<T, E, P>
where
    T: TerminalLauncher,
    E: Environment,
    P: PlatformDetector,
{
    terminal: T,
    env: E,
    platform: P,
}

impl<T, E, P> TeleportUseCase<T, E, P>
where
    T: TerminalLauncher,
    E: Environment,
    P: PlatformDetector,
{
    pub async fn execute(&self, session: &TrackedSession) -> Result<()> {
        let tmux_session = session.tmux_session.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No tmux session"))?;
        
        let attach_cmd = build_attach_command(tmux_session, session.tmux_socket.as_deref());
        self.terminal.launch(&attach_cmd).await
    }
}

/// Pure function: build tmux attach command
fn build_attach_command(session: &str, socket: Option<&str>) -> String {
    match socket {
        Some(s) => format!("tmux -S {} attach -t {}", s, session),
        None => format!("tmux attach -t {}", session),
    }
}

// src/agents/usecases/poll_status.rs

pub struct PollStatusUseCase<T, F, C>
where
    T: TmuxClient,
    F: FileSystem,
    C: Clock,
{
    tmux: T,
    fs: F,
    clock: C,
    detector: StatusDetector,
    registry_path: PathBuf,
}

impl<T, F, C> PollStatusUseCase<T, F, C>
where
    T: TmuxClient,
    F: FileSystem,
    C: Clock,
{
    pub async fn execute(&self) -> Result<Vec<TrackedSession>> {
        let mut registry = self.load_registry().await?;
        let active_sessions = self.tmux.list_sessions().await?;
        let now = self.clock.now();
        
        for session in registry.sessions.values_mut() {
            let tmux_session = match &session.tmux_session {
                Some(s) => s.clone(),
                None => continue,
            };
            
            // Session disappeared = done
            if !active_sessions.contains(&tmux_session) {
                session.update_status(AgentStatus::Done, now);
                continue;
            }
            
            // Capture and detect (pure detection)
            if let Ok(content) = self.tmux.capture_pane(&tmux_session, 30).await {
                let status = self.detector.detect(&content);
                session.update_status(status, now);
                
                if let Some(output) = self.detector.extract_last_output(&content) {
                    session.last_output = Some(output);
                }
            }
        }
        
        self.save_registry(&registry).await?;
        Ok(registry.sessions.values().cloned().collect())
    }
    
    // ... load/save methods same as SpawnAgentUseCase
}
```

### Infrastructure Layer (Adapters)

Real implementations of port traits:

```rust
// src/agents/adapters/real_tmux.rs

pub struct RealTmuxClient;

#[async_trait]
impl TmuxClient for RealTmuxClient {
    async fn session_exists(&self, session: &str) -> Result<bool> {
        let output = tokio::process::Command::new("tmux")
            .args(["has-session", "-t", session])
            .output()
            .await?;
        Ok(output.status.success())
    }
    
    async fn create_session(&self, config: &TmuxSessionConfig) -> Result<()> {
        let status = tokio::process::Command::new("tmux")
            .args([
                "new-session", "-d",
                "-s", &config.name,
                "-c", &config.working_dir.to_string_lossy(),
                "-x", &config.width.to_string(),
                "-y", &config.height.to_string(),
            ])
            .status()
            .await?;
        
        if !status.success() {
            anyhow::bail!("tmux new-session failed");
        }
        Ok(())
    }
    
    // ... other methods
}

// src/agents/adapters/real_fs.rs

pub struct RealFileSystem;

#[async_trait]
impl FileSystem for RealFileSystem {
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        // Use file locking
        let file = std::fs::File::open(path)?;
        file.lock_shared()?;
        let content = std::fs::read_to_string(path)?;
        file.unlock()?;
        Ok(content)
    }
    
    async fn write(&self, path: &Path, contents: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::File::create(path)?;
        file.lock_exclusive()?;
        std::fs::write(path, contents)?;
        file.unlock()?;
        Ok(())
    }
    
    // ... other methods
}

// src/agents/adapters/real_clock.rs

pub struct RealClock;

impl Clock for RealClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
```

### Test Doubles (Fakes)

```rust
// src/agents/adapters/fake_tmux.rs

#[derive(Default)]
pub struct FakeTmuxClient {
    pub sessions: Arc<Mutex<HashMap<String, FakeSession>>>,
    pub fail_on: Arc<Mutex<HashSet<String>>>,  // Session names that should fail
}

pub struct FakeSession {
    pub config: TmuxSessionConfig,
    pub pane_content: String,
    pub keys_sent: Vec<String>,
}

#[async_trait]
impl TmuxClient for FakeTmuxClient {
    async fn session_exists(&self, session: &str) -> Result<bool> {
        Ok(self.sessions.lock().unwrap().contains_key(session))
    }
    
    async fn create_session(&self, config: &TmuxSessionConfig) -> Result<()> {
        if self.fail_on.lock().unwrap().contains(&config.name) {
            anyhow::bail!("Fake failure");
        }
        self.sessions.lock().unwrap().insert(
            config.name.clone(),
            FakeSession {
                config: config.clone(),
                pane_content: String::new(),
                keys_sent: vec![],
            },
        );
        Ok(())
    }
    
    async fn send_keys(&self, session: &str, keys: &str) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap();
        let s = sessions.get_mut(session)
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;
        s.keys_sent.push(keys.to_string());
        Ok(())
    }
    
    async fn capture_pane(&self, session: &str, _lines: i32) -> Result<String> {
        let sessions = self.sessions.lock().unwrap();
        let s = sessions.get(session)
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;
        Ok(s.pane_content.clone())
    }
    
    async fn list_sessions(&self) -> Result<Vec<String>> {
        Ok(self.sessions.lock().unwrap().keys().cloned().collect())
    }
    
    // ... other methods
}

impl FakeTmuxClient {
    /// Test helper: set pane content for status detection tests
    pub fn set_pane_content(&self, session: &str, content: &str) {
        if let Some(s) = self.sessions.lock().unwrap().get_mut(session) {
            s.pane_content = content.to_string();
        }
    }
}

// src/agents/adapters/fake_fs.rs

#[derive(Default)]
pub struct FakeFileSystem {
    pub files: Arc<Mutex<HashMap<PathBuf, String>>>,
}

#[async_trait]
impl FileSystem for FakeFileSystem {
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        self.files.lock().unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("File not found"))
    }
    
    async fn write(&self, path: &Path, contents: &str) -> Result<()> {
        self.files.lock().unwrap().insert(path.to_path_buf(), contents.to_string());
        Ok(())
    }
    
    async fn exists(&self, path: &Path) -> bool {
        self.files.lock().unwrap().contains_key(path)
    }
    
    async fn create_dir_all(&self, _path: &Path) -> Result<()> {
        Ok(())  // No-op for fake
    }
}

// src/agents/adapters/fake_clock.rs

pub struct FakeClock {
    pub now: Arc<Mutex<DateTime<Utc>>>,
}

impl FakeClock {
    pub fn new(time: DateTime<Utc>) -> Self {
        Self { now: Arc::new(Mutex::new(time)) }
    }
    
    pub fn advance(&self, duration: chrono::Duration) {
        let mut now = self.now.lock().unwrap();
        *now = *now + duration;
    }
    
    pub fn set(&self, time: DateTime<Utc>) {
        *self.now.lock().unwrap() = time;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap()
    }
}
```

---

## 11. Test Strategy

### Test Pyramid

```
                    ┌─────────┐
                    │ E2E (3) │  ← Few, slow, real tmux
                   ┌┴─────────┴┐
                   │ Integ (10)│  ← Real FS, fake tmux
                  ┌┴───────────┴┐
                  │ Unit (50+)  │  ← Fast, pure, isolated
                 └──────────────┘
```

### Unit Tests (Domain Layer)

Fast, pure, no I/O. Run on every save.

```rust
// tests/domain/session_tests.rs

#[test]
fn test_session_creation_validates_id() {
    let result = TrackedSession::new(
        "".to_string(),
        AgentType::ClaudeCode,
        "/home/user".to_string(),
        Utc::now(),
    );
    assert!(matches!(result, Err(ValidationError::EmptySessionId)));
}

#[test]
fn test_session_status_update_changes_activity() {
    let now = Utc::now();
    let mut session = TrackedSession::new(
        "test".to_string(),
        AgentType::ClaudeCode,
        "/home/user".to_string(),
        now,
    ).unwrap();
    
    let later = now + chrono::Duration::seconds(60);
    session.update_status(AgentStatus::Done, later);
    
    assert_eq!(session.status, AgentStatus::Done);
    assert_eq!(session.last_activity, later);
}

#[test]
fn test_session_stale_detection() {
    let start = Utc::now();
    let session = TrackedSession::new(
        "test".to_string(),
        AgentType::ClaudeCode,
        "/home/user".to_string(),
        start,
    ).unwrap();
    
    let threshold = chrono::Duration::minutes(30);
    
    // Not stale after 10 minutes
    assert!(!session.is_stale(start + chrono::Duration::minutes(10), threshold));
    
    // Stale after 31 minutes
    assert!(session.is_stale(start + chrono::Duration::minutes(31), threshold));
}

// tests/domain/registry_tests.rs

#[test]
fn test_registry_add_prevents_duplicates() {
    let mut registry = SessionRegistry::new();
    let session = make_test_session("test-1");
    
    assert!(registry.add(session.clone()).is_ok());
    assert!(matches!(
        registry.add(session),
        Err(RegistryError::SessionExists(_))
    ));
}

#[test]
fn test_registry_find_by_branch() {
    let mut registry = SessionRegistry::new();
    let mut session = make_test_session("test-1");
    session.git_branch = Some("feat/login".to_string());
    registry.add(session).unwrap();
    
    assert!(registry.find_by_branch("feat/login").is_some());
    assert!(registry.find_by_branch("feat/other").is_none());
}

#[test]
fn test_registry_prune_removes_old_done_sessions() {
    let start = Utc::now();
    let mut registry = SessionRegistry::new();
    
    // Old done session - should be pruned
    let mut old = make_test_session("old");
    old.status = AgentStatus::Done;
    old.last_activity = start - chrono::Duration::days(10);
    registry.add(old).unwrap();
    
    // Recent done session - should stay
    let mut recent = make_test_session("recent");
    recent.status = AgentStatus::Done;
    recent.last_activity = start - chrono::Duration::hours(1);
    registry.add(recent).unwrap();
    
    // Running session - should stay regardless of age
    let running = make_test_session("running");
    registry.add(running).unwrap();
    
    let pruned = registry.prune(start, chrono::Duration::days(7));
    
    assert_eq!(pruned.len(), 1);
    assert_eq!(pruned[0].id, "old");
    assert_eq!(registry.sessions.len(), 2);
}

// tests/domain/status_detector_tests.rs

#[test]
fn test_detect_running_from_spinner() {
    let detector = StatusDetector::new();
    let content = "Reading file.rs...\n✻ Thinking about approach";
    assert_eq!(detector.detect(content), AgentStatus::Running);
}

#[test]
fn test_detect_idle_from_prompt() {
    let detector = StatusDetector::new();
    let content = "Done editing.\n❯ ";
    assert_eq!(detector.detect(content), AgentStatus::Idle);
}

#[test]
fn test_detect_error_from_panic() {
    let detector = StatusDetector::new();
    let content = "thread 'main' panicked at 'oops'";
    assert_eq!(detector.detect(content), AgentStatus::Error);
}

// tests/domain/naming_tests.rs

#[test]
fn test_generate_base_name_with_issue() {
    let name = SessionNameGenerator::generate_base(AgentType::ClaudeCode, Some("DRE-380"));
    assert_eq!(name, "claude-dre-380");
}

#[test]
fn test_generate_base_name_without_issue() {
    let name = SessionNameGenerator::generate_base(AgentType::Codex, None);
    assert_eq!(name, "codex");
}

#[test]
fn test_with_suffix_avoids_collision() {
    let existing = vec!["claude-dre-380".to_string(), "claude-dre-380-2".to_string()];
    let name = SessionNameGenerator::with_suffix("claude-dre-380", &existing);
    assert_eq!(name, "claude-dre-380-3");
}

#[test]
fn test_with_suffix_no_collision() {
    let existing = vec!["other-session".to_string()];
    let name = SessionNameGenerator::with_suffix("claude-dre-380", &existing);
    assert_eq!(name, "claude-dre-380");
}
```

### Integration Tests (Use Cases with Fakes)

Test use case orchestration with fake adapters:

```rust
// tests/usecases/spawn_tests.rs

#[tokio::test]
async fn test_spawn_creates_session_and_updates_registry() {
    // Arrange
    let tmux = FakeTmuxClient::default();
    let fs = FakeFileSystem::default();
    let clock = FakeClock::new(Utc::now());
    let registry_path = PathBuf::from("/tmp/test-registry.json");
    
    let use_case = SpawnAgentUseCase::new(tmux.clone(), fs.clone(), clock, registry_path.clone());
    
    let request = SpawnRequest {
        agent_type: AgentType::ClaudeCode,
        working_directory: PathBuf::from("/home/user/project"),
        git_branch: Some("feat/test".to_string()),
        issue: Some(LinearIssueRef {
            id: "123".to_string(),
            identifier: "DRE-100".to_string(),
            title: "Test issue".to_string(),
        }),
        task: "Fix the bug".to_string(),
    };
    
    // Act
    let session = use_case.execute(request).await.unwrap();
    
    // Assert - session created correctly
    assert_eq!(session.id, "claude-dre-100");
    assert_eq!(session.status, AgentStatus::Running);
    assert_eq!(session.git_branch, Some("feat/test".to_string()));
    
    // Assert - tmux session created
    assert!(tmux.session_exists("claude-dre-100").await.unwrap());
    
    // Assert - command sent
    let sessions = tmux.sessions.lock().unwrap();
    let tmux_session = sessions.get("claude-dre-100").unwrap();
    assert!(tmux_session.keys_sent[0].contains("claude --dangerously-skip-permissions"));
    
    // Assert - registry updated
    let registry_content = fs.read_to_string(&registry_path).await.unwrap();
    let registry: SessionRegistry = serde_json::from_str(&registry_content).unwrap();
    assert!(registry.sessions.contains_key("claude-dre-100"));
}

#[tokio::test]
async fn test_spawn_handles_name_collision() {
    let tmux = FakeTmuxClient::default();
    // Pre-create existing session
    tmux.create_session(&TmuxSessionConfig {
        name: "claude-dre-100".to_string(),
        working_dir: PathBuf::from("/tmp"),
        width: 200,
        height: 50,
    }).await.unwrap();
    
    let fs = FakeFileSystem::default();
    let clock = FakeClock::new(Utc::now());
    
    let use_case = SpawnAgentUseCase::new(tmux, fs, clock, PathBuf::from("/tmp/reg.json"));
    
    let request = SpawnRequest {
        agent_type: AgentType::ClaudeCode,
        working_directory: PathBuf::from("/home/user"),
        git_branch: None,
        issue: Some(LinearIssueRef {
            id: "123".to_string(),
            identifier: "DRE-100".to_string(),
            title: "Test".to_string(),
        }),
        task: "Do thing".to_string(),
    };
    
    let session = use_case.execute(request).await.unwrap();
    
    // Should get suffixed name
    assert_eq!(session.id, "claude-dre-100-2");
}

// tests/usecases/poll_status_tests.rs

#[tokio::test]
async fn test_poll_detects_running_status() {
    let tmux = FakeTmuxClient::default();
    let fs = FakeFileSystem::default();
    let clock = FakeClock::new(Utc::now());
    let registry_path = PathBuf::from("/tmp/reg.json");
    
    // Set up existing session in registry
    let mut registry = SessionRegistry::new();
    let mut session = make_test_session("test-session");
    session.tmux_session = Some("test-session".to_string());
    registry.add(session).unwrap();
    fs.write(&registry_path, &serde_json::to_string(&registry).unwrap()).await.unwrap();
    
    // Create matching tmux session with "working" content
    tmux.create_session(&TmuxSessionConfig {
        name: "test-session".to_string(),
        working_dir: PathBuf::from("/tmp"),
        width: 200,
        height: 50,
    }).await.unwrap();
    tmux.set_pane_content("test-session", "✻ Thinking about the problem...");
    
    let use_case = PollStatusUseCase::new(tmux, fs.clone(), clock, registry_path.clone());
    
    // Act
    let sessions = use_case.execute().await.unwrap();
    
    // Assert
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].status, AgentStatus::Running);
}

#[tokio::test]
async fn test_poll_marks_missing_session_as_done() {
    let tmux = FakeTmuxClient::default();
    let fs = FakeFileSystem::default();
    let clock = FakeClock::new(Utc::now());
    let registry_path = PathBuf::from("/tmp/reg.json");
    
    // Session in registry but NOT in tmux
    let mut registry = SessionRegistry::new();
    let mut session = make_test_session("gone-session");
    session.tmux_session = Some("gone-session".to_string());
    registry.add(session).unwrap();
    fs.write(&registry_path, &serde_json::to_string(&registry).unwrap()).await.unwrap();
    
    let use_case = PollStatusUseCase::new(tmux, fs, clock, registry_path);
    
    let sessions = use_case.execute().await.unwrap();
    
    assert_eq!(sessions[0].status, AgentStatus::Done);
}
```

### E2E Tests (Real tmux)

Run separately, require tmux installed:

```rust
// tests/e2e/real_tmux_tests.rs

#[tokio::test]
#[ignore]  // Run with: cargo test -- --ignored
async fn test_real_tmux_session_lifecycle() {
    let tmux = RealTmuxClient;
    let session_name = format!("panopticon-test-{}", std::process::id());
    
    // Create
    tmux.create_session(&TmuxSessionConfig {
        name: session_name.clone(),
        working_dir: std::env::temp_dir(),
        width: 80,
        height: 24,
    }).await.unwrap();
    
    assert!(tmux.session_exists(&session_name).await.unwrap());
    
    // Send keys
    tmux.send_keys(&session_name, "echo 'hello world'").await.unwrap();
    
    // Wait a bit for command to execute
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Capture
    let content = tmux.capture_pane(&session_name, 10).await.unwrap();
    assert!(content.contains("hello world"));
    
    // Clean up
    tmux.kill_session(&session_name).await.unwrap();
    assert!(!tmux.session_exists(&session_name).await.unwrap());
}
```

### Test Helpers

```rust
// tests/helpers.rs

pub fn make_test_session(id: &str) -> TrackedSession {
    TrackedSession::new(
        id.to_string(),
        AgentType::ClaudeCode,
        "/tmp/test".to_string(),
        Utc::now(),
    ).unwrap()
}

pub fn make_test_registry(sessions: Vec<TrackedSession>) -> SessionRegistry {
    let mut registry = SessionRegistry::new();
    for session in sessions {
        registry.add(session).unwrap();
    }
    registry
}
```

---

## 12. Agentic Implementation Loop

### Phase Verification Gates

Each phase must pass its tests before proceeding:

```bash
# Phase 1: Domain Layer
cargo test domain:: -- --nocapture
# Must pass: 20+ unit tests for Session, Registry, StatusDetector, Naming

# Phase 2: Ports & Fakes
cargo test adapters::fake -- --nocapture
# Must pass: Fake implementations work correctly

# Phase 3: Use Cases
cargo test usecases:: -- --nocapture
# Must pass: Spawn, Teleport, PollStatus with fakes

# Phase 4: Real Adapters
cargo test adapters::real -- --nocapture
# Must pass: RealTmuxClient, RealFileSystem basic functionality

# Phase 5: E2E
cargo test e2e:: -- --ignored
# Must pass: Full lifecycle with real tmux

# Phase 6: UI Integration
cargo test tui:: -- --nocapture
# Must pass: Modal state, keybindings, message handling
```

### Implementation Order

```
1. src/agents/domain/          ← Pure, test first
   ├── mod.rs
   ├── session.rs              + tests/domain/session_tests.rs
   ├── registry.rs             + tests/domain/registry_tests.rs
   ├── status.rs               + tests/domain/status_detector_tests.rs
   └── naming.rs               + tests/domain/naming_tests.rs

2. src/agents/ports.rs         ← Trait definitions only

3. src/agents/adapters/        ← Fakes first, then real
   ├── mod.rs
   ├── fake_tmux.rs            + tests/adapters/fake_tmux_tests.rs
   ├── fake_fs.rs
   ├── fake_clock.rs
   ├── real_tmux.rs            + tests/adapters/real_tmux_tests.rs
   ├── real_fs.rs
   └── real_clock.rs

4. src/agents/usecases/        ← Test with fakes
   ├── mod.rs
   ├── spawn.rs                + tests/usecases/spawn_tests.rs
   ├── teleport.rs             + tests/usecases/teleport_tests.rs
   └── poll_status.rs          + tests/usecases/poll_status_tests.rs

5. src/agents/terminal.rs      ← Platform detection
   + tests/terminal_tests.rs

6. src/tui/ui/spawn_modal.rs   ← UI rendering
   + tests/tui/spawn_modal_tests.rs

7. Wire up in src/tui/app.rs   ← Integration
```

### CI-Friendly Test Commands

```bash
# Fast feedback (unit only, < 5s)
cargo test domain:: naming:: status:: registry:: session::

# Full validation (all except E2E, < 30s)
cargo test --lib

# Complete (including E2E, requires tmux)
cargo test -- --include-ignored
```

---

*End of Spec*
