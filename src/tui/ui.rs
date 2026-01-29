use super::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

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
                    s.status == crate::data::AgentStatus::Running
                        || s.status == crate::data::AgentStatus::WaitingForInput
                })
                .unwrap_or(false)
        })
        .count();

    let title = if app.state.search_mode {
        format!(" Search: {} ", app.state.search_query)
    } else {
        format!(" Panopticon [{} active] ", active_count)
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

    for (status, workstreams) in &grouped {
        // Section header
        let header = format!("▼ {} ({})", status.display_name(), workstreams.len());
        items.push(ListItem::new(Line::from(vec![Span::styled(
            header,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )])));

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

    // Linear ID
    let id_span = Span::styled(
        format!("{:<10}", issue.identifier),
        Style::default().fg(Color::White),
    );

    // Title (truncated)
    let title = if issue.title.len() > 25 {
        format!("{}...", &issue.title[..22])
    } else {
        format!("{:<25}", issue.title)
    };
    let title_span = Span::raw(title);

    // PR status
    let pr_span = if let Some(pr) = &ws.github_pr {
        Span::styled(
            format!(" {} PR#{:<4} ", pr.status.icon(), pr.number),
            Style::default().fg(match pr.status {
                crate::data::GitHubPRStatus::Merged => Color::Magenta,
                crate::data::GitHubPRStatus::Approved => Color::Green,
                crate::data::GitHubPRStatus::ChangesRequested => Color::Yellow,
                crate::data::GitHubPRStatus::Draft => Color::Blue,
                _ => Color::White,
            }),
        )
    } else {
        Span::styled(" No PR    ", Style::default().fg(Color::DarkGray))
    };

    // Claude status
    let agent_span = if let Some(session) = &ws.agent_session {
        Span::styled(
            format!(" {} {} ", session.status.icon(), session.status.label()),
            Style::default().fg(match session.status {
                crate::data::AgentStatus::Running => Color::Green,
                crate::data::AgentStatus::WaitingForInput => Color::Red,
                crate::data::AgentStatus::Idle => Color::Yellow,
                _ => Color::White,
            }),
        )
    } else {
        Span::styled(" ⚪ none   ", Style::default().fg(Color::DarkGray))
    };

    // Vercel status
    let vercel_span = if let Some(deploy) = &ws.vercel_deployment {
        Span::styled(
            format!(" {} ", deploy.status.icon()),
            Style::default().fg(match deploy.status {
                crate::data::VercelStatus::Ready => Color::Green,
                crate::data::VercelStatus::Error => Color::Red,
                crate::data::VercelStatus::Building => Color::Yellow,
                _ => Color::White,
            }),
        )
    } else {
        Span::styled(" - ", Style::default().fg(Color::DarkGray))
    };

    // Elapsed time
    let elapsed = if let Some(session) = &ws.agent_session {
        let duration = chrono::Utc::now().signed_duration_since(session.started_at);
        if session.status == crate::data::AgentStatus::Done {
            "(done)".to_string()
        } else {
            format!("{:02}:{:02}", duration.num_minutes(), duration.num_seconds() % 60)
        }
    } else {
        "     ".to_string()
    };
    let elapsed_span = Span::styled(elapsed, Style::default().fg(Color::DarkGray));

    let line = Line::from(vec![
        Span::raw("  "),
        id_span,
        Span::raw(" │ "),
        title_span,
        Span::raw(" │"),
        pr_span,
        Span::raw("│"),
        agent_span,
        Span::raw("│"),
        vercel_span,
        Span::raw("│ "),
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

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let status = if let Some(err) = &app.error_message {
        Span::styled(err, Style::default().fg(Color::Red))
    } else if app.state.search_mode {
        Span::styled(
            " Type to search | Enter: select | Esc: cancel ",
            Style::default().fg(Color::Yellow),
        )
    } else {
        Span::styled(
            " j/k: navigate | /: search | Enter: open | o: links | t: teleport | q: quit | ?: help ",
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
        "  o            Link menu (Linear/GitHub/Vercel)",
        "  t            Teleport to Claude session",
        "  p            Preview Claude output",
        "  r            Refresh data",
        "",
        "  q            Quit",
        "  ?            Toggle this help",
        "",
    ];

    let paragraph = Paragraph::new(help_text.join("\n"))
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn draw_link_menu(f: &mut Frame, app: &App) {
    let area = centered_rect(40, 30, f.area());

    f.render_widget(Clear, area);

    let items = if let Some(ws) = app.selected_workstream() {
        vec![
            format!("1. Linear: {}", ws.linear_issue.identifier),
            if let Some(pr) = &ws.github_pr {
                format!("2. GitHub: PR#{}", pr.number)
            } else {
                "2. GitHub: (no PR)".to_string()
            },
            if let Some(deploy) = &ws.vercel_deployment {
                format!("3. Vercel: {}", deploy.status.icon())
            } else {
                "3. Vercel: (no deploy)".to_string()
            },
            if ws.agent_session.is_some() {
                "4. Claude: teleport".to_string()
            } else {
                "4. Claude: (no session)".to_string()
            },
        ]
    } else {
        vec!["No workstream selected".to_string()]
    };

    let paragraph = Paragraph::new(items.join("\n"))
        .block(
            Block::default()
                .title(" Open Link ")
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
