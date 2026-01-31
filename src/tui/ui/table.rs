//! Issue table rendering - header, workstream rows, and related components.

use super::icons;
use super::layout::{
    compute_column_layout, display_width, pad_to_width, title_column_offset,
    truncate_with_ellipsis, ColumnLayout, PREFIX, SEP,
};
use super::status::{
    agent_status_config, linear_status_config, pr_status_config, priority_config,
    vercel_status_config,
};
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
        .flat_map(|ws| {
            if !ws.agent_sessions.is_empty() {
                ws.agent_sessions.iter().collect::<Vec<_>>()
            } else if let Some(session) = ws.agent_session.as_ref() {
                vec![session]
            } else {
                vec![]
            }
        })
        .filter(|s| s.status == AgentStatus::Running || s.status == AgentStatus::WaitingForInput)
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
    push_header(
        COL_IDX_TITLE,
        "Title".to_string(),
        Alignment::Left,
        header_dim,
    );
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

    // Use cached separator pattern (avoid allocation every frame)
    static SEPARATOR_CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let max_width = 200; // Max reasonable terminal width
    let separator_str = SEPARATOR_CACHE.get_or_init(|| "─".repeat(max_width));
    let separator_slice = &separator_str[..separator_str.len().min(layout.row_body_width * 3)]; // UTF-8 safe slice
    let separator_line = Line::from(vec![
        Span::raw(PREFIX),
        Span::styled(separator_slice.to_string(), sep_style),
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

                // Use cached section counts (O(1) lookup instead of O(n²) calculation)
                let count = app.section_counts.get(section_type).copied().unwrap_or(0);

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
                    base_style.bg(Color::Rgb(30, 40, 60)) // Match row selection color
                } else {
                    base_style
                };

                items.push(ListItem::new(Line::from(vec![Span::styled(
                    header,
                    final_style,
                )])));
            }
            VisualItem::AgentSession {
                ws_idx,
                session_idx,
            } => {
                if let Some(ws) = app.state.workstreams.get(*ws_idx) {
                    let session = ws
                        .agent_sessions
                        .get(*session_idx)
                        .or(ws.agent_session.as_ref());
                    let search_query = if !app.state.search_query.is_empty() {
                        Some(app.state.search_query.as_str())
                    } else {
                        None
                    };
                    let row = build_workstream_row(
                        ws,
                        session,
                        is_selected,
                        &layout,
                        search_query,
                        app.frame_now,
                    );
                    items.push(row);
                }
            }
            VisualItem::Workstream(ws_idx) => {
                if let Some(ws) = app.state.workstreams.get(*ws_idx) {
                    let search_query = if !app.state.search_query.is_empty() {
                        Some(app.state.search_query.as_str())
                    } else {
                        None
                    };
                    let row = build_workstream_row(
                        ws,
                        None,
                        is_selected,
                        &layout,
                        search_query,
                        app.frame_now,
                    );
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

/// Parse OpenClaw surface label into detailed display info
///
/// Returns (surface_type, detail) tuple:
/// - ("TUI", None) for openclaw-tui
/// - ("Discord DM", Some("@username")) for DMs
/// - ("Discord", Some("#channel-name in Guild")) for guild channels
fn parse_surface_detail(
    surface: Option<&str>,
    label: Option<&str>,
) -> Option<(String, Option<String>)> {
    let label = label?;

    // TUI session
    if label == "openclaw-tui" {
        return Some(("TUI".to_string(), None));
    }

    // Discord DM: "username user id:1234567890"
    if label.contains(" user id:") {
        if let Some(username) = label.split(" user id:").next() {
            return Some(("Discord DM".to_string(), Some(format!("@{}", username))));
        }
    }

    // Discord guild channel: "Guild #channel-name channel id:1234567890"
    if let Some(rest) = label.strip_prefix("Guild #") {
        if let Some(channel_end) = rest.find(" channel id:") {
            let channel_name = &rest[..channel_end];
            return Some(("Discord".to_string(), Some(format!("#{}", channel_name))));
        }
    }

    // Raw discord channel ID: "discord:channel:1234567890"
    if let Some(channel_id) = label.strip_prefix("discord:channel:") {
        return Some((
            "Discord".to_string(),
            Some(format!("channel {}", channel_id)),
        ));
    }

    // Fallback: use surface type if available
    if let Some(s) = surface {
        return Some((s.to_string(), Some(label.to_string())));
    }

    None
}

/// Shorten a file path for display
/// Replaces home directory with ~ and truncates if needed
fn shorten_path(path: &str) -> String {
    let shortened = if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        if path.starts_with(home_str.as_ref()) {
            path.replacen(home_str.as_ref(), "~", 1)
        } else {
            path.to_string()
        }
    } else {
        path.to_string()
    };

    // If still too long, show just the last component
    if shortened.len() > 40 {
        std::path::Path::new(path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or(shortened)
    } else {
        shortened
    }
}

/// Remap DarkGray foreground to Gray for visibility on dark backgrounds
fn remap_dark_gray_to_gray(spans: Vec<Span<'static>>) -> Vec<Span<'static>> {
    spans
        .into_iter()
        .map(|span| {
            if span.style.fg == Some(Color::DarkGray) {
                let new_style = span.style.fg(Color::Gray);
                Span::styled(span.content, new_style)
            } else {
                span
            }
        })
        .collect()
}

fn build_workstream_row(
    ws: &crate::data::Workstream,
    session_override: Option<&crate::data::AgentSession>,
    selected: bool,
    layout: &ColumnLayout,
    search_query: Option<&str>,
    frame_now: chrono::DateTime<chrono::Utc>,
) -> ListItem<'static> {
    WorkstreamRowBuilder::new(ws, session_override, layout, search_query, frame_now).build(selected)
}

/// Builder for workstream row UI elements.
struct WorkstreamRowBuilder<'a> {
    ws: &'a crate::data::Workstream,
    session_override: Option<&'a crate::data::AgentSession>,
    layout: &'a ColumnLayout,
    search_query: Option<&'a str>,
    sep_style: Style,
    /// Cached current time (set once per render frame)
    frame_now: chrono::DateTime<chrono::Utc>,
}

impl<'a> WorkstreamRowBuilder<'a> {
    fn new(
        ws: &'a crate::data::Workstream,
        session_override: Option<&'a crate::data::AgentSession>,
        layout: &'a ColumnLayout,
        search_query: Option<&'a str>,
        frame_now: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            ws,
            session_override,
            layout,
            search_query,
            sep_style: Style::default().fg(Color::DarkGray),
            frame_now,
        }
    }

    fn session(&self) -> Option<&crate::data::AgentSession> {
        self.session_override
            .or(self.ws.agent_session.as_ref())
            .or_else(|| self.ws.agent_sessions.first())
    }

    fn build(self, selected: bool) -> ListItem<'static> {
        let main_spans = self.build_spans();
        // When selected, remap DarkGray text to Gray for visibility on DarkGray background
        let main_spans = if selected {
            remap_dark_gray_to_gray(main_spans)
        } else {
            main_spans
        };
        let mut lines = vec![Line::from(main_spans)];

        // Only show expanded agent detail panel when row is selected
        if selected {
            for detail_line in self.agent_detail_lines() {
                // Remap DarkGray to Gray for visibility on selection background
                let adjusted_line = Line::from(remap_dark_gray_to_gray(detail_line.spans));
                lines.push(adjusted_line);
            }
        }
        let style = if selected {
            Style::default()
                .bg(Color::Rgb(30, 40, 60)) // Dark blue-gray for better visibility
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
        let (text, style) = if let Some(session) = self.session() {
            let cfg = agent_status_config(session.status);

            // Type prefix with surface indicator for OpenClaw
            let type_prefix = match session.agent_type {
                crate::data::AgentType::ClaudeCode => "CC".to_string(),
                crate::data::AgentType::OpenClaw => {
                    // Show surface: OC/D (Discord), OC/T (TUI), OC (unknown)
                    let surface_suffix = session
                        .activity
                        .surface
                        .as_ref()
                        .map(|s| match s.as_str() {
                            "discord" => "/D",
                            "webchat" => "/T",
                            _ => "",
                        })
                        .unwrap_or("");
                    format!("OC{}", surface_suffix)
                }
            };

            // For running sessions, show current tool + target (compact activity display)
            let activity_text = if session.status == AgentStatus::Running {
                if let Some(tool) = &session.activity.current_tool {
                    let (icon, ascii) = tool_badge(tool);
                    let target = session
                        .activity
                        .current_target
                        .as_ref()
                        .map(|t| truncate_with_ellipsis(t, width.saturating_sub(10)))
                        .unwrap_or_default();
                    format!("{}{} {}", icon, ascii, target)
                } else {
                    // No tool - check if we've been idle for a while (3+ seconds)
                    let seconds_since_activity =
                        self.frame_now.signed_duration_since(session.last_activity);
                    if seconds_since_activity.num_seconds() > 3 {
                        // Been a while with no tool = effectively idle
                        format!("{}{} Idle", icons::AGENT_IDLE, icons::AGENT_IDLE_ASCII)
                    } else {
                        // Recent activity, no tool = thinking between steps
                        format!("{}{} Thinking", icons::THINKING, icons::THINKING_ASCII)
                    }
                }
            } else {
                // Non-running: show status badge
                let (icon, ascii, label) = agent_badge(session.status);
                format!("{}{} {}", icon, ascii, label)
            };

            let text = format!("{} {}", type_prefix, activity_text);
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
        let elapsed = if let Some(session) = self.session() {
            let duration = self.frame_now.signed_duration_since(session.started_at);
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

    /// Render expanded agent detail panel (multiple lines shown when row is selected)
    ///
    /// Layout mirrors the Issue Details modal for consistency:
    /// - Line 1: Model + Status
    /// - Line 2: Via + Profile (OpenClaw) or Dir + Branch (Claude)
    /// - Line 3: Branch + Tool/Target (activity info)
    /// - Line 4: Stats and subagents
    /// - Line 5: Last prompt (if available)
    /// - Line 6: Error (if any)
    fn agent_detail_lines(&self) -> Vec<Line<'static>> {
        let session = match self.session() {
            Some(s) => s,
            None => return vec![],
        };
        let activity = &session.activity;
        let is_openclaw = session.agent_type == crate::data::AgentType::OpenClaw;

        let indent = title_column_offset(self.layout);
        let indent_str = " ".repeat(indent);
        let border_style = Style::default().fg(Color::DarkGray);
        let label_style = Style::default().fg(Color::DarkGray);
        let value_style = Style::default().fg(Color::White);

        let mut lines = Vec::new();

        // Compute status info (used in multiple places)
        let (status_text, status_color) = if session.status == AgentStatus::Running {
            if activity.current_tool.is_some() {
                ("Running", Color::Cyan)
            } else {
                let seconds_since = self.frame_now.signed_duration_since(session.last_activity);
                if seconds_since.num_seconds() > 3 {
                    ("Idle", Color::DarkGray)
                } else {
                    ("Thinking", Color::Cyan)
                }
            }
        } else {
            match session.status {
                AgentStatus::Idle => ("Idle", Color::DarkGray),
                AgentStatus::WaitingForInput => ("Waiting", Color::Yellow),
                AgentStatus::Done => ("Done", Color::Green),
                AgentStatus::Error => ("Error", Color::Red),
                AgentStatus::Running => ("Running", Color::Cyan),
            }
        };

        // ─── Line 1: Model + Status (matches modal) ───
        let mut header_spans = vec![
            Span::raw(indent_str.clone()),
            Span::styled("├─ ", border_style),
        ];

        if let Some(model) = &activity.model_short {
            header_spans.push(Span::styled("Model: ", label_style));
            header_spans.push(Span::styled(
                model.clone(),
                Style::default().fg(Color::Magenta),
            ));
            header_spans.push(Span::styled(" │ ", border_style));
        }

        header_spans.push(Span::styled("Status: ", label_style));
        header_spans.push(Span::styled(
            status_text.to_string(),
            Style::default().fg(status_color),
        ));

        // Add mode for Claude Code (OpenClaw doesn't have this)
        if !is_openclaw {
            if let Some(mode) = &activity.permission_mode {
                let (mode_icon, mode_label, color) = match mode.as_str() {
                    "plan" => (icons::MODE_PLAN, "plan", Color::Magenta),
                    "acceptEdits" => (icons::MODE_ACCEPT, "accept", Color::Green),
                    "bypassPermissions" => (icons::MODE_YOLO, "yolo", Color::Rgb(255, 140, 0)),
                    "default" => ("", "default", Color::Gray),
                    _ => ("", mode.as_str(), Color::Gray),
                };
                header_spans.push(Span::styled(" │ ", border_style));
                header_spans.push(Span::styled("Mode: ", label_style));
                header_spans.push(Span::styled(
                    format!("{} {}", mode_icon, mode_label),
                    Style::default().fg(color),
                ));
            }
        }

        lines.push(Line::from(header_spans));

        // ─── Line 2: Via + Profile (OpenClaw) or Dir + Branch (Claude) ───
        // This matches the modal's second agent line
        let mut line2_spans = vec![
            Span::raw(indent_str.clone()),
            Span::styled("│  ", border_style),
        ];

        if is_openclaw {
            // OpenClaw: Show Via and Profile (more useful than directory)
            if let Some((surface_type, detail)) = parse_surface_detail(
                activity.surface.as_deref(),
                activity.surface_label.as_deref(),
            ) {
                line2_spans.push(Span::styled("Via: ", label_style));
                line2_spans.push(Span::styled(
                    surface_type,
                    Style::default().fg(if activity.surface.as_deref() == Some("discord") {
                        Color::Rgb(88, 101, 242) // Discord blurple
                    } else {
                        Color::Green // TUI
                    }),
                ));

                if let Some(detail_text) = detail {
                    line2_spans.push(Span::styled(" → ", border_style));
                    line2_spans.push(Span::styled(detail_text, value_style));
                }

                line2_spans.push(Span::styled(" │ ", border_style));
            }

            // Profile
            let profile_text = activity
                .profile
                .clone()
                .unwrap_or_else(|| "default".to_string());
            line2_spans.push(Span::styled("Profile: ", label_style));
            line2_spans.push(Span::styled(
                profile_text,
                Style::default().fg(Color::Green),
            ));
        } else {
            // Claude Code: Show Dir and Branch (matching modal)
            if let Some(dir) = &session.working_directory {
                line2_spans.push(Span::styled("Dir: ", label_style));
                line2_spans.push(Span::styled(shorten_path(dir), value_style));
            }

            if let Some(branch) = &session.git_branch {
                if session.working_directory.is_some() {
                    line2_spans.push(Span::styled(" │ ", border_style));
                }
                line2_spans.push(Span::styled("Branch: ", label_style));
                line2_spans.push(Span::styled(
                    truncate_with_ellipsis(branch, 30),
                    Style::default().fg(Color::Blue),
                ));
            }
        }

        lines.push(Line::from(line2_spans));

        // ─── Line 3: Branch + Tool/Target (activity info) ───
        // For OpenClaw: show branch here since we prioritized Via/Profile above
        // For Claude: show current tool/target
        let mut activity_spans = vec![
            Span::raw(indent_str.clone()),
            Span::styled("│  ", border_style),
        ];

        if is_openclaw {
            // OpenClaw: Branch + current tool/target
            if let Some(branch) = &session.git_branch {
                activity_spans.push(Span::styled("Branch: ", label_style));
                activity_spans.push(Span::styled(
                    truncate_with_ellipsis(branch, 25),
                    Style::default().fg(Color::Blue),
                ));

                if activity.current_tool.is_some() || activity.current_target.is_some() {
                    activity_spans.push(Span::styled(" │ ", border_style));
                }
            }

            if let Some(tool) = &activity.current_tool {
                let (icon, _ascii) = tool_badge(tool);
                activity_spans.push(Span::styled("Tool: ", label_style));
                activity_spans.push(Span::styled(
                    format!("{} {}", icon, tool),
                    Style::default().fg(Color::Cyan),
                ));
                if let Some(target) = &activity.current_target {
                    activity_spans.push(Span::styled(" → ", border_style));
                    activity_spans.push(Span::styled(
                        truncate_with_ellipsis(target, 40),
                        value_style,
                    ));
                }
            }

            // Only add this line if there's content beyond the border prefix
            if activity_spans.len() > 2 {
                lines.push(Line::from(activity_spans));
            }
        } else {
            // Claude Code: Current tool + target
            if let Some(tool) = &activity.current_tool {
                let (icon, _ascii) = tool_badge(tool);
                activity_spans.push(Span::styled("Tool: ", label_style));
                activity_spans.push(Span::styled(
                    format!("{} {}", icon, tool),
                    Style::default().fg(Color::Cyan),
                ));
                if let Some(target) = &activity.current_target {
                    activity_spans.push(Span::styled(" → ", border_style));
                    activity_spans.push(Span::styled(
                        truncate_with_ellipsis(target, 60),
                        value_style,
                    ));
                }
                lines.push(Line::from(activity_spans));
            }
        }

        // ─── Line 3: Stats and subagents ───
        let stats = &activity.stats;
        let total_stats =
            stats.files_read + stats.files_edited + stats.files_written + stats.commands_run;

        if total_stats > 0 || activity.subagent_count > 0 {
            let mut stats_spans = vec![
                Span::raw(indent_str.clone()),
                Span::styled("│  ", border_style),
            ];

            if total_stats > 0 {
                stats_spans.push(Span::styled("Stats: ", label_style));
                if stats.files_read > 0 {
                    stats_spans.push(Span::styled(
                        format!("{} reads", stats.files_read),
                        Style::default().fg(Color::Cyan),
                    ));
                    stats_spans.push(Span::styled(" │ ", border_style));
                }
                if stats.files_edited > 0 {
                    stats_spans.push(Span::styled(
                        format!("{} edits", stats.files_edited),
                        Style::default().fg(Color::Yellow),
                    ));
                    stats_spans.push(Span::styled(" │ ", border_style));
                }
                if stats.files_written > 0 {
                    stats_spans.push(Span::styled(
                        format!("{} writes", stats.files_written),
                        Style::default().fg(Color::Green),
                    ));
                    stats_spans.push(Span::styled(" │ ", border_style));
                }
                if stats.commands_run > 0 {
                    stats_spans.push(Span::styled(
                        format!("{} cmds", stats.commands_run),
                        Style::default().fg(Color::Magenta),
                    ));
                }
            }

            if activity.subagent_count > 0 {
                if total_stats > 0 {
                    stats_spans.push(Span::styled(" │ ", border_style));
                }
                stats_spans.push(Span::styled(
                    format!("{} {} subagents", icons::TOOL_TASK, activity.subagent_count),
                    Style::default().fg(Color::Yellow),
                ));
            }

            lines.push(Line::from(stats_spans));
        }

        // ─── Line 4: Last prompt ───
        if let Some(prompt) = &activity.last_prompt {
            let prompt_spans = vec![
                Span::raw(indent_str.clone()),
                Span::styled("│  ", border_style),
                Span::styled("Prompt: ", label_style),
                Span::styled(
                    format!("\"{}\"", truncate_with_ellipsis(prompt, 70)),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ];
            lines.push(Line::from(prompt_spans));
        }

        // ─── Line 5: Error (if any) ───
        if let Some(error) = &activity.last_error {
            let error_spans = vec![
                Span::raw(indent_str.clone()),
                Span::styled("│  ", border_style),
                Span::styled(
                    format!("{} Error: ", icons::AGENT_ERROR),
                    Style::default().fg(Color::Red),
                ),
                Span::styled(
                    truncate_with_ellipsis(error, 60),
                    Style::default().fg(Color::Red),
                ),
            ];
            lines.push(Line::from(error_spans));
        }

        // ─── Footer line ───
        if !lines.is_empty() {
            // Generate footer border to fit layout width
            let footer_width = self.layout.row_body_width.saturating_sub(indent);
            let footer_border: String = "─".repeat(footer_width.min(60));
            let footer_spans = vec![
                Span::raw(indent_str),
                Span::styled(format!("└{}", footer_border), border_style),
            ];
            lines.push(Line::from(footer_spans));
        }

        lines
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

/// Get icon and ASCII fallback for a tool name
fn tool_badge(tool_name: &str) -> (&'static str, char) {
    match tool_name {
        "Read" => (icons::TOOL_READ, icons::TOOL_READ_ASCII),
        "Edit" => (icons::TOOL_EDIT, icons::TOOL_EDIT_ASCII),
        "Write" => (icons::TOOL_WRITE, icons::TOOL_WRITE_ASCII),
        "Bash" => (icons::TOOL_BASH, icons::TOOL_BASH_ASCII),
        "Grep" => (icons::TOOL_GREP, icons::TOOL_GREP_ASCII),
        "Glob" => (icons::TOOL_GLOB, icons::TOOL_GLOB_ASCII),
        "WebFetch" | "WebSearch" => (icons::TOOL_WEB, icons::TOOL_WEB_ASCII),
        "Task" => (icons::TOOL_TASK, icons::TOOL_TASK_ASCII),
        _ => (icons::THINKING, icons::THINKING_ASCII),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_surface_detail_tui() {
        let result = parse_surface_detail(Some("webchat"), Some("openclaw-tui"));
        assert_eq!(result, Some(("TUI".to_string(), None)));
    }

    #[test]
    fn test_parse_surface_detail_discord_dm() {
        let result = parse_surface_detail(Some("discord"), Some("johndoe user id:1234567890"));
        assert_eq!(
            result,
            Some(("Discord DM".to_string(), Some("@johndoe".to_string())))
        );
    }

    #[test]
    fn test_parse_surface_detail_discord_guild_channel() {
        let result = parse_surface_detail(
            Some("discord"),
            Some("Guild #general channel id:1234567890"),
        );
        assert_eq!(
            result,
            Some(("Discord".to_string(), Some("#general".to_string())))
        );
    }

    #[test]
    fn test_parse_surface_detail_discord_raw_channel() {
        let result = parse_surface_detail(Some("discord"), Some("discord:channel:1234567890"));
        assert_eq!(
            result,
            Some((
                "Discord".to_string(),
                Some("channel 1234567890".to_string())
            ))
        );
    }

    #[test]
    fn test_parse_surface_detail_unknown_surface() {
        let result = parse_surface_detail(Some("slack"), Some("some-label"));
        assert_eq!(
            result,
            Some(("slack".to_string(), Some("some-label".to_string())))
        );
    }

    #[test]
    fn test_parse_surface_detail_none_label() {
        let result = parse_surface_detail(Some("discord"), None);
        assert_eq!(result, None);
    }
}
