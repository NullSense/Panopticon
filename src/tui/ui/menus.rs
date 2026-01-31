//! Menu rendering - sort and filter menus.

use super::layout::{fit_lines_to_area, popup_rect, truncate_str};
use super::status::{priority_config, StatusConfig};
use crate::data::LinearPriority;
use crate::tui::App;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn draw_sort_menu(f: &mut Frame, app: &App) {
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

pub fn draw_filter_menu(f: &mut Frame, app: &App) {
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

