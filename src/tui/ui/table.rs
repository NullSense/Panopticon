//! Issue table rendering - header, workstream rows, and related components.

use super::icons;
use super::layout::{
    compute_column_layout, display_width, pad_to_width, title_column_offset, truncate_with_ellipsis,
    ColumnLayout, PREFIX, PREFIX_WIDTH, SEP,
};
use super::status::{agent_status_config, linear_status_config, pr_status_config, priority_config, vercel_status_config};
use crate::data::{AgentStatus, SectionType, VisualItem};
use crate::tui::app::{
    COL_IDX_AGENT, COL_IDX_ID, COL_IDX_PR, COL_IDX_PRIORITY, COL_IDX_STATUS, COL_IDX_TIME,
    COL_IDX_TITLE, COL_IDX_VERCEL,
};
use crate::tui::App;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// Draw the application header.
pub fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let active_count = app
        .state
        .workstreams
        .iter()
        .filter(|ws| {
            ws.agent_session
                .as_ref()
                .map(|s| {
                    s.status == AgentStatus::Running || s.status == AgentStatus::WaitingForInput
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

    let text = if app.state.search_mode {
        Line::from(vec![
            Span::styled("󰍉 Search: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                &app.state.search_query,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    } else if app.is_loading {
        let progress_text = if let Some(ref p) = app.refresh_progress {
            if p.total_issues > 0 {
                format!(
                    "{} {} [{}/{}]",
                    app.spinner_char(),
                    p.current_stage,
                    p.completed,
                    p.total_issues
                )
            } else {
                format!("{} {}", app.spinner_char(), p.current_stage)
            }
        } else {
            format!("{} Loading...", app.spinner_char())
        };
        Line::from(vec![
            Span::styled("󰣖 ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Panopticon ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(progress_text, Style::default().fg(Color::Cyan)),
        ])
    } else {
        Line::from(vec![
            Span::styled("󰣖 ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Panopticon ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("[{} active]", active_count),
                Style::default().fg(Color::Green),
            ),
        ])
    };

    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
    f.render_widget(paragraph, inner);
}

/// Draw the workstreams table.
pub fn draw_workstreams(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = compute_column_layout(&app.column_widths, inner.width);

    let mut items: Vec<ListItem> = Vec::new();

    let header_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let header_dim = Style::default().fg(Color::DarkGray);
    let sep_style = Style::default().fg(Color::DarkGray);
    let highlight_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let col_style = |idx: usize, base: Style| -> Style {
        if app.resize_mode() && app.resize_column_idx == idx {
            highlight_style
        } else {
            base
        }
    };

    let mut header_spans: Vec<Span> = Vec::new();
    header_spans.push(Span::raw(PREFIX));
    let mut first_header = true;
    let mut push_header = |idx: usize, text: String, align: Alignment, base: Style| {
        if !layout.is_visible(idx) {
            return;
        }
        if !first_header {
            header_spans.push(Span::styled(SEP, sep_style));
        } else {
            first_header = false;
        }
        let padded = pad_to_width(&text, layout.widths[idx], align);
        header_spans.push(Span::styled(padded, col_style(idx, base)));
    };

    push_header(
        COL_IDX_STATUS,
        header_label(icons::HEADER_STATUS, ""),
        Alignment::Center,
        header_style,
    );
    push_header(
        COL_IDX_PRIORITY,
        header_label("", "Pri"),
        Alignment::Center,
        header_style,
    );
    push_header(
        COL_IDX_ID,
        header_label(icons::HEADER_ID, "ID"),
        Alignment::Left,
        header_dim,
    );
    push_header(COL_IDX_TITLE, "Title".to_string(), Alignment::Left, header_dim);
    push_header(
        COL_IDX_PR,
        header_label(icons::HEADER_PR, "PR"),
        Alignment::Left,
        header_dim,
    );
    push_header(
        COL_IDX_AGENT,
        header_label(icons::HEADER_AGENT, "Agent"),
        Alignment::Left,
        header_dim,
    );
    push_header(
        COL_IDX_VERCEL,
        header_label(icons::HEADER_VERCEL, ""),
        Alignment::Center,
        header_style,
    );
    push_header(
        COL_IDX_TIME,
        header_label(icons::HEADER_TIME, "Time"),
        Alignment::Right,
        header_dim,
    );

    items.push(ListItem::new(Line::from(header_spans)));

    let separator_line = Line::from(vec![
        Span::raw(PREFIX),
        Span::styled("─".repeat(layout.row_body_width), sep_style),
    ]);
    items.push(ListItem::new(separator_line));

    for (visual_idx, item) in app.visual_items.iter().enumerate() {
        let is_selected = visual_idx == app.visual_selected;

        match item {
            VisualItem::SectionHeader(section_type) => {
                let is_collapsed = app.state.collapsed_sections.contains(section_type);
                let indicator = if is_collapsed {
                    icons::COLLAPSED
                } else {
                    icons::EXPANDED
                };

                let count = app
                    .state
                    .workstreams
                    .iter()
                    .filter(|ws| match section_type {
                        SectionType::AgentSessions => ws.agent_session.is_some(),
                        SectionType::Issues => ws.agent_session.is_none(),
                    })
                    .filter(|ws| {
                        app.state
                            .workstreams
                            .iter()
                            .position(|w| w.linear_issue.id == ws.linear_issue.id)
                            .map(|idx| app.filtered_indices.contains(&idx))
                            .unwrap_or(false)
                    })
                    .count();

                let (icon, style) = match section_type {
                    SectionType::AgentSessions => {
                        (icons::HEADER_AGENT, Style::default().fg(Color::Cyan))
                    }
                    SectionType::Issues => (icons::HEADER_ID, Style::default().fg(Color::White)),
                };

                let header = format!(
                    "{} {} {} ({})",
                    indicator,
                    icon,
                    section_type.display_name(),
                    count
                );
                let base_style = style.add_modifier(Modifier::BOLD);
                let final_style = if is_selected {
                    base_style.bg(Color::DarkGray)
                } else {
                    base_style
                };

                items.push(ListItem::new(Line::from(vec![Span::styled(
                    header,
                    final_style,
                )])));
            }
            VisualItem::Workstream(ws_idx) => {
                if let Some(ws) = app.state.workstreams.get(*ws_idx) {
                    let search_query = if !app.state.search_query.is_empty() {
                        Some(app.state.search_query.as_str())
                    } else {
                        None
                    };
                    let row = build_workstream_row(ws, is_selected, &layout, search_query);
                    items.push(row);

                    if let Some(search_match) = app.search_excerpts.get(ws_idx) {
                        let excerpt_line = Line::from(vec![
                            Span::raw("       "),
                            Span::styled("▲ ", Style::default().fg(Color::DarkGray)),
                            Span::styled(
                                format!("\"{}\"", &search_match.excerpt),
                                Style::default()
                                    .fg(Color::DarkGray)
                                    .add_modifier(Modifier::ITALIC),
                            ),
                        ]);
                        items.push(ListItem::new(excerpt_line));
                    }
                }
            }
        }
    }

    let list = List::new(items);
    let mut list_state =
        ratatui::widgets::ListState::default().with_selected(Some(app.visual_selected + 2));

    f.render_stateful_widget(list, inner, &mut list_state);
}

fn header_label(icon: &str, label: &str) -> String {
    match (icon.is_empty(), label.is_empty()) {
        (true, true) => String::new(),
        (true, false) => label.to_string(),
        (false, true) => icon.to_string(),
        (false, false) => format!("{icon} {label}"),
    }
}

fn build_workstream_row(
    ws: &crate::data::Workstream,
    selected: bool,
    layout: &ColumnLayout,
    search_query: Option<&str>,
) -> ListItem<'static> {
    WorkstreamRowBuilder::new(ws, layout, search_query).build(selected)
}

/// Builder for workstream row UI elements.
struct WorkstreamRowBuilder<'a> {
    ws: &'a crate::data::Workstream,
    layout: &'a ColumnLayout,
    search_query: Option<&'a str>,
    sep_style: Style,
}

impl<'a> WorkstreamRowBuilder<'a> {
    fn new(
        ws: &'a crate::data::Workstream,
        layout: &'a ColumnLayout,
        search_query: Option<&'a str>,
    ) -> Self {
        Self {
            ws,
            layout,
            search_query,
            sep_style: Style::default().fg(Color::DarkGray),
        }
    }

    fn build(self, selected: bool) -> ListItem<'static> {
        let mut lines = vec![Line::from(self.build_spans())];
        if let Some(status_line) = self.agent_status_line() {
            lines.push(status_line);
        }
        let style = if selected {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        ListItem::new(lines).style(style)
    }

    fn build_spans(&self) -> Vec<Span<'static>> {
        let (sub_prefix, sub_suffix) = self.sub_issue_indicators();
        let mut spans = vec![Span::raw(PREFIX)];
        let mut first = true;

        if self.layout.is_visible(COL_IDX_STATUS) {
            self.push_column(
                &mut spans,
                &mut first,
                vec![self.status_span(self.layout.widths[COL_IDX_STATUS])],
            );
        }
        if self.layout.is_visible(COL_IDX_PRIORITY) {
            self.push_column(
                &mut spans,
                &mut first,
                vec![self.priority_span(self.layout.widths[COL_IDX_PRIORITY])],
            );
        }
        if self.layout.is_visible(COL_IDX_ID) {
            self.push_column(
                &mut spans,
                &mut first,
                self.id_spans(self.layout.widths[COL_IDX_ID], &sub_prefix),
            );
        }
        if self.layout.is_visible(COL_IDX_TITLE) {
            self.push_column(
                &mut spans,
                &mut first,
                self.title_spans(self.layout.widths[COL_IDX_TITLE], &sub_suffix),
            );
        }
        if self.layout.is_visible(COL_IDX_PR) {
            self.push_column(
                &mut spans,
                &mut first,
                vec![self.pr_span(self.layout.widths[COL_IDX_PR])],
            );
        }
        if self.layout.is_visible(COL_IDX_AGENT) {
            self.push_column(
                &mut spans,
                &mut first,
                vec![self.agent_span(self.layout.widths[COL_IDX_AGENT])],
            );
        }
        if self.layout.is_visible(COL_IDX_VERCEL) {
            self.push_column(
                &mut spans,
                &mut first,
                vec![self.vercel_span(self.layout.widths[COL_IDX_VERCEL])],
            );
        }
        if self.layout.is_visible(COL_IDX_TIME) {
            self.push_column(
                &mut spans,
                &mut first,
                vec![self.elapsed_span(self.layout.widths[COL_IDX_TIME])],
            );
        }

        spans
    }

    fn push_column(
        &self,
        spans: &mut Vec<Span<'static>>,
        first: &mut bool,
        column_spans: Vec<Span<'static>>,
    ) {
        if column_spans.is_empty() {
            return;
        }
        if !*first {
            spans.push(Span::styled(SEP, self.sep_style));
        } else {
            *first = false;
        }
        spans.extend(column_spans);
    }

    fn sub_issue_indicators(&self) -> (String, String) {
        if let Some(parent) = &self.ws.linear_issue.parent {
            ("└ ".to_string(), format!(" ← {}", parent.identifier))
        } else {
            (String::new(), String::new())
        }
    }

    fn status_span(&self, width: usize) -> Span<'static> {
        let cfg = linear_status_config(self.ws.linear_issue.status);
        let text = pad_to_width(cfg.icon, width, Alignment::Center);
        Span::styled(text, cfg.style)
    }

    fn priority_span(&self, width: usize) -> Span<'static> {
        let cfg = priority_config(self.ws.linear_issue.priority);
        let text = pad_to_width(cfg.icon, width, Alignment::Center);
        Span::styled(text, cfg.style)
    }

    fn id_spans(&self, width: usize, sub_prefix: &str) -> Vec<Span<'static>> {
        if width == 0 {
            return Vec::new();
        }
        let issue = &self.ws.linear_issue;
        let prefix_width = display_width(sub_prefix);
        let content_width = width.saturating_sub(prefix_width);
        let id_text = pad_to_width(&issue.identifier, content_width, Alignment::Left);
        let mut spans = Vec::new();
        if !sub_prefix.is_empty() {
            spans.push(Span::styled(
                sub_prefix.to_string(),
                Style::default().fg(Color::DarkGray),
            ));
        }
        spans.extend(highlight_search_matches(
            &id_text,
            self.search_query,
            linear_status_config(issue.status).style,
        ));
        spans
    }

    fn title_spans(&self, width: usize, sub_suffix: &str) -> Vec<Span<'static>> {
        if width == 0 {
            return Vec::new();
        }
        let issue = &self.ws.linear_issue;
        let suffix_width = display_width(sub_suffix);
        let mut suffix = sub_suffix.to_string();
        let title_width = if suffix_width + 1 > width {
            suffix.clear();
            width
        } else {
            width.saturating_sub(suffix_width)
        };

        let title = truncate_with_ellipsis(&issue.title, title_width);
        let title = pad_to_width(&title, title_width, Alignment::Left);
        let mut spans = highlight_search_matches(&title, self.search_query, Style::default());
        if !suffix.is_empty() {
            spans.push(Span::styled(suffix, Style::default().fg(Color::DarkGray)));
        }
        spans
    }

    fn pr_span(&self, width: usize) -> Span<'static> {
        let (text, style) = if let Some(pr) = &self.ws.github_pr {
            let cfg = pr_status_config(pr.status);
            let text = format!("{} PR#{:<5}", cfg.icon, pr.number);
            (pad_to_width(&text, width, Alignment::Left), cfg.style)
        } else {
            (
                pad_to_width(&format!("{} --", icons::AGENT_NONE), width, Alignment::Left),
                Style::default().fg(Color::DarkGray),
            )
        };
        Span::styled(text, style)
    }

    fn agent_span(&self, width: usize) -> Span<'static> {
        let (text, style) = if let Some(session) = &self.ws.agent_session {
            let cfg = agent_status_config(session.status);
            let (icon, ascii, label) = agent_badge(session.status);
            let type_prefix = match session.agent_type {
                crate::data::AgentType::ClaudeCode => "CC",
                crate::data::AgentType::Clawdbot => "MB",
            };
            let text = format!("{} {}{} {}", type_prefix, icon, ascii, label);
            (pad_to_width(&text, width, Alignment::Left), cfg.style)
        } else {
            (
                pad_to_width(&format!("{} --", icons::AGENT_NONE), width, Alignment::Left),
                Style::default().fg(Color::DarkGray),
            )
        };
        Span::styled(text, style)
    }

    fn vercel_span(&self, width: usize) -> Span<'static> {
        let (text, style) = if let Some(deploy) = &self.ws.vercel_deployment {
            let cfg = vercel_status_config(deploy.status);
            (pad_to_width(cfg.icon, width, Alignment::Center), cfg.style)
        } else {
            (
                pad_to_width(icons::VERCEL_NONE, width, Alignment::Center),
                Style::default().fg(Color::DarkGray),
            )
        };
        Span::styled(text, style)
    }

    fn elapsed_span(&self, width: usize) -> Span<'static> {
        if self.ws.stale {
            return Span::styled(
                pad_to_width("STALE", width, Alignment::Right),
                Style::default().fg(Color::Yellow),
            );
        }
        let elapsed = if let Some(session) = &self.ws.agent_session {
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
        Span::styled(
            pad_to_width(&elapsed, width, Alignment::Right),
            Style::default().fg(Color::DarkGray),
        )
    }

    fn agent_status_line(&self) -> Option<Line<'static>> {
        let session = self.ws.agent_session.as_ref()?;
        let output = session.last_output.as_deref()?;
        let snippet = output.lines().find(|line| !line.trim().is_empty())?.trim();
        if snippet.is_empty() {
            return None;
        }

        let (icon, ascii, label) = agent_badge(session.status);
        let type_prefix = match session.agent_type {
            crate::data::AgentType::ClaudeCode => "CC",
            crate::data::AgentType::Clawdbot => "MB",
        };
        let prefix_text = format!("{} {}{} {}", type_prefix, icon, ascii, label);
        let indent = title_column_offset(self.layout);
        let max_width = (self.layout.row_body_width + PREFIX_WIDTH).saturating_sub(indent);
        let snippet_width = max_width
            .saturating_sub(display_width(&prefix_text))
            .saturating_sub(3);
        let snippet = truncate_with_ellipsis(snippet, snippet_width);

        let spans = vec![
            Span::raw(" ".repeat(indent)),
            Span::styled(prefix_text, agent_status_config(session.status).style),
            Span::styled(" • ", Style::default().fg(Color::DarkGray)),
            Span::styled(snippet, Style::default().fg(Color::DarkGray)),
        ];
        Some(Line::from(spans))
    }
}

fn agent_badge(status: AgentStatus) -> (&'static str, char, &'static str) {
    match status {
        AgentStatus::Running => (icons::AGENT_RUNNING, icons::AGENT_RUNNING_ASCII, "RUN"),
        AgentStatus::Idle => (icons::AGENT_IDLE, icons::AGENT_IDLE_ASCII, "IDLE"),
        AgentStatus::WaitingForInput => (icons::AGENT_WAITING, icons::AGENT_WAITING_ASCII, "WAIT"),
        AgentStatus::Done => (icons::AGENT_DONE, icons::AGENT_DONE_ASCII, "DONE"),
        AgentStatus::Error => (icons::AGENT_ERROR, icons::AGENT_ERROR_ASCII, "ERR"),
    }
}

/// Highlight search matches in text with yellow/bold styling.
pub fn highlight_search_matches(
    text: &str,
    query: Option<&str>,
    base_style: Style,
) -> Vec<Span<'static>> {
    let highlight_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    match query {
        Some(q) if !q.is_empty() => {
            let text_lower = text.to_lowercase();
            let query_lower = q.to_lowercase();

            let mut spans = Vec::new();
            let mut last_end = 0;

            for (start, _) in text_lower.match_indices(&query_lower) {
                if start > last_end {
                    spans.push(Span::styled(text[last_end..start].to_string(), base_style));
                }
                let end = start + q.len();
                spans.push(Span::styled(text[start..end].to_string(), highlight_style));
                last_end = end;
            }

            if last_end < text.len() {
                spans.push(Span::styled(text[last_end..].to_string(), base_style));
            }

            if spans.is_empty() {
                vec![Span::styled(text.to_string(), base_style)]
            } else {
                spans
            }
        }
        _ => vec![Span::styled(text.to_string(), base_style)],
    }
}
