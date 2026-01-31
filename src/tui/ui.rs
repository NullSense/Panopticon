use super::keybindings::{generate_footer_hints, generate_keyboard_shortcuts, Mode};
use super::search::FuzzySearch;
use super::App;
use crate::data::{
    AgentStatus, GitHubPRStatus, LinearChildRef, LinearPriority, LinearStatus, SectionType,
    VercelStatus, VisualItem,
};
use pulldown_cmark::{Event, Parser, Tag};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::app::{
    COL_IDX_AGENT, COL_IDX_ID, COL_IDX_PR, COL_IDX_PRIORITY, COL_IDX_STATUS, COL_IDX_TIME,
    COL_IDX_TITLE, COL_IDX_VERCEL, NUM_COLUMNS,
};

// Nerd Font icons
mod icons {
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
}

const PREFIX: &str = "  ";
const PREFIX_WIDTH: usize = 2;
const SEP: &str = " │ ";
const SEP_WIDTH: usize = 3;

const COL_MIN_WIDTHS: [usize; NUM_COLUMNS] = [1, 3, 6, 12, 8, 8, 3, 6];
const COL_HIDE_ORDER: [usize; 6] = [
    COL_IDX_TIME,
    COL_IDX_VERCEL,
    COL_IDX_AGENT,
    COL_IDX_PR,
    COL_IDX_PRIORITY,
    COL_IDX_ID,
];

#[derive(Clone, Copy)]
struct ColumnLayout {
    widths: [usize; NUM_COLUMNS],
    visible: [bool; NUM_COLUMNS],
    row_body_width: usize,
}

impl ColumnLayout {
    fn is_visible(&self, idx: usize) -> bool {
        self.visible[idx] && self.widths[idx] > 0
    }
}

fn compute_column_layout(preferred: &[usize; NUM_COLUMNS], available_width: u16) -> ColumnLayout {
    let available = available_width as usize;
    if available <= PREFIX_WIDTH {
        return ColumnLayout {
            widths: [0; NUM_COLUMNS],
            visible: [false; NUM_COLUMNS],
            row_body_width: 0,
        };
    }

    let mut visible = [true; NUM_COLUMNS];
    let mut min_total = min_total_width(&visible);

    for &idx in &COL_HIDE_ORDER {
        if min_total <= available {
            break;
        }
        if visible[idx] {
            visible[idx] = false;
            min_total = min_total_width(&visible);
        }
    }

    let mut widths = [0; NUM_COLUMNS];
    for i in 0..NUM_COLUMNS {
        if visible[i] {
            widths[i] = COL_MIN_WIDTHS[i];
        }
    }

    if min_total > available {
        let visible_count = visible.iter().filter(|v| **v).count();
        let sep_total = visible_count.saturating_sub(1) * SEP_WIDTH;
        let body_width = available.saturating_sub(PREFIX_WIDTH);
        let other_total: usize = widths
            .iter()
            .enumerate()
            .filter(|(idx, _)| *idx != COL_IDX_TITLE && visible[*idx])
            .map(|(_, w)| *w)
            .sum();
        let title_width = body_width.saturating_sub(sep_total + other_total);
        widths[COL_IDX_TITLE] = title_width;

        let row_body_width = sep_total + other_total + title_width;
        return ColumnLayout {
            widths,
            visible,
            row_body_width,
        };
    }

    let mut remaining = available.saturating_sub(min_total);
    for &idx in &[COL_IDX_ID, COL_IDX_PR, COL_IDX_AGENT, COL_IDX_TITLE] {
        if !visible[idx] {
            continue;
        }
        let preferred_width = preferred[idx].max(COL_MIN_WIDTHS[idx]);
        let cap = preferred_width.saturating_sub(widths[idx]);
        let add = remaining.min(cap);
        widths[idx] += add;
        remaining -= add;
    }
    if remaining > 0 && visible[COL_IDX_TITLE] {
        widths[COL_IDX_TITLE] += remaining;
    }

    let visible_count = visible.iter().filter(|v| **v).count();
    let sep_total = visible_count.saturating_sub(1) * SEP_WIDTH;
    let row_body_width: usize = widths
        .iter()
        .enumerate()
        .filter(|(idx, _)| visible[*idx])
        .map(|(_, w)| *w)
        .sum::<usize>()
        + sep_total;

    ColumnLayout {
        widths,
        visible,
        row_body_width,
    }
}

fn title_column_offset(layout: &ColumnLayout) -> usize {
    let mut width = PREFIX_WIDTH;
    let mut first = true;
    for idx in [COL_IDX_STATUS, COL_IDX_PRIORITY, COL_IDX_ID] {
        if layout.is_visible(idx) {
            if !first {
                width += SEP_WIDTH;
            } else {
                first = false;
            }
            width += layout.widths[idx];
        }
    }
    if layout.is_visible(COL_IDX_TITLE) && !first {
        width += SEP_WIDTH;
    }
    width
}

fn min_total_width(visible: &[bool; NUM_COLUMNS]) -> usize {
    let visible_count = visible.iter().filter(|v| **v).count();
    if visible_count == 0 {
        return 0;
    }
    let sep_total = visible_count.saturating_sub(1) * SEP_WIDTH;
    let widths_total: usize = COL_MIN_WIDTHS
        .iter()
        .enumerate()
        .filter(|(idx, _)| visible[*idx])
        .map(|(_, w)| *w)
        .sum();
    PREFIX_WIDTH + sep_total + widths_total
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn truncate_to_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width > max_width {
            break;
        }
        out.push(ch);
        used += ch_width;
    }
    out
}

fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if display_width(text) <= max_width {
        return text.to_string();
    }
    if max_width == 1 {
        return "…".to_string();
    }
    let truncated = truncate_to_width(text, max_width.saturating_sub(1));
    format!("{truncated}…")
}

fn pad_to_width(text: &str, width: usize, alignment: Alignment) -> String {
    let mut trimmed = truncate_to_width(text, width);
    let current = display_width(&trimmed);
    let pad = width.saturating_sub(current);
    match alignment {
        Alignment::Left => {
            trimmed.push_str(&" ".repeat(pad));
            trimmed
        }
        Alignment::Right => format!("{}{}", " ".repeat(pad), trimmed),
        Alignment::Center => {
            let left = pad / 2;
            let right = pad.saturating_sub(left);
            format!("{}{}{}", " ".repeat(left), trimmed, " ".repeat(right))
        }
    }
}

fn fit_line_to_width<'a>(line: Line<'a>, max_width: usize) -> Line<'a> {
    if max_width == 0 {
        return Line::from(Vec::<Span>::new());
    }

    let Line {
        spans,
        alignment,
        style,
    } = line;
    let mut out: Vec<Span<'a>> = Vec::new();
    let mut used = 0usize;

    for span in spans {
        if used >= max_width {
            break;
        }
        let content = span.content.as_ref();
        let span_width = display_width(content);
        if used + span_width <= max_width {
            used += span_width;
            out.push(span);
        } else {
            let remaining = max_width.saturating_sub(used);
            let truncated = truncate_to_width(content, remaining);
            if !truncated.is_empty() {
                out.push(Span::styled(truncated, span.style));
            }
            break;
        }
    }

    Line {
        spans: out,
        alignment,
        style,
    }
}

fn line_display_width<'a>(line: &Line<'a>) -> usize {
    line.spans
        .iter()
        .map(|span| display_width(span.content.as_ref()))
        .sum()
}

fn pad_line_to_width<'a>(mut line: Line<'a>, width: usize) -> Line<'a> {
    let current = line_display_width(&line);
    if current < width {
        line.spans.push(Span::raw(" ".repeat(width - current)));
    }
    line
}

fn ellipsis_line(width: u16) -> Line<'static> {
    let text = pad_to_width("…", width as usize, Alignment::Center);
    Line::from(Span::styled(text, Style::default().fg(Color::DarkGray)))
}

fn fit_lines_to_area<'a>(lines: Vec<Line<'a>>, inner: Rect, keep_bottom: usize) -> Vec<Line<'a>> {
    let width = inner.width as usize;
    let height = inner.height as usize;
    if height == 0 || width == 0 {
        return Vec::new();
    }

    let mut fitted: Vec<Line<'a>> = lines
        .into_iter()
        .map(|line| fit_line_to_width(line, width))
        .collect();

    if fitted.len() <= height {
        return fitted;
    }

    let keep_bottom = keep_bottom.min(height);
    let top_space = height.saturating_sub(keep_bottom);
    let mut out: Vec<Line<'a>> = Vec::with_capacity(height);

    if top_space > 0 {
        let top_take = top_space.saturating_sub(1);
        if top_take > 0 {
            out.extend(fitted.drain(..top_take));
        }
        out.push(ellipsis_line(inner.width));
    }

    if keep_bottom > 0 {
        let start = fitted.len().saturating_sub(keep_bottom);
        out.extend(fitted.drain(start..));
    }

    if out.is_empty() {
        out.push(ellipsis_line(inner.width));
    }

    out
}

fn render_two_col_line<'a>(
    left: Vec<Span<'a>>,
    right: Vec<Span<'a>>,
    left_width: usize,
    total_width: usize,
    sep_style: Style,
) -> Line<'a> {
    let left_line = pad_line_to_width(fit_line_to_width(Line::from(left), left_width), left_width);
    let right_width = total_width.saturating_sub(left_width + SEP_WIDTH);
    let right_line = fit_line_to_width(Line::from(right), right_width);

    let mut spans = left_line.spans;
    spans.push(Span::styled(SEP, sep_style));
    spans.extend(right_line.spans);
    Line::from(spans)
}

fn header_label(icon: &str, label: &str) -> String {
    match (icon.is_empty(), label.is_empty()) {
        (true, true) => String::new(),
        (true, false) => label.to_string(),
        (false, true) => icon.to_string(),
        (false, false) => format!("{icon} {label}"),
    }
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
    if app.show_help() {
        draw_help_popup(f, app);
    }

    if app.show_link_menu() {
        draw_link_menu(f, app);
        // Draw links popup on top if visible
        if app.show_links_popup() {
            draw_links_popup(f, app);
        }
    }

    if app.show_sort_menu() {
        draw_sort_menu(f, app);
    }

    if app.show_filter_menu() {
        draw_filter_menu(f, app);
    }

    if app.show_description_modal() {
        draw_description_modal(f, app);
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

    // Render centered text inside the block
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

fn draw_workstreams(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = compute_column_layout(&app.column_widths, inner.width);

    let mut items: Vec<ListItem> = Vec::new();

    // Column header with icons and labels
    let header_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let header_dim = Style::default().fg(Color::DarkGray);
    let sep_style = Style::default().fg(Color::DarkGray);
    let highlight_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    // Helper to get style for a column (highlighted if selected in resize mode)
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

    // Separator line
    let separator_line = Line::from(vec![
        Span::raw(PREFIX),
        Span::styled("─".repeat(layout.row_body_width), sep_style),
    ]);
    items.push(ListItem::new(separator_line));

    // Render visual items
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

                // Count items in this section
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

                // Section-specific styling
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

                    // If there's a search excerpt for this item, show it expanded below
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

    // Use ListState for automatic scroll-to-selection
    // Add 2 to account for header + separator lines
    let mut list_state =
        ratatui::widgets::ListState::default().with_selected(Some(app.visual_selected + 2));

    f.render_stateful_widget(list, inner, &mut list_state);
}

fn build_workstream_row(
    ws: &crate::data::Workstream,
    selected: bool,
    layout: &ColumnLayout,
    search_query: Option<&str>,
) -> ListItem<'static> {
    WorkstreamRowBuilder::new(ws, layout, search_query).build(selected)
}

/// Builder for workstream row UI elements
/// Decomposes the row building into smaller, focused methods
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
            // Add agent type prefix: CC for Claude Code, MB for Moltbot
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

// Icon and color helpers

fn agent_badge(status: AgentStatus) -> (&'static str, char, &'static str) {
    match status {
        AgentStatus::Running => (icons::AGENT_RUNNING, icons::AGENT_RUNNING_ASCII, "RUN"),
        AgentStatus::Idle => (icons::AGENT_IDLE, icons::AGENT_IDLE_ASCII, "IDLE"),
        AgentStatus::WaitingForInput => (icons::AGENT_WAITING, icons::AGENT_WAITING_ASCII, "WAIT"),
        AgentStatus::Done => (icons::AGENT_DONE, icons::AGENT_DONE_ASCII, "DONE"),
        AgentStatus::Error => (icons::AGENT_ERROR, icons::AGENT_ERROR_ASCII, "ERR"),
    }
}

/// Highlight search matches in text with yellow/bold styling
fn highlight_search_matches(
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

            // Find all occurrences of the query
            for (start, _) in text_lower.match_indices(&query_lower) {
                // Add text before the match
                if start > last_end {
                    spans.push(Span::styled(text[last_end..start].to_string(), base_style));
                }

                // Add the highlighted match (preserve original case)
                let end = start + q.len();
                spans.push(Span::styled(text[start..end].to_string(), highlight_style));
                last_end = end;
            }

            // Add remaining text after last match
            if last_end < text.len() {
                spans.push(Span::styled(text[last_end..].to_string(), base_style));
            }

            // If no matches found, return original text
            if spans.is_empty() {
                vec![Span::styled(text.to_string(), base_style)]
            } else {
                spans
            }
        }
        _ => vec![Span::styled(text.to_string(), base_style)],
    }
}

/// Unified status configuration - single source of truth for icon and style
pub(crate) struct StatusConfig {
    pub icon: &'static str,
    pub style: Style,
}

/// Trait for types that can provide their display configuration (icon + style)
pub(crate) trait StatusConfigurable {
    fn status_config(&self) -> StatusConfig;
}

impl StatusConfigurable for LinearStatus {
    fn status_config(&self) -> StatusConfig {
        match self {
            LinearStatus::Triage => StatusConfig {
                icon: icons::STATUS_TRIAGE,
                style: Style::default().fg(Color::Rgb(255, 165, 0)), // Orange
            },
            LinearStatus::Backlog => StatusConfig {
                icon: icons::STATUS_BACKLOG,
                style: Style::default().fg(Color::DarkGray),
            },
            LinearStatus::Todo => StatusConfig {
                icon: icons::STATUS_TODO,
                style: Style::default().fg(Color::Cyan),
            },
            LinearStatus::InProgress => StatusConfig {
                icon: icons::STATUS_IN_PROGRESS,
                style: Style::default().fg(Color::Green),
            },
            LinearStatus::InReview => StatusConfig {
                icon: icons::STATUS_IN_REVIEW,
                style: Style::default().fg(Color::Yellow),
            },
            LinearStatus::Done => StatusConfig {
                icon: icons::STATUS_DONE,
                style: Style::default().fg(Color::Magenta),
            },
            LinearStatus::Canceled => StatusConfig {
                icon: icons::STATUS_CANCELED,
                style: Style::default().fg(Color::DarkGray),
            },
            LinearStatus::Duplicate => StatusConfig {
                icon: icons::STATUS_DUPLICATE,
                style: Style::default().fg(Color::DarkGray),
            },
        }
    }
}

impl StatusConfigurable for LinearPriority {
    fn status_config(&self) -> StatusConfig {
        match self {
            LinearPriority::Urgent => StatusConfig {
                icon: icons::PRIORITY_URGENT,
                style: Style::default()
                    .fg(Color::White)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            },
            LinearPriority::High => StatusConfig {
                icon: icons::PRIORITY_HIGH,
                style: Style::default().fg(Color::Yellow),
            },
            LinearPriority::Medium => StatusConfig {
                icon: icons::PRIORITY_MEDIUM,
                style: Style::default().fg(Color::Cyan),
            },
            LinearPriority::Low => StatusConfig {
                icon: icons::PRIORITY_LOW,
                style: Style::default().fg(Color::DarkGray),
            },
            LinearPriority::NoPriority => StatusConfig {
                icon: icons::PRIORITY_NONE,
                style: Style::default().fg(Color::DarkGray),
            },
        }
    }
}

impl StatusConfigurable for GitHubPRStatus {
    fn status_config(&self) -> StatusConfig {
        match self {
            GitHubPRStatus::Draft => StatusConfig {
                icon: icons::PR_DRAFT,
                style: Style::default().fg(Color::Blue),
            },
            GitHubPRStatus::Open => StatusConfig {
                icon: icons::PR_OPEN,
                style: Style::default().fg(Color::White),
            },
            GitHubPRStatus::ReviewRequested => StatusConfig {
                icon: icons::PR_REVIEW,
                style: Style::default().fg(Color::Cyan),
            },
            GitHubPRStatus::ChangesRequested => StatusConfig {
                icon: icons::PR_CHANGES,
                style: Style::default().fg(Color::Yellow),
            },
            GitHubPRStatus::Approved => StatusConfig {
                icon: icons::PR_APPROVED,
                style: Style::default().fg(Color::Green),
            },
            GitHubPRStatus::Merged => StatusConfig {
                icon: icons::PR_MERGED,
                style: Style::default().fg(Color::Magenta),
            },
            GitHubPRStatus::Closed => StatusConfig {
                icon: icons::PR_CLOSED,
                style: Style::default().fg(Color::DarkGray),
            },
        }
    }
}

impl StatusConfigurable for AgentStatus {
    fn status_config(&self) -> StatusConfig {
        match self {
            AgentStatus::Running => StatusConfig {
                icon: icons::AGENT_RUNNING,
                style: Style::default().fg(Color::Cyan),
            },
            AgentStatus::Idle => StatusConfig {
                icon: icons::AGENT_IDLE,
                style: Style::default().fg(Color::DarkGray),
            },
            AgentStatus::WaitingForInput => StatusConfig {
                icon: icons::AGENT_WAITING,
                style: Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            },
            AgentStatus::Done => StatusConfig {
                icon: icons::AGENT_DONE,
                style: Style::default().fg(Color::Green),
            },
            AgentStatus::Error => StatusConfig {
                icon: icons::AGENT_ERROR,
                style: Style::default().fg(Color::Red),
            },
        }
    }
}

impl StatusConfigurable for VercelStatus {
    fn status_config(&self) -> StatusConfig {
        match self {
            VercelStatus::Ready => StatusConfig {
                icon: icons::VERCEL_READY,
                style: Style::default().fg(Color::Green),
            },
            VercelStatus::Building => StatusConfig {
                icon: icons::VERCEL_BUILDING,
                style: Style::default().fg(Color::Yellow),
            },
            VercelStatus::Queued => StatusConfig {
                icon: icons::VERCEL_QUEUED,
                style: Style::default().fg(Color::Blue),
            },
            VercelStatus::Error => StatusConfig {
                icon: icons::VERCEL_ERROR,
                style: Style::default().fg(Color::Red),
            },
            VercelStatus::Canceled => StatusConfig {
                icon: icons::VERCEL_NONE,
                style: Style::default().fg(Color::DarkGray),
            },
        }
    }
}

// Convenience functions to maintain backward compatibility during refactoring
fn linear_status_config(status: LinearStatus) -> StatusConfig {
    status.status_config()
}

fn priority_config(priority: LinearPriority) -> StatusConfig {
    priority.status_config()
}

fn pr_status_config(status: GitHubPRStatus) -> StatusConfig {
    status.status_config()
}

fn agent_status_config(status: AgentStatus) -> StatusConfig {
    status.status_config()
}

fn vercel_status_config(status: VercelStatus) -> StatusConfig {
    status.status_config()
}

/// Generate the status legend for help popup programmatically
fn generate_status_legend() -> Vec<&'static str> {
    // Note: We return static strings for compatibility with the existing help popup code
    // The icons and descriptions are defined in the config functions and data module
    // This serves as documentation and is kept in sync by being generated from centralized data
    vec![
        "",
        "  LINEAR ISSUE STATUS",
        "  ───────────────────",
        // Generated from LinearStatus::all() conceptually
        "  ◇  Triage       Needs triage/categorization",
        "  ○  Backlog      Not yet prioritized",
        "  ◔  Todo         Ready to start",
        "  ◑  In Progress  Currently being worked on",
        "  ◕  In Review    Awaiting review/feedback",
        "  ●  Done         Completed",
        "  ⊘  Canceled     No longer needed",
        "  ◈  Duplicate    Marked as duplicate",
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
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let width = area.width as usize;

    let status = if let Some(err) = &app.error_message {
        Span::styled(err, Style::default().fg(Color::Red))
    } else if app.resize_mode() {
        let text = if width >= 60 {
            format!(
                " RESIZE: {} [{}] | h/l: -/+ width | Tab: next | Esc: done ",
                app.current_resize_column_name(),
                app.column_widths[app.resize_column_idx]
            )
        } else if width >= 40 {
            format!(
                " RESIZE: {} [{}] | h/l Tab Esc ",
                app.current_resize_column_name(),
                app.column_widths[app.resize_column_idx]
            )
        } else {
            format!(" RESIZE: {} ", app.current_resize_column_name())
        };
        Span::styled(
            text,
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )
    } else if app.state.search_mode {
        let text = if width >= 55 {
            " Type to search | ↑/↓: navigate | Enter: confirm | Esc: exit "
        } else if width >= 35 {
            " ↑/↓:nav Enter:confirm Esc:exit "
        } else {
            " Search "
        };
        Span::styled(text, Style::default().fg(Color::Yellow))
    } else {
        // Responsive shortcuts based on available width
        let text = if width >= 110 {
            let sort_indicator = format!("[{}]", app.state.sort_mode.label());
            format!(" j/k: nav | o/Enter: details | l: links | z: fold | /: search | f: filter | s: sort {} | ?: help ", sort_indicator)
        } else if width >= 90 {
            " j/k: nav | o: details | l: links | z: fold | /: search | f: filter | ?: help "
                .to_string()
        } else if width >= 65 {
            " j/k:nav o:details l:links z:fold /:search f:filter ?:help ".to_string()
        } else if width >= 40 {
            " j/k o l z / f s ? ".to_string()
        } else {
            " ? help ".to_string()
        };
        Span::styled(text, Style::default().fg(Color::DarkGray))
    };

    let paragraph = Paragraph::new(Line::from(status));
    f.render_widget(paragraph, area);
}

fn draw_help_popup(f: &mut Frame, app: &App) {
    let area = popup_rect(65, 80, 40, 12, f.area());

    f.render_widget(Clear, area);

    // Tab bar
    let tab_style_active = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let tab_style_inactive = Style::default().fg(Color::DarkGray);

    let tab_1_style = if app.help_tab() == 0 {
        tab_style_active
    } else {
        tab_style_inactive
    };
    let tab_2_style = if app.help_tab() == 1 {
        tab_style_active
    } else {
        tab_style_inactive
    };

    let tabs = Line::from(vec![
        Span::styled(" [1] Shortcuts ", tab_1_style),
        Span::raw(" │ "),
        Span::styled("[2] Status Legend ", tab_2_style),
    ]);

    let content = if app.help_tab() == 0 {
        // Keyboard shortcuts - generated from keybindings registry
        generate_keyboard_shortcuts()
    } else {
        // Status legend - generated programmatically from config functions
        generate_status_legend()
    };

    let mut lines = vec![tabs, Line::from("")];
    for line in content {
        lines.push(Line::from(line));
    }
    lines.push(Line::from(Span::styled(
        "  Press 1: Shortcuts | 2: Status Legend | Esc: Close",
        Style::default().fg(Color::DarkGray),
    )));

    let block = Block::default()
        .title(" 󰋗 Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    let lines = fit_lines_to_area(lines, inner, 1);
    let paragraph = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn draw_link_menu(f: &mut Frame, app: &App) {
    let area = popup_rect(70, 75, 50, 18, f.area());

    f.render_widget(Clear, area);

    let active_style = Style::default().fg(Color::White);
    let inactive_style = Style::default().fg(Color::DarkGray);
    let label_style = Style::default().fg(Color::Cyan);
    let title_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let selected_child_style = Style::default()
        .fg(Color::White)
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD);

    // Search highlighting style
    let search_highlight_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let block = Block::default()
        .title(" 󰌷 Issue Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(area);

    struct TwoColRow {
        left: Vec<Span<'static>>,
        right: Vec<Span<'static>>,
    }

    enum IssueLine {
        Plain(Line<'static>),
        TwoCol(TwoColRow),
    }

    let span_width = |spans: &[Span<'static>]| -> usize {
        spans
            .iter()
            .map(|span| display_width(span.content.as_ref()))
            .sum()
    };

    let lines: Vec<Line> = if let Some(ws) = app.modal_issue() {
        let issue = &ws.linear_issue;

        let mut items: Vec<IssueLine> = Vec::new();
        macro_rules! push_plain {
            ($line:expr) => {
                items.push(IssueLine::Plain($line));
            };
        }

        // Show search input if in search mode
        if app.modal_search_mode {
            let search_style = Style::default().fg(Color::Yellow);
            push_plain!(Line::from(vec![
                Span::styled("  / ", search_style),
                Span::styled(
                    app.modal_search_query.clone(),
                    Style::default().fg(Color::White)
                ),
                Span::styled("█", Style::default().fg(Color::Yellow)), // Cursor
            ]));
            push_plain!(Line::from(""));
        } else if !app.modal_search_query.is_empty() {
            // Show active search filter
            let search_style = Style::default().fg(Color::Cyan);
            push_plain!(Line::from(vec![
                Span::styled("  🔍 ", search_style),
                Span::styled(
                    format!("\"{}\"", &app.modal_search_query),
                    search_highlight_style
                ),
                Span::styled(" (/ to edit, Esc to clear)", inactive_style),
            ]));
            push_plain!(Line::from(""));
        }

        // Show navigation breadcrumb if navigated
        if !app.issue_navigation_stack.is_empty() || app.modal_issue_id.is_some() {
            let nav_style = Style::default().fg(Color::DarkGray);
            let back_style = Style::default().fg(Color::Cyan);
            push_plain!(Line::from(vec![
                Span::styled("  ", nav_style),
                Span::styled("← Esc", back_style),
                Span::styled(
                    format!(
                        " to go back ({} in history)",
                        app.issue_navigation_stack.len()
                    ),
                    nav_style
                ),
            ]));
            push_plain!(Line::from(""));
        }

        // Search query for highlighting all fields
        let search_q = if app.modal_search_query.is_empty() {
            None
        } else {
            Some(app.modal_search_query.as_str())
        };

        // Issue identifier and title (always shown) - with highlighting
        let mut title_line = vec![Span::styled("  ", title_style)];
        title_line.extend(highlight_search_matches(
            &issue.identifier,
            search_q,
            title_style,
        ));
        title_line.push(Span::styled(" ", title_style));
        title_line.extend(highlight_search_matches(
            &truncate_str(&issue.title, 50),
            search_q,
            active_style,
        ));
        push_plain!(Line::from(title_line));
        push_plain!(Line::from(""));

        // Status and Priority with icons
        let status_cfg = linear_status_config(issue.status);
        let priority_cfg = priority_config(issue.priority);
        items.push(IssueLine::TwoCol(TwoColRow {
            left: vec![
                Span::styled(format!("  {} ", status_cfg.icon), status_cfg.style),
                Span::styled("Status: ", label_style),
                Span::styled(issue.status.display_name(), active_style),
            ],
            right: vec![
                Span::styled(format!("{} ", priority_cfg.icon), priority_cfg.style),
                Span::styled("Priority: ", label_style),
                Span::styled(issue.priority.label(), active_style),
            ],
        }));

        // Team and Project with icons - with highlighting
        if issue.team.is_some() || issue.project.is_some() {
            match (&issue.team, &issue.project) {
                (Some(team), Some(project)) => {
                    let mut left = vec![
                        Span::styled(format!("  {} ", icons::ICON_TEAM), label_style),
                        Span::styled("Team: ", label_style),
                    ];
                    left.extend(highlight_search_matches(team, search_q, active_style));
                    let mut right = vec![
                        Span::styled(format!("{} ", icons::ICON_PROJECT), label_style),
                        Span::styled("Project: ", label_style),
                    ];
                    right.extend(highlight_search_matches(project, search_q, active_style));
                    items.push(IssueLine::TwoCol(TwoColRow { left, right }));
                }
                (Some(team), None) => {
                    let mut spans = vec![
                        Span::styled(format!("  {} ", icons::ICON_TEAM), label_style),
                        Span::styled("Team: ", label_style),
                    ];
                    spans.extend(highlight_search_matches(team, search_q, active_style));
                    push_plain!(Line::from(spans));
                }
                (None, Some(project)) => {
                    let mut spans = vec![
                        Span::styled("  ", label_style),
                        Span::styled(format!("{} ", icons::ICON_PROJECT), label_style),
                        Span::styled("Project: ", label_style),
                    ];
                    spans.extend(highlight_search_matches(project, search_q, active_style));
                    push_plain!(Line::from(spans));
                }
                (None, None) => {}
            }
        }

        // Cycle with icon - with highlighting
        if let Some(cycle) = &issue.cycle {
            let mut spans = vec![
                Span::styled(format!("  {} ", icons::ICON_CYCLE), label_style),
                Span::styled("Cycle: ", label_style),
            ];
            spans.extend(highlight_search_matches(
                &cycle.name,
                search_q,
                active_style,
            ));
            spans.push(Span::styled(format!(" ({})", cycle.number), active_style));
            push_plain!(Line::from(spans));
        }

        // Estimate with icon
        if let Some(est) = issue.estimate {
            push_plain!(Line::from(vec![
                Span::styled(format!("  {} ", icons::ICON_ESTIMATE), label_style),
                Span::styled("Estimate: ", label_style),
                Span::styled(format!("{} points", est), active_style),
            ]));
        }

        // Labels with icon - with highlighting
        if !issue.labels.is_empty() {
            let mut spans = vec![
                Span::styled(
                    format!("  {} ", icons::ICON_LABELS),
                    Style::default().fg(Color::Magenta),
                ),
                Span::styled("Labels: ", label_style),
            ];
            let label_style_base = Style::default().fg(Color::Magenta);
            for (i, label) in issue.labels.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(", ", label_style_base));
                }
                spans.extend(highlight_search_matches(
                    &label.name,
                    search_q,
                    label_style_base,
                ));
            }
            push_plain!(Line::from(spans));
        }

        // Dates with icons
        items.push(IssueLine::TwoCol(TwoColRow {
            left: vec![
                Span::styled(format!("  {} ", icons::ICON_CREATED), inactive_style),
                Span::styled("Created: ", label_style),
                Span::styled(
                    issue.created_at.format("%Y-%m-%d").to_string(),
                    inactive_style,
                ),
            ],
            right: vec![
                Span::styled(format!("{} ", icons::ICON_UPDATED), inactive_style),
                Span::styled("Updated: ", label_style),
                Span::styled(
                    issue.updated_at.format("%Y-%m-%d %H:%M").to_string(),
                    inactive_style,
                ),
            ],
        }));

        // Parent issue - selectable with j/k, highlighted when selected
        if let Some(parent) = &issue.parent {
            push_plain!(Line::from(""));
            let is_selected = app.parent_selected;
            let row_style = if is_selected {
                selected_child_style
            } else {
                Style::default()
            };
            let label_row_style = if is_selected {
                selected_child_style
            } else {
                label_style
            };
            let id_style = if is_selected {
                selected_child_style
            } else {
                Style::default().fg(Color::Yellow)
            };
            let title_style = if is_selected {
                selected_child_style
            } else {
                active_style
            };

            let mut parent_spans = vec![
                Span::styled(if is_selected { " >> " } else { "  " }, row_style),
                Span::styled(
                    format!("{} ", icons::ICON_PARENT),
                    if is_selected {
                        row_style
                    } else {
                        Style::default().fg(Color::Blue)
                    },
                ),
                Span::styled("Parent: ", label_row_style),
            ];
            parent_spans.extend(highlight_search_matches(
                &parent.identifier,
                search_q,
                id_style,
            ));
            parent_spans.push(Span::styled(" ", row_style));
            parent_spans.extend(highlight_search_matches(
                &truncate_str(&parent.title, 40),
                search_q,
                title_style,
            ));
            push_plain!(Line::from(parent_spans));
        }

        // Sub-issues (children) - with j/k navigation, scrolling, sort, and search filtering
        if !issue.children.is_empty() {
            push_plain!(Line::from(""));

            // Filter children based on modal search query (searches all fields)
            let filtered_children: Vec<&LinearChildRef> = if app.modal_search_query.is_empty() {
                issue.children.iter().collect()
            } else {
                let mut fuzzy = FuzzySearch::new();
                issue
                    .children
                    .iter()
                    .filter(|child| {
                        // Search identifier, title, status name, and priority label
                        let text = format!(
                            "{} {} {} {}",
                            child.identifier,
                            child.title,
                            child.status.display_name(),
                            child.priority.label()
                        );
                        fuzzy
                            .multi_term_match(&app.modal_search_query, &text)
                            .is_some()
                    })
                    .collect()
            };

            // Sort filtered children according to current sort mode
            let sorted_children = sort_children(filtered_children, app.state.sort_mode);
            let total_children = sorted_children.len();
            let visible_height: usize = inner.height.saturating_sub(12) as usize;
            let visible_height = visible_height.clamp(3, 10);
            let scroll = app.sub_issues_scroll;

            // Show header with count (filtered vs total)
            let count_text = if app.modal_search_query.is_empty() {
                format!("Sub-Issues ({}):", total_children)
            } else {
                format!("Sub-Issues ({}/{}):", total_children, issue.children.len())
            };

            let hint = if app.selected_child_idx.is_some() {
                " (j/k: nav, Enter: open in modal)"
            } else {
                " (j/k to select)"
            };
            push_plain!(Line::from(vec![
                Span::styled(
                    format!("  {} ", icons::ICON_CHILDREN),
                    Style::default().fg(Color::Green)
                ),
                Span::styled(count_text, label_style),
                Span::styled(hint, inactive_style),
            ]));

            // Show "No matches" if filter returns empty
            if sorted_children.is_empty() && !app.modal_search_query.is_empty() {
                push_plain!(Line::from(vec![
                    Span::styled("    ", inactive_style),
                    Span::styled(
                        "No matching sub-issues",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC)
                    ),
                ]));
            } else {
                // Show scroll-up indicator if scrolled
                if scroll > 0 {
                    push_plain!(Line::from(vec![
                        Span::styled("    ", inactive_style),
                        Span::styled(
                            format!("↑ {} more above", scroll),
                            Style::default().fg(Color::Cyan)
                        ),
                    ]));
                }

                // Prepare search query for highlighting
                let search_query = if app.modal_search_query.is_empty() {
                    None
                } else {
                    Some(app.modal_search_query.as_str())
                };

                // Show visible children with scrolling and highlighting
                for (display_i, child) in sorted_children
                    .iter()
                    .skip(scroll)
                    .take(visible_height)
                    .enumerate()
                {
                    let is_selected = app.selected_child_idx == Some(scroll + display_i);
                    let child_status_cfg = linear_status_config(child.status);
                    let child_priority_cfg = priority_config(child.priority);

                    let row_style = if is_selected {
                        selected_child_style
                    } else {
                        Style::default()
                    };
                    let id_style = if is_selected {
                        selected_child_style
                    } else {
                        Style::default().fg(Color::Yellow)
                    };
                    let title_row_style = if is_selected {
                        selected_child_style
                    } else {
                        active_style
                    };

                    // Build line with highlighted spans for identifier and title
                    let mut spans = vec![
                        Span::styled(if is_selected { " >> " } else { "    " }, row_style),
                        Span::styled(
                            format!("{} ", child_status_cfg.icon),
                            if is_selected {
                                row_style
                            } else {
                                child_status_cfg.style
                            },
                        ),
                        Span::styled(
                            format!("{} ", child_priority_cfg.icon),
                            if is_selected {
                                row_style
                            } else {
                                child_priority_cfg.style
                            },
                        ),
                    ];

                    // Add highlighted identifier
                    spans.extend(highlight_search_matches(
                        &child.identifier,
                        search_query,
                        id_style,
                    ));
                    spans.push(Span::styled(" ", row_style));

                    // Add highlighted title
                    let truncated_title = truncate_str(&child.title, 35);
                    spans.extend(highlight_search_matches(
                        &truncated_title,
                        search_query,
                        title_row_style,
                    ));

                    push_plain!(Line::from(spans));
                }

                // Show scroll-down indicator if more below
                let visible_end = scroll + visible_height;
                if visible_end < total_children {
                    push_plain!(Line::from(vec![
                        Span::styled("    ", inactive_style),
                        Span::styled(
                            format!("↓ {} more below", total_children - visible_end),
                            Style::default().fg(Color::Cyan)
                        ),
                    ]));
                }
            }
        }

        // Attachments (documents) - with highlighting
        if !issue.attachments.is_empty() {
            push_plain!(Line::from(""));
            push_plain!(Line::from(vec![
                Span::styled(format!("  {} ", icons::ICON_DOCUMENT), label_style),
                Span::styled(
                    format!("Documents ({}):", issue.attachments.len()),
                    label_style
                ),
            ]));
            for (i, attachment) in issue.attachments.iter().take(5).enumerate() {
                let source_icon = match attachment.source_type.as_deref() {
                    Some("figma") => "󰡁",
                    Some("notion") => "󰈙",
                    Some("github") => "",
                    Some("slack") => "󰒱",
                    _ => "󰈙",
                };
                let mut att_spans =
                    vec![Span::styled(format!("    {} ", source_icon), active_style)];
                att_spans.extend(highlight_search_matches(
                    &truncate_str(&attachment.title, 40),
                    search_q,
                    active_style,
                ));
                att_spans.push(Span::styled(format!(" [d{}]", i + 1), inactive_style));
                push_plain!(Line::from(att_spans));
            }
            if issue.attachments.len() > 5 {
                push_plain!(Line::from(Span::styled(
                    format!("    ... and {} more", issue.attachments.len() - 5),
                    inactive_style,
                )));
            }
        }

        // Description (truncated) - press 'd' for full view - with highlighting
        if let Some(desc) = &issue.description {
            push_plain!(Line::from(""));
            push_plain!(Line::from(vec![
                Span::styled(format!("  {} ", icons::ICON_DOCUMENT), label_style),
                Span::styled("Description ", label_style),
                Span::styled("[d] full view", inactive_style),
            ]));
            // Wrap description to ~60 chars per line, max 3 lines - with highlighting
            let desc_clean = desc.replace('\n', " ").replace("  ", " ");
            for (i, chunk) in desc_clean
                .chars()
                .collect::<Vec<_>>()
                .chunks(58)
                .enumerate()
            {
                if i >= 3 {
                    push_plain!(Line::from(Span::styled("    ...", inactive_style)));
                    break;
                }
                let text: String = chunk.iter().collect();
                let trimmed = text.trim();
                let mut desc_spans = vec![Span::styled("    ", inactive_style)];
                desc_spans.extend(highlight_search_matches(trimmed, search_q, inactive_style));
                push_plain!(Line::from(desc_spans));
            }
        }

        // Footer hint - generated from keybindings registry
        push_plain!(Line::from(""));
        push_plain!(Line::from(Span::styled(
            generate_footer_hints(Mode::LinkMenu),
            inactive_style,
        )));

        let total_width = inner.width as usize;
        let mut left_max = 0usize;
        for item in &items {
            if let IssueLine::TwoCol(row) = item {
                left_max = left_max.max(span_width(&row.left));
            }
        }
        let min_right = 18usize.min(total_width);
        let left_width_raw = if total_width > SEP_WIDTH + min_right {
            left_max.min(total_width - SEP_WIDTH - min_right)
        } else {
            total_width.saturating_sub(SEP_WIDTH) / 2
        };
        let max_left = total_width.saturating_sub(SEP_WIDTH);
        let left_width = if max_left < 8 {
            max_left
        } else {
            left_width_raw.clamp(8, max_left)
        };

        let mut lines: Vec<Line> = Vec::new();
        for item in items {
            match item {
                IssueLine::Plain(line) => lines.push(line),
                IssueLine::TwoCol(row) => lines.push(render_two_col_line(
                    row.left,
                    row.right,
                    left_width,
                    total_width,
                    inactive_style,
                )),
            }
        }

        lines
    } else {
        vec![Line::from(Span::styled(
            "  No workstream selected",
            inactive_style,
        ))]
    };

    let lines = fit_lines_to_area(lines, inner, 1);
    let paragraph = Paragraph::new(lines).block(block);

    f.render_widget(paragraph, area);
}

/// Draw the quick links popup (overlays issue details)
fn draw_links_popup(f: &mut Frame, app: &App) {
    // Small centered popup
    let area = popup_rect(40, 30, 30, 8, f.area());

    f.render_widget(Clear, area);

    let active_style = Style::default().fg(Color::White);
    let inactive_style = Style::default().fg(Color::DarkGray);

    let lines: Vec<Line> = if let Some(ws) = app.modal_issue() {
        let issue = &ws.linear_issue;
        let has_linear = !issue.url.is_empty();
        let has_pr = ws.github_pr.is_some();
        let has_vercel = ws.vercel_deployment.is_some();
        let has_session = ws.agent_session.is_some();

        vec![
            Line::from(""),
            Line::from(Span::styled(
                if has_linear {
                    format!("  [1] 󰌷 Linear: {}", issue.identifier)
                } else {
                    "  [1] 󰌷 Linear: (unlinked)".to_string()
                },
                if has_linear {
                    active_style
                } else {
                    inactive_style
                },
            )),
            Line::from(Span::styled(
                if let Some(pr) = &ws.github_pr {
                    format!("  [2]  GitHub: PR#{}", pr.number)
                } else {
                    "  [2]  GitHub: (no PR)".to_string()
                },
                if has_pr { active_style } else { inactive_style },
            )),
            Line::from(Span::styled(
                if ws.vercel_deployment.is_some() {
                    "  [3] ▲ Vercel: preview".to_string()
                } else {
                    "  [3] ▲ Vercel: (no deploy)".to_string()
                },
                if has_vercel {
                    active_style
                } else {
                    inactive_style
                },
            )),
            Line::from(Span::styled(
                if ws.agent_session.is_some() {
                    "  [4] 󰚩 Agent: teleport".to_string()
                } else {
                    "  [4] 󰚩 Agent: (no session)".to_string()
                },
                if has_session {
                    active_style
                } else {
                    inactive_style
                },
            )),
            Line::from(""),
            Line::from(Span::styled("  1-4: open | Esc: close", inactive_style)),
        ]
    } else {
        vec![Line::from(Span::styled("  No issue", inactive_style))]
    };

    let block = Block::default()
        .title(" 󰌷 Open Links ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    let lines = fit_lines_to_area(lines, inner, 1);
    let paragraph = Paragraph::new(lines).block(block);

    f.render_widget(paragraph, area);
}

fn draw_description_modal(f: &mut Frame, app: &App) {
    let area = popup_rect(80, 80, 50, 12, f.area());

    f.render_widget(Clear, area);

    let title_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(Color::White);
    let dim_style = Style::default().fg(Color::DarkGray);

    let block = Block::default()
        .title(" 󰈚 Description ")
        .title_bottom(Line::from(" j/k: scroll | Esc: close ").centered())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    let content_width = inner.width.saturating_sub(2) as usize;

    let mut scroll_line = app.description_scroll + 1;
    let lines: Vec<Line> = if let Some(ws) = app.selected_workstream() {
        let issue = &ws.linear_issue;
        let mut lines = vec![
            Line::from(vec![
                Span::styled(format!("  {} ", issue.identifier), title_style),
                Span::styled(&issue.title, text_style),
            ]),
            Line::from(""),
        ];

        if let Some(desc) = &issue.description {
            // Parse markdown and convert to styled lines
            let markdown_lines = parse_markdown_to_lines(desc, content_width);

            // Apply scroll offset
            let max_scroll = markdown_lines.len().saturating_sub(inner.height as usize);
            let scroll = app.description_scroll.min(max_scroll);
            scroll_line = scroll + 1;
            let visible_lines: Vec<Line> = markdown_lines.into_iter().skip(scroll).collect();

            lines.extend(visible_lines);
        } else {
            lines.push(Line::from(Span::styled("  No description", dim_style)));
        }

        lines
    } else {
        vec![Line::from(Span::styled(
            "  No workstream selected",
            dim_style,
        ))]
    };

    let scroll_hint = format!("[line {}]", scroll_line);
    let title = format!(" {} Description  {} ", icons::ICON_DOCUMENT, scroll_hint);
    let block = block.title(title);
    let lines = fit_lines_to_area(lines, inner, 0);
    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// Parse markdown text into styled ratatui Lines
fn parse_markdown_to_lines(markdown: &str, max_width: usize) -> Vec<Line<'static>> {
    let parser = Parser::new(markdown);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();

    // Style stacks
    let mut is_bold = false;
    let mut is_italic = false;
    let mut in_heading = false;
    let mut in_code_block = false;
    let mut in_blockquote = false;
    let mut list_depth = 0usize;
    let mut in_link = false;

    let text_style = Style::default().fg(Color::White);
    let bold_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let italic_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::ITALIC);
    let bold_italic_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD | Modifier::ITALIC);
    let code_style = Style::default().fg(Color::Gray);
    let code_block_style = Style::default().fg(Color::Gray);
    let heading_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let link_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::UNDERLINED);
    let quote_style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);

    let flush_line =
        |spans: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>, indent: &str| {
            if !spans.is_empty() {
                let mut line_spans = vec![Span::raw(indent.to_string())];
                line_spans.append(spans);
                lines.push(Line::from(line_spans));
            }
        };

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading(_, _, _) => {
                    flush_line(&mut current_spans, &mut lines, "  ");
                    in_heading = true;
                }
                Tag::Paragraph => {
                    flush_line(&mut current_spans, &mut lines, "  ");
                }
                Tag::Strong => is_bold = true,
                Tag::Emphasis => is_italic = true,
                Tag::CodeBlock(_) => {
                    flush_line(&mut current_spans, &mut lines, "  ");
                    in_code_block = true;
                }
                Tag::BlockQuote => {
                    flush_line(&mut current_spans, &mut lines, "  ");
                    in_blockquote = true;
                }
                Tag::List(_) => {
                    flush_line(&mut current_spans, &mut lines, "  ");
                    list_depth += 1;
                }
                Tag::Item => {
                    flush_line(&mut current_spans, &mut lines, "  ");
                    let indent = "  ".repeat(list_depth);
                    current_spans.push(Span::raw(format!("{}• ", indent)));
                }
                Tag::Link(_, _, _) => {
                    in_link = true;
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                Tag::Heading(_, _, _) => {
                    flush_line(&mut current_spans, &mut lines, "  ");
                    lines.push(Line::from(""));
                    in_heading = false;
                }
                Tag::Paragraph => {
                    flush_line(&mut current_spans, &mut lines, "  ");
                    lines.push(Line::from(""));
                }
                Tag::Strong => is_bold = false,
                Tag::Emphasis => is_italic = false,
                Tag::CodeBlock(_) => {
                    flush_line(&mut current_spans, &mut lines, "    ");
                    lines.push(Line::from(""));
                    in_code_block = false;
                }
                Tag::BlockQuote => {
                    flush_line(&mut current_spans, &mut lines, "  ");
                    in_blockquote = false;
                }
                Tag::List(_) => {
                    list_depth = list_depth.saturating_sub(1);
                }
                Tag::Item => {
                    flush_line(&mut current_spans, &mut lines, "  ");
                }
                Tag::Link(_, _, _) => {
                    in_link = false;
                }
                _ => {}
            },
            Event::Text(text) => {
                let style = if in_heading {
                    heading_style
                } else if in_code_block {
                    code_block_style
                } else if in_blockquote {
                    quote_style
                } else if in_link {
                    link_style
                } else if is_bold && is_italic {
                    bold_italic_style
                } else if is_bold {
                    bold_style
                } else if is_italic {
                    italic_style
                } else {
                    text_style
                };

                let text_str = text.to_string();

                // Handle code blocks: split by newlines
                if in_code_block {
                    for line in text_str.lines() {
                        current_spans.push(Span::styled(line.to_string(), style));
                        flush_line(&mut current_spans, &mut lines, "    ");
                    }
                } else {
                    // Word wrap for regular text
                    let words: Vec<&str> = text_str.split_whitespace().collect();
                    let mut current_line_len = current_spans
                        .iter()
                        .map(|s| display_width(s.content.as_ref()))
                        .sum::<usize>();

                    for word in words {
                        let word_width = display_width(word);
                        if current_line_len + word_width + 1 > max_width && current_line_len > 0 {
                            flush_line(&mut current_spans, &mut lines, "  ");
                            current_line_len = 0;
                        }

                        if current_line_len > 0 {
                            current_spans.push(Span::raw(" ".to_string()));
                            current_line_len += 1;
                        }
                        current_spans.push(Span::styled(word.to_string(), style));
                        current_line_len += word_width;
                    }
                }
            }
            Event::Code(code) => {
                current_spans.push(Span::styled(format!("`{}`", code), code_style));
            }
            Event::SoftBreak | Event::HardBreak => {
                flush_line(&mut current_spans, &mut lines, "  ");
            }
            _ => {}
        }
    }

    // Flush any remaining content
    flush_line(&mut current_spans, &mut lines, "  ");

    // Ensure we have at least one line
    if lines.is_empty() {
        lines.push(Line::from("  (empty)"));
    }

    lines
}

/// Sort children by the current sort mode (inheriting from main view)
/// Sort children list (pre-filtered or full)
// Use shared sorting logic from data module
use crate::data::sort_children;

/// Truncate a string to max length, adding ellipsis if needed
fn truncate_str(s: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    if display_width(s) <= max_len {
        return s.to_string();
    }
    if max_len <= 3 {
        return truncate_to_width(s, max_len);
    }
    let truncated = truncate_to_width(s, max_len.saturating_sub(3));
    format!("{truncated}...")
}

fn draw_sort_menu(f: &mut Frame, app: &App) {
    use crate::data::SortMode;

    let area = popup_rect(50, 45, 34, 12, f.area());

    f.render_widget(Clear, area);

    let current = app.state.sort_mode;
    let active_style = Style::default().fg(Color::White);
    let dim_style = Style::default().fg(Color::DarkGray);
    let icon_style = Style::default().fg(Color::Cyan);

    // Sort options with icons
    let options: Vec<(usize, SortMode, &str, &str)> = vec![
        (
            1,
            SortMode::ByAgentStatus,
            "󰚩",
            "Agent Status (waiting first)",
        ),
        (
            2,
            SortMode::ByVercelStatus,
            "▲",
            "Vercel Status (errors first)",
        ),
        (
            3,
            SortMode::ByLastUpdated,
            "󰥔",
            "Last Updated (recent first)",
        ),
        (4, SortMode::ByPriority, "⚠", "Priority (urgent first)"),
        (5, SortMode::ByLinearStatus, "◐", "Linear Status (default)"),
        (
            6,
            SortMode::ByPRActivity,
            "",
            "PR Activity (needs attention)",
        ),
    ];

    let mut lines: Vec<Line> = vec![Line::from("")];

    for (idx, mode, icon, label) in options {
        let is_selected = current == mode;
        let marker = if is_selected { "●" } else { "○" };
        let text_style = if is_selected { active_style } else { dim_style };

        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} ", marker),
                if is_selected {
                    Style::default().fg(Color::Green)
                } else {
                    dim_style
                },
            ),
            Span::styled(format!("[{}] ", idx), text_style),
            Span::styled(format!("{} ", icon), icon_style),
            Span::styled(label, text_style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Press 1-6 to select | Esc: Cancel",
        dim_style,
    )));

    let block = Block::default()
        .title(" 󰒺 Sort By ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    let lines = fit_lines_to_area(lines, inner, 1);
    let paragraph = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn draw_filter_menu(f: &mut Frame, app: &App) {
    // Calculate height based on content
    let base_height = 25; // Base height for headers and footer
    let cycle_height = app.available_cycles.len().min(5) + 2;
    let project_height = if app.available_projects.is_empty() {
        0
    } else {
        app.available_projects.len().min(5) + 2
    };
    let assignee_height = 5 + app.available_team_members.len().min(5); // header + all + me/unassigned + members + spacer
    let total_height =
        (base_height + cycle_height + project_height + assignee_height).min(45) as u16;

    let area = popup_rect(55, 80, 42, total_height, f.area());

    f.render_widget(Clear, area);

    let active_style = Style::default().fg(Color::White);
    let dim_style = Style::default().fg(Color::DarkGray);
    let header_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let mut lines: Vec<Line> = Vec::new();

    // ─────────────────────────────────────────────────────────────────
    // Cycle section
    // ─────────────────────────────────────────────────────────────────
    lines.push(Line::from(Span::styled("  CYCLE", header_style)));
    lines.extend(render_filter_checkbox(
        "0",
        "All cycles",
        app.filter_cycles.is_empty(),
        active_style,
        dim_style,
    ));
    for (idx, cycle) in app.available_cycles.iter().enumerate().take(5) {
        let is_selected = app.filter_cycles.contains(&cycle.id);
        let label = format!("Cycle {} ({})", cycle.number, truncate_str(&cycle.name, 10));
        lines.extend(render_filter_checkbox(
            &(idx + 1).to_string(),
            &label,
            is_selected,
            active_style,
            dim_style,
        ));
    }
    lines.push(Line::from(""));

    // ─────────────────────────────────────────────────────────────────
    // Priority section
    // ─────────────────────────────────────────────────────────────────
    lines.push(Line::from(Span::styled("  PRIORITY", header_style)));
    let priorities = [
        ('u', LinearPriority::Urgent, "Urgent"),
        ('h', LinearPriority::High, "High"),
        ('m', LinearPriority::Medium, "Medium"),
        ('l', LinearPriority::Low, "Low"),
        ('n', LinearPriority::NoPriority, "No Priority"),
    ];
    for (key, priority, label) in priorities {
        let is_selected =
            app.filter_priorities.is_empty() || app.filter_priorities.contains(&priority);
        let priority_cfg = priority_config(priority);
        lines.push(render_priority_checkbox(
            key,
            priority_cfg,
            label,
            is_selected,
            active_style,
            dim_style,
        ));
    }
    lines.push(Line::from(""));

    // ─────────────────────────────────────────────────────────────────
    // Project section (only if projects available)
    // ─────────────────────────────────────────────────────────────────
    if !app.available_projects.is_empty() {
        lines.push(Line::from(Span::styled("  PROJECT", header_style)));
        lines.extend(render_filter_checkbox(
            "p0",
            "All projects",
            app.filter_projects.is_empty(),
            active_style,
            dim_style,
        ));
        for (idx, project) in app.available_projects.iter().enumerate().take(5) {
            let is_selected = app.filter_projects.contains(&project.id);
            lines.extend(render_filter_checkbox(
                &format!("p{}", idx + 1),
                &truncate_str(&project.name, 20),
                is_selected,
                active_style,
                dim_style,
            ));
        }
        lines.push(Line::from(""));
    }

    // ─────────────────────────────────────────────────────────────────
    // Assignee section
    // ─────────────────────────────────────────────────────────────────
    lines.push(Line::from(Span::styled("  ASSIGNEE", header_style)));
    let all_assignees = app.filter_assignees.is_empty();
    let me_selected = app.filter_assignees.contains("me");
    let unassigned_selected = app.filter_assignees.contains("unassigned");
    lines.extend(render_filter_checkbox(
        "s9",
        "All assignees",
        all_assignees,
        active_style,
        dim_style,
    ));
    lines.extend(render_filter_checkbox(
        "s0",
        "Me",
        me_selected,
        active_style,
        dim_style,
    ));
    lines.extend(render_filter_checkbox(
        "s1",
        "Unassigned",
        unassigned_selected,
        active_style,
        dim_style,
    ));
    for (idx, member) in app.available_team_members.iter().enumerate().take(5) {
        let is_selected = app.filter_assignees.contains(&member.id);
        let name = member.display_name.as_ref().unwrap_or(&member.name);
        lines.extend(render_filter_checkbox(
            &format!("s{}", idx + 2),
            &truncate_str(name, 20),
            is_selected,
            active_style,
            dim_style,
        ));
    }
    lines.push(Line::from(""));

    // ─────────────────────────────────────────────────────────────────
    // Status section
    // ─────────────────────────────────────────────────────────────────
    lines.push(Line::from(Span::styled("  STATUS", header_style)));
    lines.extend(render_filter_checkbox(
        "d",
        "Show completed",
        app.show_completed,
        active_style,
        dim_style,
    ));
    lines.extend(render_filter_checkbox(
        "x",
        "Show canceled",
        app.show_canceled,
        active_style,
        dim_style,
    ));
    lines.push(Line::from(""));

    // ─────────────────────────────────────────────────────────────────
    // Hierarchy section
    // ─────────────────────────────────────────────────────────────────
    lines.push(Line::from(Span::styled("  HIERARCHY", header_style)));
    lines.extend(render_filter_checkbox(
        "t",
        "Show sub-issues",
        app.show_sub_issues,
        active_style,
        dim_style,
    ));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  [a] All | [c] Clear | Esc: Close",
        dim_style,
    )));

    let block = Block::default()
        .title(" 󰈲 Filter ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let inner = block.inner(area);
    let lines = fit_lines_to_area(lines, inner, 1);
    let paragraph = Paragraph::new(lines).block(block);

    f.render_widget(paragraph, area);
}

/// Render a filter checkbox line
fn render_filter_checkbox<'a>(
    key: &str,
    label: &str,
    is_selected: bool,
    active_style: Style,
    dim_style: Style,
) -> Vec<Line<'a>> {
    let marker = if is_selected { "[x]" } else { "[ ]" };
    let style = if is_selected { active_style } else { dim_style };
    vec![Line::from(Span::styled(
        format!("  [{}] {:<28} {}", key, label, marker),
        style,
    ))]
}

/// Render a priority filter checkbox with icon
fn render_priority_checkbox<'a>(
    key: char,
    priority_cfg: StatusConfig,
    label: &str,
    is_selected: bool,
    active_style: Style,
    dim_style: Style,
) -> Line<'a> {
    let marker = if is_selected { "[x]" } else { "[ ]" };
    let style = if is_selected { active_style } else { dim_style };
    Line::from(vec![
        Span::styled(format!("  [{}] ", key), style),
        Span::styled(format!("{} ", priority_cfg.icon), priority_cfg.style),
        Span::styled(format!("{:<20}", label), style),
        Span::styled(format!("  {}", marker), style),
    ])
}

fn popup_rect(percent_x: u16, percent_y: u16, min_width: u16, min_height: u16, r: Rect) -> Rect {
    let max_width = r.width.saturating_sub(2).max(1);
    let max_height = r.height.saturating_sub(2).max(1);

    let target_width = (r.width.saturating_mul(percent_x) / 100).max(min_width);
    let target_height = (r.height.saturating_mul(percent_y) / 100).max(min_height);

    let width = target_width.min(max_width);
    let height = target_height.min(max_height);

    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;

    Rect {
        x,
        y,
        width,
        height,
    }
}
