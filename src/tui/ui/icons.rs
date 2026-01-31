//! Nerd Font icons used throughout the UI.

// Column header icons
pub const HEADER_STATUS: &str = "◐"; // Status indicator
pub const HEADER_ID: &str = ""; // nf-cod-issue_opened (ticket)
pub const HEADER_PR: &str = ""; // nf-dev-github_badge
pub const HEADER_AGENT: &str = "󰚩"; // nf-md-robot
pub const HEADER_VERCEL: &str = "▲"; // Vercel triangle
pub const HEADER_TIME: &str = "󰥔"; // nf-md-clock_outline

// Priority icons (signal bar style)
pub const PRIORITY_NONE: &str = "╌╌╌"; // Gray dashes - no priority
pub const PRIORITY_URGENT: &str = "⚠!"; // Warning + exclaim - urgent (will have orange bg)
pub const PRIORITY_HIGH: &str = "▮▮▮"; // 3 bars - high
pub const PRIORITY_MEDIUM: &str = "▮▮╌"; // 2 bars - medium
pub const PRIORITY_LOW: &str = "▮╌╌"; // 1 bar - low

// Linear Status - Fractional circles (like Linear app)
pub const STATUS_TRIAGE: &str = "◇"; // Diamond outline - needs triage
pub const STATUS_BACKLOG: &str = "○"; // Empty circle
pub const STATUS_TODO: &str = "◔"; // 1/4 filled
pub const STATUS_IN_PROGRESS: &str = "◑"; // 1/2 filled
pub const STATUS_IN_REVIEW: &str = "◕"; // 3/4 filled
pub const STATUS_DONE: &str = "●"; // Full circle
pub const STATUS_CANCELED: &str = "⊘"; // Slashed circle
pub const STATUS_DUPLICATE: &str = "◈"; // Diamond fill - duplicate

// PR Status
pub const PR_DRAFT: &str = "󰏫"; // nf-md-file_document_edit_outline
pub const PR_OPEN: &str = "󰐊"; // nf-md-play
pub const PR_REVIEW: &str = "󰈈"; // nf-md-eye
pub const PR_CHANGES: &str = "󰏭"; // nf-md-file_document_alert
pub const PR_APPROVED: &str = "󰄬"; // nf-md-check
pub const PR_MERGED: &str = "󰜛"; // nf-md-source_merge
pub const PR_CLOSED: &str = "󰅖"; // nf-md-close

// Agent Status (Claude-like)
pub const AGENT_RUNNING: &str = "󰇌"; // nf-md-brain
pub const AGENT_IDLE: &str = "󰏤"; // nf-md-pause
pub const AGENT_WAITING: &str = "󰁤"; // nf-md-keyboard
pub const AGENT_DONE: &str = "󰄬"; // nf-md-check
pub const AGENT_ERROR: &str = "󰅚"; // nf-md-close_circle
pub const AGENT_NONE: &str = "󰝦"; // nf-md-minus_circle_outline

// Agent ASCII fallbacks (single-char)
pub const AGENT_RUNNING_ASCII: char = '*';
pub const AGENT_IDLE_ASCII: char = '-';
pub const AGENT_WAITING_ASCII: char = '?';
pub const AGENT_DONE_ASCII: char = 'v';
pub const AGENT_ERROR_ASCII: char = '!';

// Vercel Status
pub const VERCEL_READY: &str = "󰄬"; // nf-md-check
pub const VERCEL_BUILDING: &str = "󰑮"; // nf-md-cog_sync
pub const VERCEL_QUEUED: &str = "󰔟"; // nf-md-clock_outline
pub const VERCEL_ERROR: &str = "󰅚"; // nf-md-close_circle
pub const VERCEL_NONE: &str = "󰝦"; // nf-md-minus_circle_outline

// Section indicators
pub const EXPANDED: &str = "▼";
pub const COLLAPSED: &str = "▶";

// Issue detail category icons
pub const ICON_TEAM: &str = "󰏬"; // nf-md-account_group
pub const ICON_PROJECT: &str = "󰈙"; // nf-md-folder
pub const ICON_CYCLE: &str = "󰃰"; // nf-md-calendar_clock
pub const ICON_ESTIMATE: &str = "󰎚"; // nf-md-numeric
pub const ICON_LABELS: &str = "󰌕"; // nf-md-tag_multiple
pub const ICON_CREATED: &str = "󰃭"; // nf-md-calendar_plus
pub const ICON_UPDATED: &str = "󰦒"; // nf-md-calendar_edit
pub const ICON_DOCUMENT: &str = "󰈚"; // nf-md-file_document
pub const ICON_PARENT: &str = "󰁝"; // nf-md-arrow_up_bold
pub const ICON_CHILDREN: &str = "󰁅"; // nf-md-arrow_down_bold

// Tool activity icons (for agent status display)
pub const TOOL_READ: &str = "󰈙"; // nf-md-file_document
pub const TOOL_EDIT: &str = "󰏫"; // nf-md-pencil
pub const TOOL_WRITE: &str = "󰈔"; // nf-md-file_plus
pub const TOOL_BASH: &str = "󰆍"; // nf-md-console
pub const TOOL_GREP: &str = "󰍉"; // nf-md-magnify
pub const TOOL_GLOB: &str = "󰉋"; // nf-md-folder_search
pub const TOOL_WEB: &str = "󰖟"; // nf-md-web
pub const TOOL_TASK: &str = "󰜎"; // nf-md-robot_outline (subagent)
pub const THINKING: &str = "󰇌"; // nf-md-brain (same as AGENT_RUNNING)

// Tool activity ASCII fallbacks
pub const TOOL_READ_ASCII: char = 'R';
pub const TOOL_EDIT_ASCII: char = 'E';
pub const TOOL_WRITE_ASCII: char = 'W';
pub const TOOL_BASH_ASCII: char = '$';
pub const TOOL_GREP_ASCII: char = '?';
pub const TOOL_GLOB_ASCII: char = 'G';
pub const TOOL_WEB_ASCII: char = '@';
pub const TOOL_TASK_ASCII: char = 'T';
pub const THINKING_ASCII: char = '*';

// Permission mode badges
pub const MODE_PLAN: &str = "󰙅"; // nf-md-map_marker_path
pub const MODE_ACCEPT: &str = "󰄬"; // nf-md-check (same as done)
pub const MODE_YOLO: &str = "󱐋"; // nf-md-lightning_bolt

// Permission mode ASCII fallbacks
pub const MODE_PLAN_ASCII: char = 'P';
pub const MODE_ACCEPT_ASCII: char = 'A';
pub const MODE_YOLO_ASCII: char = 'Y';
