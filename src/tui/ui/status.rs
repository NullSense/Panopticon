//! Status configuration and status bar rendering.

use super::icons;
use super::layout::{fit_lines_to_area, popup_rect};
use crate::data::{AgentStatus, GitHubPRStatus, LinearPriority, LinearStatus, VercelStatus};
use crate::tui::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Unified status configuration - single source of truth for icon and style.
pub struct StatusConfig {
    pub icon: &'static str,
    pub style: Style,
}

/// Trait for types that can provide their display configuration (icon + style).
pub trait StatusConfigurable {
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

// Convenience functions
pub fn linear_status_config(status: LinearStatus) -> StatusConfig {
    status.status_config()
}

pub fn priority_config(priority: LinearPriority) -> StatusConfig {
    priority.status_config()
}

pub fn pr_status_config(status: GitHubPRStatus) -> StatusConfig {
    status.status_config()
}

pub fn agent_status_config(status: AgentStatus) -> StatusConfig {
    status.status_config()
}

pub fn vercel_status_config(status: VercelStatus) -> StatusConfig {
    status.status_config()
}

/// Generate the status legend for help popup.
pub fn generate_status_legend() -> Vec<&'static str> {
    vec![
        "",
        "  LINEAR ISSUE STATUS",
        "  ───────────────────",
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

/// Draw the status bar at the bottom of the screen.
pub fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
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

/// Draw the view tab bar (only shown when multiple views are configured).
pub fn draw_view_tabs(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = Vec::new();
    spans.push(Span::styled(" ", Style::default()));

    for (i, vc) in app.view_configs.iter().enumerate() {
        let is_active = i == app.active_view;
        let is_loading = if is_active {
            app.is_loading
        } else {
            app.view_is_loading.get(i).copied().unwrap_or(false)
        };

        let label = format!(" {} {} ", i + 1, vc.name);

        let style = if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else if is_loading {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        spans.push(Span::styled(label, style));
        if i + 1 < app.view_configs.len() {
            spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        }
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    f.render_widget(paragraph, area);
}

/// Draw the help popup.
pub fn draw_help_popup(f: &mut Frame, app: &App) {
    use crate::tui::keybindings::generate_keyboard_shortcuts;

    let area = popup_rect(65, 80, 40, 12, f.area());

    f.render_widget(ratatui::widgets::Clear, area);

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
        generate_keyboard_shortcuts()
    } else {
        generate_status_legend()
    };

    let mut lines = vec![tabs, Line::from("")];
    for line in content {
        lines.push(Line::from(line));
    }
    // Add multi-view hint if applicable
    if app.view_configs.len() > 1 {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Tab/Shift+Tab: switch views",
            Style::default().fg(Color::DarkGray),
        )));
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
