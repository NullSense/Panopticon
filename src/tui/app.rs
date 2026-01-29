use crate::config::Config;
use crate::data::{AppState, LinearCycle, LinearPriority, LinearStatus, VisualItem, Workstream};
use crate::integrations;
use anyhow::Result;
use chrono::Utc;
use std::collections::{HashMap, HashSet};

/// Braille spinner frames for loading animation
pub const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Search match result with excerpt
#[derive(Clone)]
pub struct SearchMatch {
    pub excerpt: String,
    #[allow(dead_code)]
    pub match_in: String, // "title", "description", or "id" - for potential future use
}

/// Column indices for resize mode
pub const COL_IDX_STATUS: usize = 0;
pub const COL_IDX_PRIORITY: usize = 1;
pub const COL_IDX_ID: usize = 2;
pub const COL_IDX_TITLE: usize = 3;
pub const COL_IDX_PR: usize = 4;
pub const COL_IDX_AGENT: usize = 5;
pub const COL_IDX_VERCEL: usize = 6;
pub const COL_IDX_TIME: usize = 7;
pub const NUM_COLUMNS: usize = 8;

/// Column names for resize mode display
pub const COLUMN_NAMES: [&str; NUM_COLUMNS] = [
    "Status", "Priority", "ID", "Title", "PR", "Agent", "Vercel", "Time"
];

pub struct App {
    pub config: Config,
    pub state: AppState,
    pub filtered_indices: Vec<usize>,
    pub visual_items: Vec<VisualItem>,
    pub visual_selected: usize,
    pub show_help: bool,
    pub help_tab: usize, // 0 = shortcuts, 1 = status legend
    pub show_link_menu: bool,
    pub show_sort_menu: bool,
    pub show_preview: bool,
    pub search_all: bool,
    pub error_message: Option<String>,
    pub is_loading: bool,
    pub spinner_frame: usize,
    /// Search excerpts for workstreams that matched in description
    pub search_excerpts: HashMap<usize, SearchMatch>,
    /// Resize mode active
    pub resize_mode: bool,
    /// Currently selected column in resize mode
    pub resize_column_idx: usize,
    /// Column widths (Status, Priority, ID, Title, PR, Agent, Vercel, Time)
    pub column_widths: [usize; NUM_COLUMNS],
    /// Filter menu visible
    pub show_filter_menu: bool,
    /// Filter by cycle IDs (empty = show all)
    pub filter_cycles: HashSet<String>,
    /// Filter by priorities (empty = show all)
    pub filter_priorities: HashSet<LinearPriority>,
    /// Available cycles (populated from workstreams)
    pub available_cycles: Vec<LinearCycle>,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: AppState::default(),
            filtered_indices: vec![],
            visual_items: vec![],
            visual_selected: 0,
            show_help: false,
            help_tab: 0,
            show_link_menu: false,
            show_sort_menu: false,
            show_preview: false,
            search_all: false,
            error_message: None,
            is_loading: false,
            spinner_frame: 0,
            search_excerpts: HashMap::new(),
            resize_mode: false,
            resize_column_idx: COL_IDX_TITLE, // Start with title column selected
            // Default widths: Status=1, Priority=3, ID=10, Title=26, PR=12, Agent=10, Vercel=3, Time=6
            column_widths: [1, 3, 10, 26, 12, 10, 3, 6],
            show_filter_menu: false,
            filter_cycles: HashSet::new(),
            filter_priorities: HashSet::new(),
            available_cycles: Vec::new(),
        }
    }

    /// Advance spinner frame (call on tick while loading)
    pub fn tick_spinner(&mut self) {
        if self.is_loading {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
        }
    }

    /// Get current spinner character
    pub fn spinner_char(&self) -> char {
        SPINNER_FRAMES[self.spinner_frame]
    }

    /// Rebuild the visual items list (call after any state change)
    pub fn rebuild_visual_items(&mut self) {
        // Remember if we were on a section header
        let was_on_header = matches!(
            self.visual_items.get(self.visual_selected),
            Some(VisualItem::SectionHeader(_))
        );
        let previous_status = self.selected_section();

        self.visual_items = self.state.build_visual_items(&self.filtered_indices);

        // Ensure selection is valid
        if self.visual_items.is_empty() {
            self.visual_selected = 0;
            return;
        }

        // Clamp to valid range
        if self.visual_selected >= self.visual_items.len() {
            self.visual_selected = self.visual_items.len().saturating_sub(1);
        }

        // If we were on a header, try to stay on same section's header
        if was_on_header {
            if let Some(status) = previous_status {
                for (idx, item) in self.visual_items.iter().enumerate() {
                    if let VisualItem::SectionHeader(s) = item {
                        if *s == status {
                            self.visual_selected = idx;
                            return;
                        }
                    }
                }
            }
        }

        // Otherwise snap to nearest workstream
        self.snap_to_workstream(1);
    }

    /// Snap selection to nearest workstream in given direction
    fn snap_to_workstream(&mut self, direction: i32) {
        let len = self.visual_items.len();
        if len == 0 {
            return;
        }

        let mut pos = self.visual_selected;
        for _ in 0..len {
            if let Some(VisualItem::Workstream(_)) = self.visual_items.get(pos) {
                self.visual_selected = pos;
                return;
            }
            if direction > 0 {
                pos = (pos + 1) % len;
            } else {
                pos = pos.checked_sub(1).unwrap_or(len - 1);
            }
        }
    }

    pub async fn refresh(&mut self) -> Result<()> {
        self.is_loading = true;

        match integrations::fetch_workstreams(&self.config).await {
            Ok(workstreams) => {
                self.state.workstreams = workstreams;
                self.state.last_refresh = Some(Utc::now());

                // Extract available cycles from workstreams
                self.update_available_cycles();

                // Apply filters
                self.apply_filters();
                self.rebuild_visual_items();
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("Refresh failed: {}", e));
                tracing::error!("Failed to fetch workstreams: {}", e);
            }
        }

        self.is_loading = false;
        Ok(())
    }

    /// Extract unique cycles from workstreams
    fn update_available_cycles(&mut self) {
        let mut seen_ids = HashSet::new();
        self.available_cycles.clear();

        for ws in &self.state.workstreams {
            if let Some(cycle) = &ws.linear_issue.cycle {
                if !seen_ids.contains(&cycle.id) {
                    seen_ids.insert(cycle.id.clone());
                    self.available_cycles.push(cycle.clone());
                }
            }
        }

        // Sort by cycle number (most recent first)
        self.available_cycles.sort_by(|a, b| b.number.cmp(&a.number));
    }

    pub async fn on_tick(&mut self) {
        // Advance spinner animation
        self.tick_spinner();
    }

    pub fn enter_search(&mut self, search_all: bool) {
        self.state.search_mode = true;
        self.state.search_query.clear();
        self.search_all = search_all;
    }

    pub fn exit_search(&mut self) {
        self.state.search_mode = false;
        self.state.search_query.clear();
        self.search_excerpts.clear();
        self.filtered_indices = (0..self.state.workstreams.len()).collect();
        self.rebuild_visual_items();
    }

    pub fn update_search(&mut self) {
        self.search_excerpts.clear();

        if self.state.search_query.is_empty() {
            self.filtered_indices = (0..self.state.workstreams.len()).collect();
        } else {
            let query = &self.state.search_query;
            let query_lower = query.to_lowercase();

            let mut results: Vec<(usize, u32, Option<SearchMatch>)> = Vec::new();

            for (i, ws) in self.state.workstreams.iter().enumerate() {
                let issue = &ws.linear_issue;

                // Check identifier (exact substring match)
                let id_score = if issue.identifier.to_lowercase().contains(&query_lower) {
                    Some(1000u32)
                } else {
                    None
                };

                // Check title (case-insensitive substring match)
                let title_lower = issue.title.to_lowercase();
                let title_score = if title_lower.contains(&query_lower) {
                    // Score based on how much of the title is matched
                    let ratio = query.len() as f32 / issue.title.len() as f32;
                    Some((ratio * 500.0) as u32 + 500)
                } else {
                    None
                };

                // Check description (case-insensitive substring match)
                let (desc_score, desc_excerpt) = if let Some(desc) = &issue.description {
                    let desc_lower = desc.to_lowercase();
                    if desc_lower.contains(&query_lower) {
                        let excerpt = create_excerpt(desc, query, 80);
                        let ratio = query.len() as f32 / desc.len().min(200) as f32;
                        (Some((ratio * 200.0) as u32 + 100), Some(excerpt))
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };

                // Combine scores
                let best_score = [id_score, title_score, desc_score]
                    .into_iter()
                    .flatten()
                    .max();

                if let Some(score) = best_score {
                    let search_match = if desc_excerpt.is_some() && id_score.is_none() && title_score.is_none() {
                        // Only show excerpt if the match was in description (not title/id)
                        desc_excerpt.map(|excerpt| SearchMatch {
                            excerpt,
                            match_in: "description".to_string(),
                        })
                    } else if desc_excerpt.is_some() && title_score.is_none() {
                        // Also show if it matched in description even if ID matched
                        desc_excerpt.map(|excerpt| SearchMatch {
                            excerpt,
                            match_in: "description".to_string(),
                        })
                    } else {
                        None
                    };

                    results.push((i, score, search_match));
                }
            }

            // Sort by score (higher is better)
            results.sort_by(|a, b| b.1.cmp(&a.1));

            // Store filtered indices and excerpts
            self.filtered_indices = results.iter().map(|(i, _, _)| *i).collect();

            for (idx, _, search_match) in results {
                if let Some(sm) = search_match {
                    self.search_excerpts.insert(idx, sm);
                }
            }
        }

        self.rebuild_visual_items();
    }

    pub fn confirm_search(&mut self) {
        self.state.search_mode = false;
        // Keep filtered results
    }

    pub fn move_selection(&mut self, delta: i32) {
        let len = self.visual_items.len();
        if len == 0 {
            return;
        }

        let mut pos = self.visual_selected;
        let steps = delta.unsigned_abs() as usize;

        for _ in 0..steps {
            // Move in the direction
            loop {
                if delta > 0 {
                    if pos >= len - 1 {
                        break; // At end
                    }
                    pos += 1;
                } else {
                    if pos == 0 {
                        break; // At start
                    }
                    pos -= 1;
                }

                // Stop on any item (workstream or section header)
                // This allows navigating to collapsed sections to expand them
                break;
            }
        }

        self.visual_selected = pos;
    }

    pub fn go_to_top(&mut self) {
        self.visual_selected = 0;
        self.snap_to_workstream(1);
    }

    pub fn go_to_bottom(&mut self) {
        if !self.visual_items.is_empty() {
            self.visual_selected = self.visual_items.len() - 1;
            self.snap_to_workstream(-1);
        }
    }

    pub fn page_down(&mut self) {
        self.move_selection(10);
    }

    pub fn page_up(&mut self) {
        self.move_selection(-10);
    }

    /// Get the currently selected workstream
    pub fn selected_workstream(&self) -> Option<&Workstream> {
        match self.visual_items.get(self.visual_selected) {
            Some(VisualItem::Workstream(idx)) => self.state.workstreams.get(*idx),
            _ => None,
        }
    }

    /// Get the currently selected section (for section headers)
    pub fn selected_section(&self) -> Option<LinearStatus> {
        match self.visual_items.get(self.visual_selected) {
            Some(VisualItem::SectionHeader(status)) => Some(*status),
            Some(VisualItem::Workstream(idx)) => {
                self.state.workstreams.get(*idx).map(|ws| ws.linear_issue.status)
            }
            None => None,
        }
    }

    pub async fn open_primary_link(&self) -> Result<()> {
        if let Some(ws) = self.selected_workstream() {
            open_linear_url(&ws.linear_issue.url)?;
        }
        Ok(())
    }

    pub async fn open_linear_link(&self) -> Result<()> {
        if let Some(ws) = self.selected_workstream() {
            open_linear_url(&ws.linear_issue.url)?;
        }
        Ok(())
    }

    pub async fn open_github_link(&self) -> Result<()> {
        if let Some(ws) = self.selected_workstream() {
            if let Some(pr) = &ws.github_pr {
                open_url(&pr.url)?;
            }
        }
        Ok(())
    }

    pub async fn open_vercel_link(&self) -> Result<()> {
        if let Some(ws) = self.selected_workstream() {
            if let Some(deploy) = &ws.vercel_deployment {
                open_url(&deploy.url)?;
            }
        }
        Ok(())
    }

    pub fn open_link_menu(&mut self) {
        self.show_link_menu = !self.show_link_menu;
    }

    pub async fn teleport_to_session(&self) -> Result<()> {
        if let Some(ws) = self.selected_workstream() {
            if let Some(session) = &ws.agent_session {
                integrations::claude::focus_session_window(session).await?;
            }
        }
        Ok(())
    }

    pub fn toggle_preview(&mut self) {
        self.show_preview = !self.show_preview;
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    // Section collapse/expand
    pub fn collapse_current_section(&mut self) {
        if let Some(status) = self.selected_section() {
            self.state.collapsed_sections.insert(status);
            self.rebuild_visual_items();
        }
    }

    pub fn expand_current_section(&mut self) {
        if let Some(status) = self.selected_section() {
            self.state.collapsed_sections.remove(&status);
            self.rebuild_visual_items();
        }
    }

    // Sorting
    pub fn toggle_sort_menu(&mut self) {
        self.show_sort_menu = !self.show_sort_menu;
    }

    pub fn set_sort_mode(&mut self, mode: crate::data::SortMode) {
        self.state.sort_mode = mode;
        self.show_sort_menu = false;
        self.rebuild_visual_items();
    }

    // Resize mode
    pub fn toggle_resize_mode(&mut self) {
        self.resize_mode = !self.resize_mode;
    }

    pub fn exit_resize_mode(&mut self) {
        self.resize_mode = false;
    }

    pub fn resize_next_column(&mut self) {
        self.resize_column_idx = (self.resize_column_idx + 1) % NUM_COLUMNS;
    }

    pub fn resize_prev_column(&mut self) {
        self.resize_column_idx = if self.resize_column_idx == 0 {
            NUM_COLUMNS - 1
        } else {
            self.resize_column_idx - 1
        };
    }

    pub fn resize_column_wider(&mut self) {
        let width = &mut self.column_widths[self.resize_column_idx];
        *width = (*width + 1).min(50); // Max width 50
    }

    pub fn resize_column_narrower(&mut self) {
        let width = &mut self.column_widths[self.resize_column_idx];
        *width = (*width).saturating_sub(1).max(1); // Min width 1
    }

    pub fn current_resize_column_name(&self) -> &'static str {
        COLUMN_NAMES[self.resize_column_idx]
    }

    // Filter methods
    pub fn toggle_filter_menu(&mut self) {
        self.show_filter_menu = !self.show_filter_menu;
    }

    pub fn toggle_cycle_filter(&mut self, cycle_idx: usize) {
        if cycle_idx == 0 {
            // "All cycles" - clear filter
            self.filter_cycles.clear();
        } else if let Some(cycle) = self.available_cycles.get(cycle_idx - 1) {
            let id = cycle.id.clone();
            if self.filter_cycles.contains(&id) {
                self.filter_cycles.remove(&id);
            } else {
                self.filter_cycles.insert(id);
            }
        }
        self.apply_filters();
        self.rebuild_visual_items();
    }

    pub fn toggle_priority_filter(&mut self, priority: LinearPriority) {
        if self.filter_priorities.contains(&priority) {
            self.filter_priorities.remove(&priority);
        } else {
            self.filter_priorities.insert(priority);
        }
        self.apply_filters();
        self.rebuild_visual_items();
    }

    pub fn clear_all_filters(&mut self) {
        self.filter_cycles.clear();
        self.filter_priorities.clear();
        self.apply_filters();
        self.rebuild_visual_items();
    }

    pub fn select_all_filters(&mut self) {
        // Select all cycles
        self.filter_cycles = self.available_cycles.iter().map(|c| c.id.clone()).collect();
        // Select all priorities
        self.filter_priorities = [
            LinearPriority::Urgent,
            LinearPriority::High,
            LinearPriority::Medium,
            LinearPriority::Low,
            LinearPriority::NoPriority,
        ].into_iter().collect();
        self.apply_filters();
        self.rebuild_visual_items();
    }

    /// Apply filters to create filtered_indices
    pub fn apply_filters(&mut self) {
        self.filtered_indices = self.state.workstreams
            .iter()
            .enumerate()
            .filter(|(_, ws)| {
                // Cycle filter (empty = show all)
                if !self.filter_cycles.is_empty() {
                    match &ws.linear_issue.cycle {
                        Some(cycle) => {
                            if !self.filter_cycles.contains(&cycle.id) {
                                return false;
                            }
                        }
                        None => return false, // No cycle, filtered out
                    }
                }

                // Priority filter (empty = show all)
                if !self.filter_priorities.is_empty() {
                    if !self.filter_priorities.contains(&ws.linear_issue.priority) {
                        return false;
                    }
                }

                true
            })
            .map(|(idx, _)| idx)
            .collect();
    }

    /// Check if any filter is active
    #[allow(dead_code)]
    pub fn has_active_filters(&self) -> bool {
        !self.filter_cycles.is_empty() || !self.filter_priorities.is_empty()
    }
}

fn open_url(url: &str) -> Result<()> {
    // Use xdg-open on Linux, which works in WSL
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .or_else(|_| {
            // Fallback to wslview for WSL
            std::process::Command::new("wslview").arg(url).spawn()
        })?;
    Ok(())
}

/// Open a Linear URL, preferring the desktop app via linear:// protocol
fn open_linear_url(url: &str) -> Result<()> {
    // Convert https://linear.app/... to linear://...
    let linear_url = url.replace("https://linear.app/", "linear://");

    // Try to open with linear:// protocol (desktop app)
    let result = std::process::Command::new("xdg-open")
        .arg(&linear_url)
        .spawn();

    match result {
        Ok(_) => Ok(()),
        Err(_) => {
            // Fallback to browser URL if desktop app not available
            open_url(url)
        }
    }
}

/// Create an excerpt from text around a search query
fn create_excerpt(text: &str, query: &str, max_len: usize) -> String {
    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();

    // Find the position of the query in the text
    if let Some(pos) = text_lower.find(&query_lower) {
        // Calculate start position (with some context before)
        let context_before = 20;
        let start = pos.saturating_sub(context_before);

        // Find actual char boundaries
        let start = text
            .char_indices()
            .find(|(i, _)| *i >= start)
            .map(|(i, _)| i)
            .unwrap_or(0);

        // Get the excerpt
        let excerpt: String = text.chars().skip(start).take(max_len).collect();

        // Add ellipsis if truncated
        let prefix = if start > 0 { "..." } else { "" };
        let suffix = if start + max_len < text.len() { "..." } else { "" };

        format!("{}{}{}", prefix, excerpt.trim(), suffix)
    } else {
        // Fallback: just take the beginning
        let excerpt: String = text.chars().take(max_len).collect();
        if text.len() > max_len {
            format!("{}...", excerpt.trim())
        } else {
            excerpt
        }
    }
}
