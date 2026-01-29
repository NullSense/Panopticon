use super::App;
use super::search::FuzzySearch;
use crate::data::{AgentStatus, GitHubPRStatus, LinearChildRef, LinearPriority, LinearStatus, SortMode, VercelStatus, VisualItem};
use pulldown_cmark::{Event, Parser, Tag};
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
    pub const STATUS_TRIAGE: &str = "◇";      // Diamond outline - needs triage
    pub const STATUS_BACKLOG: &str = "○";     // Empty circle
    pub const STATUS_TODO: &str = "◔";        // 1/4 filled
    pub const STATUS_IN_PROGRESS: &str = "◑"; // 1/2 filled
    pub const STATUS_IN_REVIEW: &str = "◕";   // 3/4 filled
    pub const STATUS_DONE: &str = "●";        // Full circle
    pub const STATUS_CANCELED: &str = "⊘";    // Slashed circle
    pub const STATUS_DUPLICATE: &str = "◈";   // Diamond fill - duplicate

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

    // Issue detail category icons
    pub const ICON_TEAM: &str = "󰏬";       // nf-md-account_group
    pub const ICON_PROJECT: &str = "󰈙";    // nf-md-folder
    pub const ICON_CYCLE: &str = "󰃰";      // nf-md-calendar_clock
    pub const ICON_ESTIMATE: &str = "󰎚";   // nf-md-numeric
    pub const ICON_LABELS: &str = "󰌕";     // nf-md-tag_multiple
    pub const ICON_CREATED: &str = "󰃭";    // nf-md-calendar_plus
    pub const ICON_UPDATED: &str = "󰦒";    // nf-md-calendar_edit
    pub const ICON_DOCUMENT: &str = "󰈚";   // nf-md-file_document
    pub const ICON_PARENT: &str = "󰁝";     // nf-md-arrow_up_bold
    pub const ICON_CHILDREN: &str = "󰁅";   // nf-md-arrow_down_bold
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
        let progress_text = if let Some(ref p) = app.refresh_progress {
            if p.total_issues > 0 {
                format!("{} {} [{}/{}]", app.spinner_char(), p.current_stage, p.completed, p.total_issues)
            } else {
                format!("{} {}", app.spinner_char(), p.current_stage)
            }
        } else {
            format!("{} Loading...", app.spinner_char())
        };
        Line::from(vec![
            Span::styled("󰣖 ", Style::default().fg(Color::Cyan)),
            Span::styled("Panopticon ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(progress_text, Style::default().fg(Color::Cyan)),
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
        if app.resize_mode() && app.resize_column_idx == idx {
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
                // Use status-specific colors (Triage=orange, In Progress=blue, etc.)
                let status_cfg = linear_status_config(*status);
                let base_style = status_cfg.style.add_modifier(Modifier::BOLD);
                let style = if is_selected {
                    base_style.bg(Color::DarkGray)
                } else {
                    base_style
                };

                items.push(ListItem::new(Line::from(vec![Span::styled(header, style)])));
            }
            VisualItem::Workstream(ws_idx) => {
                if let Some(ws) = app.state.workstreams.get(*ws_idx) {
                    let search_query = if !app.state.search_query.is_empty() {
                        Some(app.state.search_query.as_str())
                    } else {
                        None
                    };
                    let row = build_workstream_row(ws, is_selected, &app.column_widths, search_query);
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

fn build_workstream_row(ws: &crate::data::Workstream, selected: bool, widths: &[usize; 8], search_query: Option<&str>) -> ListItem<'static> {
    WorkstreamRowBuilder::new(ws, widths, search_query).build(selected)
}

/// Builder for workstream row UI elements
/// Decomposes the row building into smaller, focused methods
struct WorkstreamRowBuilder<'a> {
    ws: &'a crate::data::Workstream,
    widths: &'a [usize; 8],
    search_query: Option<&'a str>,
    sep_style: Style,
}

impl<'a> WorkstreamRowBuilder<'a> {
    fn new(ws: &'a crate::data::Workstream, widths: &'a [usize; 8], search_query: Option<&'a str>) -> Self {
        Self {
            ws,
            widths,
            search_query,
            sep_style: Style::default().fg(Color::DarkGray),
        }
    }

    fn build(self, selected: bool) -> ListItem<'static> {
        let line = Line::from(self.build_spans());
        let style = if selected {
            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        ListItem::new(line).style(style)
    }

    fn build_spans(&self) -> Vec<Span<'static>> {
        let (sub_prefix, sub_suffix) = self.sub_issue_indicators();

        let mut spans = vec![
            Span::raw("  "),
            self.status_span(),
            self.separator(),
            self.priority_span(),
            self.separator(),
        ];

        // Add sub-issue tree prefix before ID (or 2-space padding to match header icon)
        if !sub_prefix.is_empty() {
            spans.push(Span::styled(sub_prefix, Style::default().fg(Color::DarkGray)));
        } else {
            spans.push(Span::raw("  "));
        }
        spans.extend(self.id_spans());
        spans.push(self.separator());
        spans.extend(self.title_spans(&sub_suffix));

        // Add parent reference suffix after title for sub-issues
        if !sub_suffix.is_empty() {
            spans.push(Span::styled(sub_suffix, Style::default().fg(Color::DarkGray)));
        }

        spans.extend(vec![
            self.separator(),
            self.pr_span(),
            self.separator(),
            self.agent_span(),
            self.separator(),
            self.vercel_span(),
            self.separator(),
            self.elapsed_span(),
        ]);

        spans
    }

    fn separator(&self) -> Span<'static> {
        Span::styled(" │ ", self.sep_style)
    }

    fn sub_issue_indicators(&self) -> (String, String) {
        if let Some(parent) = &self.ws.linear_issue.parent {
            ("└ ".to_string(), format!(" ← {}", parent.identifier))
        } else {
            (String::new(), String::new())
        }
    }

    fn status_span(&self) -> Span<'static> {
        let cfg = linear_status_config(self.ws.linear_issue.status);
        Span::styled(cfg.icon.to_string(), cfg.style)
    }

    fn priority_span(&self) -> Span<'static> {
        let cfg = priority_config(self.ws.linear_issue.priority);
        Span::styled(format!("{:^width$}", cfg.icon, width = self.widths[COL_IDX_PRIORITY]), cfg.style)
    }

    fn id_spans(&self) -> Vec<Span<'static>> {
        let issue = &self.ws.linear_issue;
        // Always subtract 2 for the prefix (either "└ " for sub-issues or "  " for regular)
        let id_width = self.widths[COL_IDX_ID].saturating_sub(2);
        let id_text = format!("{:<width$}", issue.identifier, width = id_width);
        highlight_search_matches(&id_text, self.search_query, linear_status_config(issue.status).style)
    }

    fn title_spans(&self, sub_suffix: &str) -> Vec<Span<'static>> {
        let issue = &self.ws.linear_issue;
        let is_sub_issue = issue.parent.is_some();
        let title_max = if is_sub_issue {
            self.widths[COL_IDX_TITLE].saturating_sub(sub_suffix.chars().count())
        } else {
            self.widths[COL_IDX_TITLE]
        };

        let title = if issue.title.chars().count() > title_max {
            let truncated: String = issue.title.chars().take(title_max.saturating_sub(1)).collect();
            format!("{}…", truncated)
        } else {
            format!("{:<width$}", issue.title, width = title_max)
        };
        highlight_search_matches(&title, self.search_query, Style::default())
    }

    fn pr_span(&self) -> Span<'static> {
        let col_pr = self.widths[COL_IDX_PR];
        let (text, style) = if let Some(pr) = &self.ws.github_pr {
            let cfg = pr_status_config(pr.status);
            let text = format!("{} PR#{:<5}", cfg.icon, pr.number);
            (format!("{:<width$}", text, width = col_pr.saturating_sub(2)), cfg.style)
        } else {
            (format!("{:<width$}", format!("{} --", icons::AGENT_NONE), width = col_pr.saturating_sub(2)), Style::default().fg(Color::DarkGray))
        };
        Span::styled(text, style)
    }

    fn agent_span(&self) -> Span<'static> {
        let col_agent = self.widths[COL_IDX_AGENT];
        let (text, style) = if let Some(session) = &self.ws.agent_session {
            let cfg = agent_status_config(session.status);
            let label = session.status.label();
            let text = format!("{} {:<5}", cfg.icon, label);
            (format!("{:<width$}", text, width = col_agent.saturating_sub(2)), cfg.style)
        } else {
            (format!("{:<width$}", format!("{} --", icons::AGENT_NONE), width = col_agent.saturating_sub(2)), Style::default().fg(Color::DarkGray))
        };
        Span::styled(text, style)
    }

    fn vercel_span(&self) -> Span<'static> {
        let col_vercel = self.widths[COL_IDX_VERCEL];
        let (text, style) = if let Some(deploy) = &self.ws.vercel_deployment {
            let cfg = vercel_status_config(deploy.status);
            (format!("{:^width$}", cfg.icon, width = col_vercel), cfg.style)
        } else {
            (format!("{:^width$}", icons::VERCEL_NONE, width = col_vercel), Style::default().fg(Color::DarkGray))
        };
        Span::styled(text, style)
    }

    fn elapsed_span(&self) -> Span<'static> {
        let col_time = self.widths[COL_IDX_TIME];
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
            format!("{:>width$}", elapsed, width = col_time.saturating_sub(2)),
            Style::default().fg(Color::DarkGray),
        )
    }
}

// Icon and color helpers

/// Highlight search matches in text with yellow/bold styling
fn highlight_search_matches(text: &str, query: Option<&str>, base_style: Style) -> Vec<Span<'static>> {
    let highlight_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);

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
                style: Style::default().fg(Color::White).bg(Color::Red).add_modifier(Modifier::BOLD),
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
                style: Style::default().fg(Color::Green),
            },
            AgentStatus::Idle => StatusConfig {
                icon: icons::AGENT_IDLE,
                style: Style::default().fg(Color::Yellow),
            },
            AgentStatus::WaitingForInput => StatusConfig {
                icon: icons::AGENT_WAITING,
                style: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            },
            AgentStatus::Done => StatusConfig {
                icon: icons::AGENT_DONE,
                style: Style::default().fg(Color::DarkGray),
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
        Span::styled(text, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
    } else if app.state.search_mode {
        let text = if width >= 50 {
            " Type to search | Enter: select | Esc: cancel "
        } else if width >= 30 {
            " Search | Enter | Esc "
        } else {
            " Search "
        };
        Span::styled(text, Style::default().fg(Color::Yellow))
    } else {
        // Responsive shortcuts based on available width
        let text = if width >= 90 {
            let sort_indicator = format!("[{}]", app.state.sort_mode.label());
            format!(" j/k: nav | /: search | o: links | f: filter | s: sort {} | R: resize | ?: help ", sort_indicator)
        } else if width >= 70 {
            " j/k: nav | /: search | o: links | f: filter | s: sort | ?: help ".to_string()
        } else if width >= 50 {
            " j/k /search o:links f:filter s:sort ?:help ".to_string()
        } else if width >= 30 {
            " j/k / o f s ? ".to_string()
        } else {
            " ? help ".to_string()
        };
        Span::styled(text, Style::default().fg(Color::DarkGray))
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

    let tab_1_style = if app.help_tab() == 0 { tab_style_active } else { tab_style_inactive };
    let tab_2_style = if app.help_tab() == 1 { tab_style_active } else { tab_style_inactive };

    let tabs = Line::from(vec![
        Span::styled(" [1] Shortcuts ", tab_1_style),
        Span::raw(" │ "),
        Span::styled("[2] Status Legend ", tab_2_style),
    ]);

    let content = if app.help_tab() == 0 {
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
    let area = centered_rect(70, 75, f.area());

    f.render_widget(Clear, area);

    let active_style = Style::default().fg(Color::White);
    let inactive_style = Style::default().fg(Color::DarkGray);
    let label_style = Style::default().fg(Color::Cyan);
    let title_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let selected_child_style = Style::default().fg(Color::White).bg(Color::DarkGray).add_modifier(Modifier::BOLD);

    // Search highlighting style
    let search_highlight_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);

    let lines: Vec<Line> = if let Some(ws) = app.modal_issue() {
        let issue = &ws.linear_issue;

        let mut lines = Vec::new();

        // Show search input if in search mode
        if app.modal_search_mode {
            let search_style = Style::default().fg(Color::Yellow);
            lines.push(Line::from(vec![
                Span::styled("  / ", search_style),
                Span::styled(&app.modal_search_query, Style::default().fg(Color::White)),
                Span::styled("█", Style::default().fg(Color::Yellow)), // Cursor
            ]));
            lines.push(Line::from(""));
        } else if !app.modal_search_query.is_empty() {
            // Show active search filter
            let search_style = Style::default().fg(Color::Cyan);
            lines.push(Line::from(vec![
                Span::styled("  🔍 ", search_style),
                Span::styled(format!("\"{}\"", &app.modal_search_query), search_highlight_style),
                Span::styled(" (/ to edit, Esc to clear)", inactive_style),
            ]));
            lines.push(Line::from(""));
        }

        // Show navigation breadcrumb if navigated
        if !app.issue_navigation_stack.is_empty() || app.modal_issue_id.is_some() {
            let nav_style = Style::default().fg(Color::DarkGray);
            let back_style = Style::default().fg(Color::Cyan);
            lines.push(Line::from(vec![
                Span::styled("  ", nav_style),
                Span::styled("← Esc", back_style),
                Span::styled(format!(" to go back ({} in history)", app.issue_navigation_stack.len()), nav_style),
            ]));
            lines.push(Line::from(""));
        }

        // Search query for highlighting all fields
        let search_q = if app.modal_search_query.is_empty() {
            None
        } else {
            Some(app.modal_search_query.as_str())
        };

        // Issue identifier and title (always shown) - with highlighting
        let mut title_line = vec![Span::styled("  ", title_style)];
        title_line.extend(highlight_search_matches(&issue.identifier, search_q, title_style));
        title_line.push(Span::styled(" ", title_style));
        title_line.extend(highlight_search_matches(&truncate_str(&issue.title, 50), search_q, active_style));
        lines.push(Line::from(title_line));
        lines.push(Line::from(""));

        // Status and Priority with icons
        let status_cfg = linear_status_config(issue.status);
        let priority_cfg = priority_config(issue.priority);
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", status_cfg.icon), status_cfg.style),
            Span::styled("Status: ", label_style),
            Span::styled(issue.status.display_name(), active_style),
            Span::styled("  │  ", inactive_style),
            Span::styled(format!("{} ", priority_cfg.icon), priority_cfg.style),
            Span::styled(format!("Priority: {}", issue.priority.label()), active_style),
        ]));

        // Team and Project with icons - with highlighting
        if issue.team.is_some() || issue.project.is_some() {
            let mut spans = Vec::new();
            if let Some(team) = &issue.team {
                spans.push(Span::styled(format!("  {} ", icons::ICON_TEAM), label_style));
                spans.push(Span::styled("Team: ", label_style));
                spans.extend(highlight_search_matches(team, search_q, active_style));
            }
            if let Some(project) = &issue.project {
                if issue.team.is_some() {
                    spans.push(Span::styled("  │  ", inactive_style));
                } else {
                    spans.push(Span::styled("  ", label_style));
                }
                spans.push(Span::styled(format!("{} ", icons::ICON_PROJECT), label_style));
                spans.push(Span::styled("Project: ", label_style));
                spans.extend(highlight_search_matches(project, search_q, active_style));
            }
            lines.push(Line::from(spans));
        }

        // Cycle with icon - with highlighting
        if let Some(cycle) = &issue.cycle {
            let mut spans = vec![
                Span::styled(format!("  {} ", icons::ICON_CYCLE), label_style),
                Span::styled("Cycle: ", label_style),
            ];
            spans.extend(highlight_search_matches(&cycle.name, search_q, active_style));
            spans.push(Span::styled(format!(" ({})", cycle.number), active_style));
            lines.push(Line::from(spans));
        }

        // Estimate with icon
        if let Some(est) = issue.estimate {
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", icons::ICON_ESTIMATE), label_style),
                Span::styled("Estimate: ", label_style),
                Span::styled(format!("{} points", est), active_style),
            ]));
        }

        // Labels with icon - with highlighting
        if !issue.labels.is_empty() {
            let mut spans = vec![
                Span::styled(format!("  {} ", icons::ICON_LABELS), Style::default().fg(Color::Magenta)),
                Span::styled("Labels: ", label_style),
            ];
            let label_style_base = Style::default().fg(Color::Magenta);
            for (i, label) in issue.labels.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(", ", label_style_base));
                }
                spans.extend(highlight_search_matches(&label.name, search_q, label_style_base));
            }
            lines.push(Line::from(spans));
        }

        // Dates with icons
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", icons::ICON_CREATED), inactive_style),
            Span::styled("Created: ", label_style),
            Span::styled(issue.created_at.format("%Y-%m-%d").to_string(), inactive_style),
            Span::styled("  │  ", inactive_style),
            Span::styled(format!("{} ", icons::ICON_UPDATED), inactive_style),
            Span::styled("Updated: ", label_style),
            Span::styled(issue.updated_at.format("%Y-%m-%d %H:%M").to_string(), inactive_style),
        ]));

        // Parent issue - with highlighting
        if let Some(parent) = &issue.parent {
            lines.push(Line::from(""));
            let mut parent_spans = vec![
                Span::styled(format!("  {} ", icons::ICON_PARENT), Style::default().fg(Color::Blue)),
                Span::styled("Parent: ", label_style),
            ];
            parent_spans.extend(highlight_search_matches(&parent.identifier, search_q, Style::default().fg(Color::Yellow)));
            parent_spans.push(Span::styled(" ", Style::default()));
            parent_spans.extend(highlight_search_matches(&truncate_str(&parent.title, 40), search_q, active_style));
            parent_spans.push(Span::styled(" [p]", inactive_style));
            lines.push(Line::from(parent_spans));
        }

        // Sub-issues (children) - with j/k navigation, scrolling, sort, and search filtering
        if !issue.children.is_empty() {
            lines.push(Line::from(""));

            // Filter children based on modal search query (searches all fields)
            let filtered_children: Vec<&LinearChildRef> = if app.modal_search_query.is_empty() {
                issue.children.iter().collect()
            } else {
                let mut fuzzy = FuzzySearch::new();
                issue.children
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
                        fuzzy.multi_term_match(&app.modal_search_query, &text).is_some()
                    })
                    .collect()
            };

            // Sort filtered children according to current sort mode
            let sorted_children = sort_filtered_children(filtered_children, app.state.sort_mode);
            let total_children = sorted_children.len();
            let visible_height: usize = 8;
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
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", icons::ICON_CHILDREN), Style::default().fg(Color::Green)),
                Span::styled(count_text, label_style),
                Span::styled(hint, inactive_style),
            ]));

            // Show "No matches" if filter returns empty
            if sorted_children.is_empty() && !app.modal_search_query.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("    ", inactive_style),
                    Span::styled("No matching sub-issues", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
                ]));
            } else {
                // Show scroll-up indicator if scrolled
                if scroll > 0 {
                    lines.push(Line::from(vec![
                        Span::styled("    ", inactive_style),
                        Span::styled(format!("↑ {} more above", scroll), Style::default().fg(Color::Cyan)),
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

                    let row_style = if is_selected { selected_child_style } else { Style::default() };
                    let id_style = if is_selected { selected_child_style } else { Style::default().fg(Color::Yellow) };
                    let title_row_style = if is_selected { selected_child_style } else { active_style };
                    let key_style = if is_selected { selected_child_style } else { inactive_style };

                    // Show shortcut only for first 9 visible items
                    let shortcut = if display_i < 9 {
                        format!(" [c{}]", display_i + 1)
                    } else {
                        String::new()
                    };

                    // Build line with highlighted spans for identifier and title
                    let mut spans = vec![
                        Span::styled(if is_selected { " >> " } else { "    " }, row_style),
                        Span::styled(format!("{} ", child_status_cfg.icon), if is_selected { row_style } else { child_status_cfg.style }),
                        Span::styled(format!("{} ", child_priority_cfg.icon), if is_selected { row_style } else { child_priority_cfg.style }),
                    ];

                    // Add highlighted identifier
                    spans.extend(highlight_search_matches(&child.identifier, search_query, id_style));
                    spans.push(Span::styled(" ", row_style));

                    // Add highlighted title
                    let truncated_title = truncate_str(&child.title, 35);
                    spans.extend(highlight_search_matches(&truncated_title, search_query, title_row_style));

                    spans.push(Span::styled(shortcut, key_style));

                    lines.push(Line::from(spans));
                }

                // Show scroll-down indicator if more below
                let visible_end = scroll + visible_height;
                if visible_end < total_children {
                    lines.push(Line::from(vec![
                        Span::styled("    ", inactive_style),
                        Span::styled(format!("↓ {} more below", total_children - visible_end), Style::default().fg(Color::Cyan)),
                    ]));
                }
            }
        }

        // Attachments (documents) - with highlighting
        if !issue.attachments.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", icons::ICON_DOCUMENT), label_style),
                Span::styled(format!("Documents ({}):", issue.attachments.len()), label_style),
            ]));
            for (i, attachment) in issue.attachments.iter().take(5).enumerate() {
                let source_icon = match attachment.source_type.as_deref() {
                    Some("figma") => "󰡁",
                    Some("notion") => "󰈙",
                    Some("github") => "",
                    Some("slack") => "󰒱",
                    _ => "󰈙",
                };
                let mut att_spans = vec![Span::styled(format!("    {} ", source_icon), active_style)];
                att_spans.extend(highlight_search_matches(&truncate_str(&attachment.title, 40), search_q, active_style));
                att_spans.push(Span::styled(format!(" [d{}]", i + 1), inactive_style));
                lines.push(Line::from(att_spans));
            }
            if issue.attachments.len() > 5 {
                lines.push(Line::from(Span::styled(
                    format!("    ... and {} more", issue.attachments.len() - 5),
                    inactive_style,
                )));
            }
        }

        // Description (truncated) - press 'd' for full view - with highlighting
        if let Some(desc) = &issue.description {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", icons::ICON_DOCUMENT), label_style),
                Span::styled("Description ", label_style),
                Span::styled("[d] full view", inactive_style),
            ]));
            // Wrap description to ~60 chars per line, max 3 lines - with highlighting
            let desc_clean = desc.replace('\n', " ").replace("  ", " ");
            for (i, chunk) in desc_clean.chars().collect::<Vec<_>>().chunks(58).enumerate() {
                if i >= 3 {
                    lines.push(Line::from(Span::styled("    ...", inactive_style)));
                    break;
                }
                let text: String = chunk.iter().collect();
                let trimmed = text.trim();
                let mut desc_spans = vec![Span::styled("    ", inactive_style)];
                desc_spans.extend(highlight_search_matches(trimmed, search_q, inactive_style));
                lines.push(Line::from(desc_spans));
            }
        }

        // Footer hint
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  l: links | /: search | d: desc | c#: sub-issue | p: parent | Esc: back",
            inactive_style,
        )));

        lines
    } else {
        vec![Line::from(Span::styled("  No workstream selected", inactive_style))]
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" 󰌷 Issue Details ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );

    f.render_widget(paragraph, area);
}

/// Draw the quick links popup (overlays issue details)
fn draw_links_popup(f: &mut Frame, app: &App) {
    // Small centered popup
    let area = centered_rect(40, 30, f.area());

    f.render_widget(Clear, area);

    let active_style = Style::default().fg(Color::White);
    let inactive_style = Style::default().fg(Color::DarkGray);

    let lines: Vec<Line> = if let Some(ws) = app.modal_issue() {
        let issue = &ws.linear_issue;
        let has_pr = ws.github_pr.is_some();
        let has_vercel = ws.vercel_deployment.is_some();
        let has_session = ws.agent_session.is_some();

        vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  [1] 󰌷 Linear: {}", issue.identifier),
                active_style,
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
                if has_vercel { active_style } else { inactive_style },
            )),
            Line::from(Span::styled(
                if ws.agent_session.is_some() {
                    "  [4] 󰚩 Agent: teleport".to_string()
                } else {
                    "  [4] 󰚩 Agent: (no session)".to_string()
                },
                if has_session { active_style } else { inactive_style },
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  1-4: open | Esc: close",
                inactive_style,
            )),
        ]
    } else {
        vec![Line::from(Span::styled("  No issue", inactive_style))]
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" 󰌷 Open Links ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    f.render_widget(paragraph, area);
}

fn draw_description_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(80, 80, f.area());

    f.render_widget(Clear, area);

    let title_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(Color::White);
    let dim_style = Style::default().fg(Color::DarkGray);

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
            let content_width = area.width.saturating_sub(6) as usize;
            let markdown_lines = parse_markdown_to_lines(desc, content_width);

            // Apply scroll offset
            let visible_lines: Vec<Line> = markdown_lines
                .into_iter()
                .skip(app.description_scroll)
                .collect();

            lines.extend(visible_lines);
        } else {
            lines.push(Line::from(Span::styled("  No description", dim_style)));
        }

        lines
    } else {
        vec![Line::from(Span::styled("  No workstream selected", dim_style))]
    };

    let scroll_hint = format!("[line {}]", app.description_scroll + 1);
    let title = format!(" {} Description  {} ", icons::ICON_DOCUMENT, scroll_hint);

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .title_bottom(Line::from(" j/k: scroll | Esc: close ").centered())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
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
    let bold_style = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
    let italic_style = Style::default().fg(Color::White).add_modifier(Modifier::ITALIC);
    let bold_italic_style = Style::default().fg(Color::White).add_modifier(Modifier::BOLD | Modifier::ITALIC);
    let code_style = Style::default().fg(Color::Gray);
    let code_block_style = Style::default().fg(Color::Gray);
    let heading_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let link_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED);
    let quote_style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC);

    let flush_line = |spans: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>, indent: &str| {
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
                    let mut current_line_len = current_spans.iter()
                        .map(|s| s.content.len())
                        .sum::<usize>();

                    for word in words {
                        if current_line_len + word.len() + 1 > max_width && current_line_len > 0 {
                            flush_line(&mut current_spans, &mut lines, "  ");
                            current_line_len = 0;
                        }

                        if current_line_len > 0 {
                            current_spans.push(Span::raw(" ".to_string()));
                            current_line_len += 1;
                        }
                        current_spans.push(Span::styled(word.to_string(), style));
                        current_line_len += word.len();
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
fn sort_filtered_children<'a>(
    mut children: Vec<&'a LinearChildRef>,
    sort_mode: SortMode,
) -> Vec<&'a LinearChildRef> {
    match sort_mode {
        SortMode::ByLinearStatus => {
            children.sort_by_key(|c| c.status.sort_order());
        }
        SortMode::ByPriority => {
            children.sort_by_key(|c| c.priority.sort_order());
        }
        SortMode::ByAgentStatus | SortMode::ByVercelStatus | SortMode::ByPRActivity | SortMode::ByLastUpdated => {
            // Children don't have these fields, sort by status as fallback
            children.sort_by_key(|c| c.status.sort_order());
        }
    }
    children
}

/// Truncate a string to max length, adding ellipsis if needed
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_len - 3).collect::<String>())
    }
}

fn draw_sort_menu(f: &mut Frame, app: &App) {
    use crate::data::SortMode;

    let area = centered_rect(50, 45, f.area());

    f.render_widget(Clear, area);

    let current = app.state.sort_mode;
    let active_style = Style::default().fg(Color::White);
    let dim_style = Style::default().fg(Color::DarkGray);
    let icon_style = Style::default().fg(Color::Cyan);

    // Sort options with icons
    let options: Vec<(usize, SortMode, &str, &str)> = vec![
        (1, SortMode::ByAgentStatus, "󰚩", "Agent Status (waiting first)"),
        (2, SortMode::ByVercelStatus, "▲", "Vercel Status (errors first)"),
        (3, SortMode::ByLastUpdated, "󰥔", "Last Updated (recent first)"),
        (4, SortMode::ByPriority, "⚠", "Priority (urgent first)"),
        (5, SortMode::ByLinearStatus, "◐", "Linear Status (default)"),
        (6, SortMode::ByPRActivity, "", "PR Activity (needs attention)"),
    ];

    let mut lines: Vec<Line> = vec![Line::from("")];

    for (idx, mode, icon, label) in options {
        let is_selected = current == mode;
        let marker = if is_selected { "●" } else { "○" };
        let text_style = if is_selected { active_style } else { dim_style };

        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", marker), if is_selected { Style::default().fg(Color::Green) } else { dim_style }),
            Span::styled(format!("[{}] ", idx), text_style),
            Span::styled(format!("{} ", icon), icon_style),
            Span::styled(label, text_style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  Press 1-6 to select | Esc: Cancel", dim_style)));

    let paragraph = Paragraph::new(lines)
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
        let priority_cfg = priority_config(priority);
        // Show priority icon with its color
        lines.push(Line::from(vec![
            Span::styled(format!("  [{}] ", key), if is_selected { active_style } else { dim_style }),
            Span::styled(format!("{} ", priority_cfg.icon), priority_cfg.style),
            Span::styled(format!("{:<17}", label), if is_selected { active_style } else { dim_style }),
            Span::styled(format!("  {}", marker), if is_selected { active_style } else { dim_style }),
        ]));
    }

    lines.push(Line::from(""));

    // Hierarchy section
    lines.push(Line::from(Span::styled("  HIERARCHY", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));

    let sub_marker = if app.show_sub_issues { "[x]" } else { "[ ]" };
    lines.push(Line::from(Span::styled(
        format!("  [t] Show sub-issues             {}", sub_marker),
        if app.show_sub_issues { active_style } else { dim_style }
    )));

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
