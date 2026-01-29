use super::App;
use crate::data::{AgentStatus, GitHubPRStatus, LinearPriority, LinearStatus, VercelStatus, VisualItem};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use super::app::{COL_IDX_STATUS, COL_IDX_PRIORITY, COL_IDX_ID, COL_IDX_TITLE, COL_IDX_PR, COL_IDX_AGENT, COL_IDX_VERCEL, COL_IDX_TIME};

// Nerd Font icons
mod icons {
    // Column header icons
    pub const HEADER_STATUS: &str = "◐";      // Status indicator
    pub const HEADER_ID: &str = "";          // nf-cod-issue_opened (ticket)
    pub const HEADER_PR: &str = "";          // nf-dev-github_badge
    pub const HEADER_AGENT: &str = "󰚩";       // nf-md-robot
    pub const HEADER_VERCEL: &str = "▲";      // Vercel triangle
    pub const HEADER_TIME: &str = "󰥔";        // nf-md-clock_outline

    // Priority icons (signal bar style)
    pub const PRIORITY_NONE: &str = "╌╌╌";    // Gray dashes - no priority
    pub const PRIORITY_URGENT: &str = "⚠!";   // Warning + exclaim - urgent (will have orange bg)
    pub const PRIORITY_HIGH: &str = "▮▮▮";    // 3 bars - high
    pub const PRIORITY_MEDIUM: &str = "▮▮╌";  // 2 bars - medium
    pub const PRIORITY_LOW: &str = "▮╌╌";     // 1 bar - low

    // Linear Status - Fractional circles (like Linear app)
    pub const STATUS_BACKLOG: &str = "○";     // Empty circle
    pub const STATUS_TODO: &str = "◔";        // 1/4 filled
    pub const STATUS_IN_PROGRESS: &str = "◑"; // 1/2 filled
    pub const STATUS_IN_REVIEW: &str = "◕";   // 3/4 filled
    pub const STATUS_DONE: &str = "●";        // Full circle
    pub const STATUS_CANCELED: &str = "⊘";    // Slashed circle

    // PR Status
    pub const PR_DRAFT: &str = "󰏫";      // nf-md-file_document_edit_outline
    pub const PR_OPEN: &str = "󰐊";       // nf-md-play
    pub const PR_REVIEW: &str = "󰈈";     // nf-md-eye
    pub const PR_CHANGES: &str = "󰏭";    // nf-md-file_document_alert
    pub const PR_APPROVED: &str = "󰄬";   // nf-md-check
    pub const PR_MERGED: &str = "󰜛";     // nf-md-source_merge
    pub const PR_CLOSED: &str = "󰅖";     // nf-md-close

    // Agent Status
    pub const AGENT_RUNNING: &str = "󰐊";  // nf-md-play
    pub const AGENT_IDLE: &str = "󰏤";     // nf-md-pause
    pub const AGENT_WAITING: &str = "󰋗";  // nf-md-help_circle
    pub const AGENT_DONE: &str = "󰄬";     // nf-md-check
    pub const AGENT_ERROR: &str = "󰅚";    // nf-md-close_circle
    pub const AGENT_NONE: &str = "󰝦";     // nf-md-minus_circle_outline

    // Vercel Status
    pub const VERCEL_READY: &str = "󰄬";    // nf-md-check
    pub const VERCEL_BUILDING: &str = "󰑮"; // nf-md-cog_sync
    pub const VERCEL_QUEUED: &str = "󰔟";   // nf-md-clock_outline
    pub const VERCEL_ERROR: &str = "󰅚";    // nf-md-close_circle
    pub const VERCEL_NONE: &str = "󰝦";     // nf-md-minus_circle_outline

    // Section indicators
    pub const EXPANDED: &str = "▼";
    pub const COLLAPSED: &str = "▶";
}

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header/search
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);
    draw_workstreams(f, app, chunks[1]);
    draw_status_bar(f, app, chunks[2]);

    // Overlays
    if app.show_help {
        draw_help_popup(f, app);
    }

    if app.show_link_menu {
        draw_link_menu(f, app);
    }

    if app.show_sort_menu {
        draw_sort_menu(f, app);
    }

    if app.show_filter_menu {
        draw_filter_menu(f, app);
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let active_count = app
        .state
        .workstreams
        .iter()
        .filter(|ws| {
            ws.agent_session
                .as_ref()
                .map(|s| {
                    s.status == AgentStatus::Running
                        || s.status == AgentStatus::WaitingForInput
                })
                .unwrap_or(false)
        })
        .count();

    let border_style = if app.state.search_mode {
        Style::default().fg(Color::Yellow)
    } else if app.is_loading {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Render centered text inside the block
    let text = if app.state.search_mode {
        Line::from(vec![
            Span::styled("󰍉 Search: ", Style::default().fg(Color::Yellow)),
            Span::styled(&app.state.search_query, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ])
    } else if app.is_loading {
        Line::from(vec![
            Span::styled("󰣖 ", Style::default().fg(Color::Cyan)),
            Span::styled("Panopticon ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{} Loading...", app.spinner_char()), Style::default().fg(Color::Cyan)),
        ])
    } else {
        Line::from(vec![
            Span::styled("󰣖 ", Style::default().fg(Color::Cyan)),
            Span::styled("Panopticon ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(format!("[{} active]", active_count), Style::default().fg(Color::Green)),
        ])
    };

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center);

    f.render_widget(paragraph, inner);
}

fn draw_workstreams(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Get column widths from app
    let col_status = app.column_widths[COL_IDX_STATUS];
    let col_priority = app.column_widths[COL_IDX_PRIORITY];
    let col_id = app.column_widths[COL_IDX_ID];
    let col_title = app.column_widths[COL_IDX_TITLE];
    let col_pr = app.column_widths[COL_IDX_PR];
    let col_agent = app.column_widths[COL_IDX_AGENT];
    let col_vercel = app.column_widths[COL_IDX_VERCEL];
    let col_time = app.column_widths[COL_IDX_TIME];

    let mut items: Vec<ListItem> = Vec::new();

    // Column header with icons and labels
    let header_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let header_dim = Style::default().fg(Color::DarkGray);
    let sep_style = Style::default().fg(Color::DarkGray);
    let highlight_style = Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD);

    // Helper to get style for a column (highlighted if selected in resize mode)
    let col_style = |idx: usize, base: Style| -> Style {
        if app.resize_mode && app.resize_column_idx == idx {
            highlight_style
        } else {
            base
        }
    };

    let header_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(icons::HEADER_STATUS, col_style(COL_IDX_STATUS, header_style)),
        Span::styled(" │ ", sep_style),
        Span::styled(format!("{:^width$}", "Pri", width = col_priority), col_style(COL_IDX_PRIORITY, header_style)),
        Span::styled(" │ ", sep_style),
        Span::styled(format!("{} ", icons::HEADER_ID), col_style(COL_IDX_ID, header_style)),
        Span::styled(format!("{:<width$}", "ID", width = col_id.saturating_sub(2)), col_style(COL_IDX_ID, header_dim)),
        Span::styled(" │ ", sep_style),
        Span::styled(format!("{:<width$}", "Title", width = col_title), col_style(COL_IDX_TITLE, header_dim)),
        Span::styled(" │ ", sep_style),
        Span::styled(format!("{} ", icons::HEADER_PR), col_style(COL_IDX_PR, header_style)),
        Span::styled(format!("{:<width$}", "PR", width = col_pr.saturating_sub(2)), col_style(COL_IDX_PR, header_dim)),
        Span::styled(" │ ", sep_style),
        Span::styled(format!("{} ", icons::HEADER_AGENT), col_style(COL_IDX_AGENT, header_style)),
        Span::styled(format!("{:<width$}", "Agent", width = col_agent.saturating_sub(2)), col_style(COL_IDX_AGENT, header_dim)),
        Span::styled(" │ ", sep_style),
        Span::styled(format!("{:^width$}", icons::HEADER_VERCEL, width = col_vercel), col_style(COL_IDX_VERCEL, header_style)),
        Span::styled(" │ ", sep_style),
        Span::styled(format!("{} ", icons::HEADER_TIME), col_style(COL_IDX_TIME, header_style)),
        Span::styled(format!("{:>width$}", "Time", width = col_time.saturating_sub(2)), col_style(COL_IDX_TIME, header_dim)),
    ]);
    items.push(ListItem::new(header_line));

    // Separator line
    let sep_width = col_status + col_priority + col_id + col_title + col_pr + col_agent + col_vercel + col_time + 24;
    let separator_line = Line::from(vec![
        Span::raw("  "),
        Span::styled("─".repeat(sep_width), sep_style),
    ]);
    items.push(ListItem::new(separator_line));

    // Render visual items
    for (visual_idx, item) in app.visual_items.iter().enumerate() {
        let is_selected = visual_idx == app.visual_selected;

        match item {
            VisualItem::SectionHeader(status) => {
                let is_collapsed = app.state.collapsed_sections.contains(status);
                let indicator = if is_collapsed { icons::COLLAPSED } else { icons::EXPANDED };

                // Count items in this section
                let count = app.state.workstreams.iter()
                    .filter(|ws| ws.linear_issue.status == *status)
                    .filter(|ws| {
                        app.state.workstreams.iter()
                            .position(|w| w.linear_issue.id == ws.linear_issue.id)
                            .map(|idx| app.filtered_indices.contains(&idx))
                            .unwrap_or(false)
                    })
                    .count();

                let header = format!("{} {} ({})", indicator, status.display_name(), count);
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::DarkGray)
                } else {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                };

                items.push(ListItem::new(Line::from(vec![Span::styled(header, style)])));
            }
            VisualItem::Workstream(ws_idx) => {
                if let Some(ws) = app.state.workstreams.get(*ws_idx) {
                    let row = build_workstream_row(ws, is_selected, &app.column_widths);
                    items.push(row);

                    // If there's a search excerpt for this item, show it expanded below
                    if let Some(search_match) = app.search_excerpts.get(ws_idx) {
                        let excerpt_line = Line::from(vec![
                            Span::raw("       "),
                            Span::styled("▲ ", Style::default().fg(Color::DarkGray)),
                            Span::styled(
                                format!("\"{}\"", &search_match.excerpt),
                                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                            ),
                        ]);
                        items.push(ListItem::new(excerpt_line));
                    }
                }
            }
        }
    }

    let list = List::new(items);
    f.render_widget(list, inner);
}

fn build_workstream_row(ws: &crate::data::Workstream, selected: bool, widths: &[usize; 8]) -> ListItem<'static> {
    let issue = &ws.linear_issue;
    let sep_style = Style::default().fg(Color::DarkGray);

    // Get widths
    let col_priority = widths[COL_IDX_PRIORITY];
    let col_id = widths[COL_IDX_ID];
    let col_title = widths[COL_IDX_TITLE];
    let col_pr = widths[COL_IDX_PR];
    let col_agent = widths[COL_IDX_AGENT];
    let col_vercel = widths[COL_IDX_VERCEL];
    let col_time = widths[COL_IDX_TIME];

    // Status icon (fractional circle)
    let (status_icon, status_style) = linear_status_icon_and_style(issue.status);
    let status_span = Span::styled(status_icon.to_string(), status_style);

    // Priority icon (signal bars)
    let (priority_icon, priority_style) = priority_icon_and_style(issue.priority);
    let priority_span = Span::styled(format!("{:^width$}", priority_icon, width = col_priority), priority_style);

    // Linear ID - colored by status
    let id_text = format!("{:<width$}", issue.identifier, width = col_id);
    let id_span = Span::styled(id_text, linear_status_style(issue.status));

    // Title (truncated with ellipsis)
    let title = if issue.title.chars().count() > col_title {
        let truncated: String = issue.title.chars().take(col_title - 1).collect();
        format!("{}…", truncated)
    } else {
        format!("{:<width$}", issue.title, width = col_title)
    };
    let title_span = Span::raw(title);

    // PR status
    let (pr_text, pr_style) = if let Some(pr) = &ws.github_pr {
        let icon = pr_status_icon(pr.status);
        let text = format!("{} PR#{:<5}", icon, pr.number);
        let text = format!("{:<width$}", text, width = col_pr);
        (text, pr_status_style(pr.status))
    } else {
        (format!("{:<width$}", format!("{} --", icons::AGENT_NONE), width = col_pr), Style::default().fg(Color::DarkGray))
    };
    let pr_span = Span::styled(pr_text, pr_style);

    // Agent status
    let (agent_text, agent_style) = if let Some(session) = &ws.agent_session {
        let icon = agent_status_icon(session.status);
        let label = session.status.label();
        let text = format!("{} {:<5}", icon, label);
        let text = format!("{:<width$}", text, width = col_agent);
        (text, agent_status_style(session.status))
    } else {
        (format!("{:<width$}", format!("{} --", icons::AGENT_NONE), width = col_agent), Style::default().fg(Color::DarkGray))
    };
    let agent_span = Span::styled(agent_text, agent_style);

    // Vercel status
    let (vercel_text, vercel_style) = if let Some(deploy) = &ws.vercel_deployment {
        let icon = vercel_status_icon(deploy.status);
        (format!("{:^width$}", icon, width = col_vercel), vercel_status_style(deploy.status))
    } else {
        (format!("{:^width$}", icons::VERCEL_NONE, width = col_vercel), Style::default().fg(Color::DarkGray))
    };
    let vercel_span = Span::styled(vercel_text, vercel_style);

    // Elapsed time
    let elapsed = if let Some(session) = &ws.agent_session {
        let duration = chrono::Utc::now().signed_duration_since(session.started_at);
        if session.status == AgentStatus::Done {
            "done".to_string()
        } else {
            let mins = duration.num_minutes();
            let secs = duration.num_seconds() % 60;
            if mins > 99 {
                format!("{}m", mins)
            } else {
                format!("{:02}:{:02}", mins, secs)
            }
        }
    } else {
        "".to_string()
    };
    let elapsed_span = Span::styled(
        format!("{:>width$}", elapsed, width = col_time),
        Style::default().fg(Color::DarkGray),
    );

    let line = Line::from(vec![
        Span::raw("  "),
        status_span,
        Span::styled(" │ ", sep_style),
        priority_span,
        Span::styled(" │ ", sep_style),
        id_span,
        Span::styled(" │ ", sep_style),
        title_span,
        Span::styled(" │ ", sep_style),
        pr_span,
        Span::styled(" │ ", sep_style),
        agent_span,
        Span::styled(" │ ", sep_style),
        vercel_span,
        Span::styled(" │ ", sep_style),
        elapsed_span,
    ]);

    let style = if selected {
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    ListItem::new(line).style(style)
}

// Icon and color helpers

fn linear_status_icon_and_style(status: LinearStatus) -> (&'static str, Style) {
    match status {
        LinearStatus::Backlog => (icons::STATUS_BACKLOG, Style::default().fg(Color::DarkGray)),
        LinearStatus::Todo => (icons::STATUS_TODO, Style::default().fg(Color::Cyan)),
        LinearStatus::InProgress => (icons::STATUS_IN_PROGRESS, Style::default().fg(Color::Green)),
        LinearStatus::InReview => (icons::STATUS_IN_REVIEW, Style::default().fg(Color::Yellow)),
        LinearStatus::Done => (icons::STATUS_DONE, Style::default().fg(Color::Magenta)),
        LinearStatus::Canceled => (icons::STATUS_CANCELED, Style::default().fg(Color::DarkGray)),
    }
}

fn linear_status_style(status: LinearStatus) -> Style {
    match status {
        LinearStatus::InProgress => Style::default().fg(Color::Green),
        LinearStatus::InReview => Style::default().fg(Color::Yellow),
        LinearStatus::Todo => Style::default().fg(Color::Cyan),
        LinearStatus::Backlog => Style::default().fg(Color::DarkGray),
        LinearStatus::Done => Style::default().fg(Color::Magenta),
        LinearStatus::Canceled => Style::default().fg(Color::DarkGray),
    }
}

fn priority_icon_and_style(priority: LinearPriority) -> (&'static str, Style) {
    match priority {
        LinearPriority::Urgent => (
            icons::PRIORITY_URGENT,
            Style::default().fg(Color::White).bg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        LinearPriority::High => (
            icons::PRIORITY_HIGH,
            Style::default().fg(Color::Yellow),
        ),
        LinearPriority::Medium => (
            icons::PRIORITY_MEDIUM,
            Style::default().fg(Color::Cyan),
        ),
        LinearPriority::Low => (
            icons::PRIORITY_LOW,
            Style::default().fg(Color::DarkGray),
        ),
        LinearPriority::NoPriority => (
            icons::PRIORITY_NONE,
            Style::default().fg(Color::DarkGray),
        ),
    }
}

fn pr_status_icon(status: GitHubPRStatus) -> &'static str {
    match status {
        GitHubPRStatus::Draft => icons::PR_DRAFT,
        GitHubPRStatus::Open => icons::PR_OPEN,
        GitHubPRStatus::ReviewRequested => icons::PR_REVIEW,
        GitHubPRStatus::ChangesRequested => icons::PR_CHANGES,
        GitHubPRStatus::Approved => icons::PR_APPROVED,
        GitHubPRStatus::Merged => icons::PR_MERGED,
        GitHubPRStatus::Closed => icons::PR_CLOSED,
    }
}

fn pr_status_style(status: GitHubPRStatus) -> Style {
    match status {
        GitHubPRStatus::Draft => Style::default().fg(Color::Blue),
        GitHubPRStatus::Open => Style::default().fg(Color::White),
        GitHubPRStatus::ReviewRequested => Style::default().fg(Color::Cyan),
        GitHubPRStatus::ChangesRequested => Style::default().fg(Color::Yellow),
        GitHubPRStatus::Approved => Style::default().fg(Color::Green),
        GitHubPRStatus::Merged => Style::default().fg(Color::Magenta),
        GitHubPRStatus::Closed => Style::default().fg(Color::DarkGray),
    }
}

fn agent_status_icon(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Running => icons::AGENT_RUNNING,
        AgentStatus::Idle => icons::AGENT_IDLE,
        AgentStatus::WaitingForInput => icons::AGENT_WAITING,
        AgentStatus::Done => icons::AGENT_DONE,
        AgentStatus::Error => icons::AGENT_ERROR,
    }
}

fn agent_status_style(status: AgentStatus) -> Style {
    match status {
        AgentStatus::Running => Style::default().fg(Color::Green),
        AgentStatus::Idle => Style::default().fg(Color::Yellow),
        AgentStatus::WaitingForInput => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        AgentStatus::Done => Style::default().fg(Color::DarkGray),
        AgentStatus::Error => Style::default().fg(Color::Red),
    }
}

fn vercel_status_icon(status: VercelStatus) -> &'static str {
    match status {
        VercelStatus::Ready => icons::VERCEL_READY,
        VercelStatus::Building => icons::VERCEL_BUILDING,
        VercelStatus::Queued => icons::VERCEL_QUEUED,
        VercelStatus::Error => icons::VERCEL_ERROR,
        VercelStatus::Canceled => icons::VERCEL_NONE,
    }
}

fn vercel_status_style(status: VercelStatus) -> Style {
    match status {
        VercelStatus::Ready => Style::default().fg(Color::Green),
        VercelStatus::Building => Style::default().fg(Color::Yellow),
        VercelStatus::Queued => Style::default().fg(Color::Blue),
        VercelStatus::Error => Style::default().fg(Color::Red),
        VercelStatus::Canceled => Style::default().fg(Color::DarkGray),
    }
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let sort_indicator = format!("[Sort: {}]", app.state.sort_mode.label());

    let status = if let Some(err) = &app.error_message {
        Span::styled(err, Style::default().fg(Color::Red))
    } else if app.resize_mode {
        Span::styled(
            format!(
                " RESIZE: {} [{}] | h/l: -/+ width | Tab: next | Esc: done ",
                app.current_resize_column_name(),
                app.column_widths[app.resize_column_idx]
            ),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        )
    } else if app.state.search_mode {
        Span::styled(
            " Type to search | Enter: select | Esc: cancel ",
            Style::default().fg(Color::Yellow),
        )
    } else {
        Span::styled(
            format!(" j/k: nav | /: search | o: links | s: sort {} | R: resize | q: quit | ?: help ", sort_indicator),
            Style::default().fg(Color::DarkGray),
        )
    };

    let paragraph = Paragraph::new(Line::from(status));
    f.render_widget(paragraph, area);
}

fn draw_help_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(65, 80, f.area());

    f.render_widget(Clear, area);

    // Tab bar
    let tab_style_active = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let tab_style_inactive = Style::default().fg(Color::DarkGray);

    let tab_1_style = if app.help_tab == 0 { tab_style_active } else { tab_style_inactive };
    let tab_2_style = if app.help_tab == 1 { tab_style_active } else { tab_style_inactive };

    let tabs = Line::from(vec![
        Span::styled(" [1] Shortcuts ", tab_1_style),
        Span::raw(" │ "),
        Span::styled("[2] Status Legend ", tab_2_style),
    ]);

    let content = if app.help_tab == 0 {
        // Keyboard shortcuts
        vec![
            "",
            "  Navigation",
            "  ──────────",
            "  j/k, ↑/↓     Move up/down",
            "  gg           Go to top",
            "  G            Go to bottom",
            "  Ctrl+d/u     Page down/up",
            "  h/l, ←/→     Collapse/expand section",
            "",
            "  Search",
            "  ──────",
            "  /            Search active work",
            "  Ctrl+/       Search all Linear issues",
            "  Enter        Confirm search",
            "  Esc          Cancel search",
            "",
            "  Actions",
            "  ───────",
            "  Enter        Open Linear issue",
            "  o            Link menu (1-4 to select)",
            "  t            Teleport to Claude session",
            "  s            Open sort menu",
            "  r            Refresh data",
            "",
            "  q            Quit",
            "  ?            Toggle this help",
            "",
        ]
    } else {
        // Status legend
        vec![
            "",
            "  LINEAR ISSUE STATUS",
            "  ───────────────────",
            "  ○  Backlog      Not yet prioritized",
            "  ◔  Todo         Ready to start",
            "  ◑  In Progress  Currently being worked on",
            "  ◕  In Review    Awaiting review/feedback",
            "  ●  Done         Completed",
            "  ⊘  Canceled     No longer needed",
            "",
            "  PRIORITY",
            "  ────────",
            "  ⚠!  Urgent      Highest priority (red bg)",
            "  ▮▮▮ High        High priority",
            "  ▮▮╌ Medium      Medium priority",
            "  ▮╌╌ Low         Low priority",
            "  ╌╌╌ None        No priority set",
            "",
            "  GITHUB PR STATUS",
            "  ────────────────",
            "  󰏫  Draft        Work in progress PR",
            "  󰐊  Open         Ready for review",
            "  󰈈  Review       Review requested",
            "  󰏭  Changes      Changes requested",
            "  󰄬  Approved     Ready to merge",
            "  󰜛  Merged       Successfully merged",
            "  󰅖  Closed       Closed without merging",
            "",
            "  AGENT STATUS",
            "  ────────────",
            "  󰐊  Running      Agent actively working",
            "  󰏤  Idle         Agent paused/waiting",
            "  󰋗  Waiting      Needs your input (!)",
            "  󰄬  Done         Agent finished",
            "  󰅚  Error        Agent encountered error",
            "",
            "  VERCEL DEPLOYMENT",
            "  ─────────────────",
            "  󰄬  Ready        Deployed successfully",
            "  󰑮  Building     Build in progress",
            "  󰔟  Queued       Waiting to build",
            "  󰅚  Error        Deployment failed",
            "",
        ]
    };

    let mut lines = vec![tabs, Line::from("")];
    for line in content {
        lines.push(Line::from(line));
    }
    lines.push(Line::from(Span::styled(
        "  Press 1: Shortcuts | 2: Status Legend | Esc: Close",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" 󰋗 Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn draw_link_menu(f: &mut Frame, app: &App) {
    let area = centered_rect(45, 35, f.area());

    f.render_widget(Clear, area);

    let active_style = Style::default().fg(Color::White);
    let inactive_style = Style::default().fg(Color::DarkGray);

    let lines: Vec<Line> = if let Some(ws) = app.selected_workstream() {
        let has_pr = ws.github_pr.is_some();
        let has_vercel = ws.vercel_deployment.is_some();
        let has_session = ws.agent_session.is_some();

        vec![
            // Linear is always active
            Line::from(Span::styled(
                format!("  1. 󰌷 Linear: {}", ws.linear_issue.identifier),
                active_style,
            )),
            // GitHub PR
            Line::from(Span::styled(
                if let Some(pr) = &ws.github_pr {
                    format!("  2.  GitHub: PR#{}", pr.number)
                } else {
                    "  2.  GitHub: (no PR)".to_string()
                },
                if has_pr { active_style } else { inactive_style },
            )),
            // Vercel
            Line::from(Span::styled(
                if ws.vercel_deployment.is_some() {
                    "  3. ▲ Vercel: open preview".to_string()
                } else {
                    "  3. ▲ Vercel: (no deploy)".to_string()
                },
                if has_vercel { active_style } else { inactive_style },
            )),
            // Agent session
            Line::from(Span::styled(
                if ws.agent_session.is_some() {
                    "  4. 󰚩 Agent: teleport".to_string()
                } else {
                    "  4. 󰚩 Agent: (no session)".to_string()
                },
                if has_session { active_style } else { inactive_style },
            )),
            Line::from(""),
            Line::from(Span::styled("  Press 1-4 or Esc to cancel", active_style)),
        ]
    } else {
        vec![Line::from(Span::styled("  No workstream selected", inactive_style))]
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" 󰌷 Open Link ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );

    f.render_widget(paragraph, area);
}

fn draw_sort_menu(f: &mut Frame, app: &App) {
    use crate::data::SortMode;

    let area = centered_rect(50, 45, f.area());

    f.render_widget(Clear, area);

    let current = app.state.sort_mode;

    let make_item = |idx: usize, mode: SortMode, label: &str| {
        let marker = if current == mode { "●" } else { "○" };
        format!("  {} [{}] {}", marker, idx, label)
    };

    let items = vec![
        "".to_string(),
        make_item(1, SortMode::ByAgentStatus, "Agent Status (waiting first)"),
        make_item(2, SortMode::ByVercelStatus, "Vercel Status (errors first)"),
        make_item(3, SortMode::ByLastUpdated, "Last Updated (recent first)"),
        make_item(4, SortMode::ByPriority, "Priority (urgent first)"),
        make_item(5, SortMode::ByLinearStatus, "Linear Status (default)"),
        make_item(6, SortMode::ByPRActivity, "PR Activity (needs attention)"),
        "".to_string(),
        "  Press 1-6 to select | Esc: Cancel".to_string(),
    ];

    let paragraph = Paragraph::new(items.join("\n"))
        .block(
            Block::default()
                .title(" 󰒺 Sort By ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn draw_filter_menu(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 60, f.area());

    f.render_widget(Clear, area);

    let active_style = Style::default().fg(Color::White);
    let dim_style = Style::default().fg(Color::DarkGray);

    let mut lines: Vec<Line> = Vec::new();

    // Cycle section
    lines.push(Line::from(Span::styled("  CYCLE", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));

    // "All cycles" option
    let all_marker = if app.filter_cycles.is_empty() { "[x]" } else { "[ ]" };
    lines.push(Line::from(Span::styled(
        format!("  [0] All cycles                    {}", all_marker),
        if app.filter_cycles.is_empty() { active_style } else { dim_style }
    )));

    // Individual cycles
    for (idx, cycle) in app.available_cycles.iter().enumerate().take(9) {
        let is_selected = app.filter_cycles.contains(&cycle.id);
        let marker = if is_selected { "[x]" } else { "[ ]" };
        let label = format!("  [{}] Cycle {} ({})            {}", idx + 1, cycle.number, &cycle.name[..cycle.name.len().min(12)], marker);
        lines.push(Line::from(Span::styled(
            label,
            if is_selected { active_style } else { dim_style }
        )));
    }

    lines.push(Line::from(""));

    // Priority section
    lines.push(Line::from(Span::styled("  PRIORITY", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));

    let priorities = [
        ('u', LinearPriority::Urgent, "Urgent"),
        ('h', LinearPriority::High, "High"),
        ('m', LinearPriority::Medium, "Medium"),
        ('l', LinearPriority::Low, "Low"),
        ('n', LinearPriority::NoPriority, "No Priority"),
    ];

    for (key, priority, label) in priorities {
        let is_selected = app.filter_priorities.is_empty() || app.filter_priorities.contains(&priority);
        let marker = if is_selected { "[x]" } else { "[ ]" };
        lines.push(Line::from(Span::styled(
            format!("  [{}] {:<20}          {}", key, label, marker),
            if is_selected { active_style } else { dim_style }
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  [a] All | [c] Clear | Esc: Close", dim_style)));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" 󰈲 Filter ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        );

    f.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
