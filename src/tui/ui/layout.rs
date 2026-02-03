//! Layout calculations and text utilities for the TUI.

use once_cell::sync::Lazy;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Pre-computed padding strings to avoid repeated " ".repeat(n) allocations.
/// Covers padding widths 0-100 (column widths are typically < 60).
static PADDING: Lazy<Vec<String>> = Lazy::new(|| (0..=100).map(|n| " ".repeat(n)).collect());

/// Get a padding string of the given width (reuses pre-computed strings).
#[inline]
fn get_padding(width: usize) -> &'static str {
    if width <= 100 {
        &PADDING[width]
    } else {
        // Clamp to the maximum pre-computed width to avoid leaking memory.
        // Columns should never be wider than 100 chars in practice.
        &PADDING[100]
    }
}

use crate::tui::app::{
    COL_IDX_AGENT, COL_IDX_ID, COL_IDX_PR, COL_IDX_PRIORITY, COL_IDX_STATUS, COL_IDX_TIME,
    COL_IDX_TITLE, COL_IDX_VERCEL, NUM_COLUMNS,
};

// Layout constants
pub const PREFIX: &str = "  ";
pub const PREFIX_WIDTH: usize = 2;
pub const SEP: &str = " │ ";
pub const SEP_WIDTH: usize = 3;

pub const COL_MIN_WIDTHS: [usize; NUM_COLUMNS] = [1, 3, 6, 12, 8, 14, 3, 6];
pub const COL_HIDE_ORDER: [usize; 6] = [
    COL_IDX_TIME,
    COL_IDX_VERCEL,
    COL_IDX_AGENT,
    COL_IDX_PR,
    COL_IDX_PRIORITY,
    COL_IDX_ID,
];

/// Column layout configuration with widths and visibility.
#[derive(Clone, Copy)]
pub struct ColumnLayout {
    pub widths: [usize; NUM_COLUMNS],
    pub visible: [bool; NUM_COLUMNS],
    pub row_body_width: usize,
}

impl ColumnLayout {
    pub fn is_visible(&self, idx: usize) -> bool {
        self.visible[idx] && self.widths[idx] > 0
    }
}

/// Compute column layout based on preferred widths and available space.
pub fn compute_column_layout(
    preferred: &[usize; NUM_COLUMNS],
    available_width: u16,
) -> ColumnLayout {
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

/// Calculate the horizontal offset where the title column starts.
pub fn title_column_offset(layout: &ColumnLayout) -> usize {
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

/// Calculate minimum total width for visible columns.
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

/// Calculate the display width of text (accounting for Unicode).
pub fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// Truncate text to a maximum display width.
pub fn truncate_to_width(text: &str, max_width: usize) -> String {
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

/// Truncate text with an ellipsis if it exceeds max width.
pub fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
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

/// Pad text to a specific width with given alignment.
/// Uses pre-computed padding strings to avoid allocations.
pub fn pad_to_width(text: &str, width: usize, alignment: Alignment) -> String {
    let mut trimmed = truncate_to_width(text, width);
    let current = display_width(&trimmed);
    let pad = width.saturating_sub(current);
    match alignment {
        Alignment::Left => {
            trimmed.push_str(get_padding(pad));
            trimmed
        }
        Alignment::Right => format!("{}{}", get_padding(pad), trimmed),
        Alignment::Center => {
            let left = pad / 2;
            let right = pad.saturating_sub(left);
            format!("{}{}{}", get_padding(left), trimmed, get_padding(right))
        }
    }
}

/// Fit a Line to a maximum width by truncating spans.
pub fn fit_line_to_width<'a>(line: Line<'a>, max_width: usize) -> Line<'a> {
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

/// Calculate the display width of a Line.
pub fn line_display_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|span| display_width(span.content.as_ref()))
        .sum()
}

/// Pad a Line to a specific width by adding trailing spaces.
pub fn pad_line_to_width<'a>(mut line: Line<'a>, width: usize) -> Line<'a> {
    let current = line_display_width(&line);
    if current < width {
        line.spans.push(Span::raw(" ".repeat(width - current)));
    }
    line
}

/// Create an ellipsis line centered in the given width.
pub fn ellipsis_line(width: u16) -> Line<'static> {
    let text = pad_to_width("…", width as usize, Alignment::Center);
    Line::from(Span::styled(text, Style::default().fg(Color::DarkGray)))
}

/// Fit lines to an area, adding ellipsis if content is truncated.
pub fn fit_lines_to_area<'a>(
    lines: Vec<Line<'a>>,
    inner: Rect,
    keep_bottom: usize,
) -> Vec<Line<'a>> {
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

/// Render a two-column line with separator.
pub fn render_two_col_line<'a>(
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

/// Calculate a centered popup rectangle within a container.
pub fn popup_rect(
    percent_x: u16,
    percent_y: u16,
    min_width: u16,
    min_height: u16,
    r: Rect,
) -> Rect {
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

/// Truncate a string to max length, adding ellipsis if needed.
pub fn truncate_str(s: &str, max_len: usize) -> String {
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
