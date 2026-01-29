use super::App;
use crate::data::{AgentStatus, GitHubPRStatus, LinearStatus, VercelStatus};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

// Column widths for consistent alignment
const COL_ID: usize = 10;
const COL_TITLE: usize = 30;
const COL_PR: usize = 12;
const COL_AGENT: usize = 12;
const COL_VERCEL: usize = 5;
const COL_TIME: usize = 6;

// Nerd Font icons (requires a Nerd Font to display properly)
mod icons {
    // PR Status
    pub const PR_DRAFT: &str = "󰏫";      // nf-md-file_document_edit_outline
    pub const PR_OPEN: &str = "󰐊";       // nf-md-play (open/active)
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
        draw_help_popup(f);
    }

    if app.show_link_menu {
        draw_link_menu(f, app);
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

    let title = if app.state.search_mode {
        format!(" 󰍉 Search: {} ", app.state.search_query)
    } else {
        format!(" 󰣖 Panopticon [{} active] ", active_count)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if app.state.search_mode {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    f.render_widget(block, area);
}

fn draw_workstreams(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Group by status
    let grouped = app.state.grouped_workstreams();

    let mut items: Vec<ListItem> = Vec::new();

    // Column header
    let header_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(format!("{:<width$}", "ID", width = COL_ID), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:<width$}", "Title", width = COL_TITLE), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:<width$}", "PR", width = COL_PR), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:<width$}", "Agent", width = COL_AGENT), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:<width$}", "VCL", width = COL_VERCEL), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:<width$}", "Time", width = COL_TIME), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]);
    items.push(ListItem::new(header_line));

    // Separator line
    let sep_width = COL_ID + COL_TITLE + COL_PR + COL_AGENT + COL_VERCEL + COL_TIME + 17; // 17 for separators
    let separator_line = Line::from(vec![
        Span::raw("  "),
        Span::styled("─".repeat(sep_width), Style::default().fg(Color::DarkGray)),
    ]);
    items.push(ListItem::new(separator_line));

    for (status, workstreams) in &grouped {
        let is_collapsed = app.state.collapsed_sections.contains(status);
        let indicator = if is_collapsed { icons::COLLAPSED } else { icons::EXPANDED };

        // Section header
        let header = format!("{} {} ({})", indicator, status.display_name(), workstreams.len());
        items.push(ListItem::new(Line::from(vec![Span::styled(
            header,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )])));

        // Skip items if collapsed
        if is_collapsed {
            continue;
        }

        // Workstream rows
        for ws in workstreams {
            let idx = app
                .state
                .workstreams
                .iter()
                .position(|w| w.linear_issue.id == ws.linear_issue.id)
                .unwrap_or(0);

            // Skip if not in filtered list
            if !app.filtered_indices.contains(&idx) {
                continue;
            }

            let is_selected = app.filtered_indices.get(app.state.selected_index) == Some(&idx);

            let row = build_workstream_row(ws, is_selected);
            items.push(row);
        }
    }

    let list = List::new(items);
    f.render_widget(list, inner);
}

fn build_workstream_row(ws: &crate::data::Workstream, selected: bool) -> ListItem<'static> {
    let issue = &ws.linear_issue;

    // Linear ID - colored by status
    let id_text = format!("{:<width$}", issue.identifier, width = COL_ID);
    let id_span = Span::styled(id_text, linear_status_style(issue.status));

    // Title (truncated with ellipsis)
    let title = if issue.title.chars().count() > COL_TITLE {
        let truncated: String = issue.title.chars().take(COL_TITLE - 1).collect();
        format!("{}…", truncated)
    } else {
        format!("{:<width$}", issue.title, width = COL_TITLE)
    };
    let title_span = Span::raw(title);

    // PR status
    let (pr_text, pr_style) = if let Some(pr) = &ws.github_pr {
        let icon = pr_status_icon(pr.status);
        let text = format!("{} PR#{:<5}", icon, pr.number);
        let text = format!("{:<width$}", text, width = COL_PR);
        (text, pr_status_style(pr.status))
    } else {
        (format!("{:<width$}", format!("{} --", icons::AGENT_NONE), width = COL_PR), Style::default().fg(Color::DarkGray))
    };
    let pr_span = Span::styled(pr_text, pr_style);

    // Agent status
    let (agent_text, agent_style) = if let Some(session) = &ws.agent_session {
        let icon = agent_status_icon(session.status);
        let label = session.status.label();
        let text = format!("{} {:<7}", icon, label);
        let text = format!("{:<width$}", text, width = COL_AGENT);
        (text, agent_status_style(session.status))
    } else {
        (format!("{:<width$}", format!("{} none", icons::AGENT_NONE), width = COL_AGENT), Style::default().fg(Color::DarkGray))
    };
    let agent_span = Span::styled(agent_text, agent_style);

    // Vercel status
    let (vercel_text, vercel_style) = if let Some(deploy) = &ws.vercel_deployment {
        let icon = vercel_status_icon(deploy.status);
        (format!("{:^width$}", icon, width = COL_VERCEL), vercel_status_style(deploy.status))
    } else {
        (format!("{:^width$}", icons::VERCEL_NONE, width = COL_VERCEL), Style::default().fg(Color::DarkGray))
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
        format!("{:>width$}", elapsed, width = COL_TIME),
        Style::default().fg(Color::DarkGray),
    );

    let sep_style = Style::default().fg(Color::DarkGray);
    let line = Line::from(vec![
        Span::raw("  "),
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

// Color helper functions

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
    } else if app.state.search_mode {
        Span::styled(
            " Type to search | Enter: select | Esc: cancel ",
            Style::default().fg(Color::Yellow),
        )
    } else {
        Span::styled(
            format!(" j/k: nav | /: search | o: links | h/l: collapse | s: sort {} | q: quit | ?: help ", sort_indicator),
            Style::default().fg(Color::DarkGray),
        )
    };

    let paragraph = Paragraph::new(Line::from(status));
    f.render_widget(paragraph, area);
}

fn draw_help_popup(f: &mut Frame) {
    let area = centered_rect(60, 70, f.area());

    f.render_widget(Clear, area);

    let help_text = vec![
        "",
        "  Navigation",
        "  ----------",
        "  j/k, ↑/↓     Move up/down",
        "  gg           Go to top",
        "  G            Go to bottom",
        "  Ctrl+d/u     Page down/up",
        "  h/l, ←/→     Collapse/expand section",
        "",
        "  Search",
        "  ------",
        "  /            Search active work",
        "  Ctrl+/       Search all Linear issues",
        "  Enter        Confirm search",
        "  Esc          Cancel search",
        "",
        "  Actions",
        "  -------",
        "  Enter        Open Linear issue",
        "  o            Link menu (1-4 to select)",
        "  t            Teleport to Claude session",
        "  s            Cycle sort mode",
        "  r            Refresh data",
        "",
        "  q            Quit",
        "  ?            Toggle this help",
        "",
    ];

    let paragraph = Paragraph::new(help_text.join("\n"))
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

    let items = if let Some(ws) = app.selected_workstream() {
        vec![
            format!("  1. 󰌷 Linear: {}", ws.linear_issue.identifier),
            if let Some(pr) = &ws.github_pr {
                format!("  2. 󰊤 GitHub: PR#{}", pr.number)
            } else {
                "  2. 󰊤 GitHub: (no PR)".to_string()
            },
            if let Some(_deploy) = &ws.vercel_deployment {
                "  3. 󰔶 Vercel: open preview".to_string()
            } else {
                "  3. 󰔶 Vercel: (no deploy)".to_string()
            },
            if ws.agent_session.is_some() {
                "  4. 󰚩 Agent: teleport".to_string()
            } else {
                "  4. 󰚩 Agent: (no session)".to_string()
            },
            "".to_string(),
            "  Press 1-4 or Esc to cancel".to_string(),
        ]
    } else {
        vec!["  No workstream selected".to_string()]
    };

    let paragraph = Paragraph::new(items.join("\n"))
        .block(
            Block::default()
                .title(" 󰌷 Open Link ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().fg(Color::White));

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
