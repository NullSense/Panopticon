//! Modal popup rendering - issue details, links, description.

use super::icons;
use super::layout::{
    display_width, fit_lines_to_area, popup_rect, render_two_col_line, truncate_str, SEP_WIDTH,
};
use super::status::{agent_status_config, linear_status_config, priority_config};
use super::table::highlight_search_matches;
use crate::data::{sort_children, AgentStatus, AgentType, LinearChildRef};
use crate::tui::keybindings::{generate_footer_hints, Mode};
use crate::tui::search::FuzzySearch;
use crate::tui::App;
use pulldown_cmark::{Event, Parser, Tag};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn draw_link_menu(f: &mut Frame, app: &App) {
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

        // Agent session details (if linked)
        if let Some(session) = ws
            .agent_session
            .as_ref()
            .or_else(|| ws.agent_sessions.first())
        {
            push_plain!(Line::from(""));

            // Agent type prefix and status
            let type_prefix = match session.agent_type {
                AgentType::ClaudeCode => "CC",
                AgentType::OpenClaw => "OC",
            };
            let status_cfg = agent_status_config(session.status);

            // Determine detailed status text
            let status_text = match session.status {
                AgentStatus::Running => {
                    if let Some(tool) = &session.activity.current_tool {
                        format!("Running ({})", tool)
                    } else {
                        "Running".to_string()
                    }
                }
                AgentStatus::Idle => "Idle".to_string(),
                AgentStatus::WaitingForInput => "Waiting for input".to_string(),
                AgentStatus::Done => "Done".to_string(),
                AgentStatus::Error => "Error".to_string(),
            };

            // First line: Agent type + status
            items.push(IssueLine::TwoCol(TwoColRow {
                left: vec![
                    Span::styled(
                        format!("  {} ", icons::HEADER_AGENT),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled("Agent: ", label_style),
                    Span::styled(type_prefix.to_string(), active_style),
                ],
                right: vec![
                    Span::styled(format!("{} ", status_cfg.icon), status_cfg.style),
                    Span::styled("Status: ", label_style),
                    Span::styled(status_text, status_cfg.style),
                ],
            }));

            // Second line: Model + Surface/Profile (OpenClaw) or Dir (Claude)
            let model_text = session
                .activity
                .model_short
                .clone()
                .unwrap_or_else(|| "-".to_string());

            // For OpenClaw, show surface and profile; for Claude, show directory
            let right_content = if session.agent_type == AgentType::OpenClaw {
                // Surface info (TUI, Discord, etc.)
                let surface_text = session
                    .activity
                    .surface_label
                    .clone()
                    .or_else(|| session.activity.surface.clone())
                    .unwrap_or_else(|| "-".to_string());
                vec![
                    Span::styled("Via: ", label_style),
                    Span::styled(surface_text, Style::default().fg(Color::Yellow)),
                ]
            } else {
                // Working directory for Claude Code
                let dir_text = session
                    .working_directory
                    .as_ref()
                    .map(|d| {
                        if let Some(home) = dirs::home_dir() {
                            if let Some(home_str) = home.to_str() {
                                if let Some(stripped) = d.strip_prefix(home_str) {
                                    return format!("~{}", stripped);
                                }
                            }
                        }
                        truncate_str(d, 30).to_string()
                    })
                    .unwrap_or_else(|| "-".to_string());
                vec![
                    Span::styled("Dir: ", label_style),
                    Span::styled(dir_text, inactive_style),
                ]
            };

            items.push(IssueLine::TwoCol(TwoColRow {
                left: vec![
                    Span::styled("  ", Style::default()),
                    Span::styled("Model: ", label_style),
                    Span::styled(model_text, Style::default().fg(Color::Magenta)),
                ],
                right: right_content,
            }));

            // Third line: Profile (OpenClaw) or Branch (both)
            if session.agent_type == AgentType::OpenClaw {
                // Show profile and working directory for OpenClaw
                let profile_text = session
                    .activity
                    .profile
                    .clone()
                    .unwrap_or_else(|| "default".to_string());
                let dir_text = session
                    .working_directory
                    .as_ref()
                    .map(|d| {
                        if let Some(home) = dirs::home_dir() {
                            if let Some(home_str) = home.to_str() {
                                if let Some(stripped) = d.strip_prefix(home_str) {
                                    return format!("~{}", stripped);
                                }
                            }
                        }
                        truncate_str(d, 25).to_string()
                    })
                    .unwrap_or_else(|| "-".to_string());

                items.push(IssueLine::TwoCol(TwoColRow {
                    left: vec![
                        Span::styled("  ", Style::default()),
                        Span::styled("Profile: ", label_style),
                        Span::styled(profile_text, Style::default().fg(Color::Green)),
                    ],
                    right: vec![
                        Span::styled("Dir: ", label_style),
                        Span::styled(dir_text, inactive_style),
                    ],
                }));
            }

            // Fourth line: Git branch + current target (if available)
            if session.git_branch.is_some() || session.activity.current_target.is_some() {
                let branch_text = session
                    .git_branch
                    .clone()
                    .unwrap_or_else(|| "-".to_string());
                let target_text = session
                    .activity
                    .current_target
                    .as_ref()
                    .map(|t| truncate_str(t, 30).to_string())
                    .unwrap_or_default();

                let mut row = TwoColRow {
                    left: vec![
                        Span::styled("  ", Style::default()),
                        Span::styled("Branch: ", label_style),
                        Span::styled(
                            truncate_str(&branch_text, 25).to_string(),
                            Style::default().fg(Color::Blue),
                        ),
                    ],
                    right: vec![],
                };

                if !target_text.is_empty() {
                    row.right = vec![
                        Span::styled("Target: ", label_style),
                        Span::styled(target_text, inactive_style),
                    ];
                }

                items.push(IssueLine::TwoCol(row));
            }
        }

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
pub fn draw_links_popup(f: &mut Frame, app: &App) {
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
        let has_session = !ws.agent_sessions.is_empty() || ws.agent_session.is_some();

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
                if !ws.agent_sessions.is_empty() || ws.agent_session.is_some() {
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

pub fn draw_description_modal(f: &mut Frame, app: &App) {
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
    let lines: Vec<Line> = if let Some(ws) = app.modal_issue().or_else(|| app.selected_workstream())
    {
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
