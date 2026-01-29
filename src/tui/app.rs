use crate::config::Config;
use crate::data::{AppState, LinearCycle, LinearPriority, LinearStatus, VisualItem, Workstream};
use crate::integrations;
use crate::tui::search::FuzzySearch;
use anyhow::Result;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Braille spinner frames for loading animation
pub const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Search match result with excerpt
#[derive(Clone)]
pub struct SearchMatch {
    pub excerpt: String,
    #[allow(dead_code)]
    pub match_in: String, // "title", "description", or "id" - for potential future use
}

/// Progress tracking for background refresh
#[derive(Clone, Debug)]
pub struct RefreshProgress {
    pub total_issues: usize,
    pub completed: usize,
    pub current_stage: String,
}

/// Result from background refresh task
pub enum RefreshResult {
    /// Progress update
    Progress(RefreshProgress),
    /// Single workstream completed
    Workstream(Workstream),
    /// Refresh completed successfully
    Complete,
    /// Error occurred
    Error(String),
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

/// Active modal state - only one modal can be active at a time
/// This enum consolidates the previous 7 boolean modal flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModalState {
    #[default]
    None,
    Help { tab: usize },
    LinkMenu { show_links_popup: bool },
    SortMenu,
    FilterMenu,
    Description,
    Resize,
}

impl ModalState {
    pub fn is_none(&self) -> bool {
        matches!(self, ModalState::None)
    }
}

pub struct App {
    pub config: Arc<Config>,
    pub state: AppState,
    pub filtered_indices: Vec<usize>,
    pub visual_items: Vec<VisualItem>,
    pub visual_selected: usize,

    // Modal state - single enum replacing 7 booleans
    pub modal: ModalState,

    // UI state
    pub show_preview: bool,
    pub error_message: Option<String>,
    pub is_loading: bool,
    pub spinner_frame: usize,
    pub column_widths: [usize; NUM_COLUMNS],
    pub resize_column_idx: usize,

    // Search state
    pub search_all: bool,
    pub search_excerpts: HashMap<usize, SearchMatch>,

    // Filter state
    pub filter_cycles: HashSet<String>,
    pub filter_priorities: HashSet<LinearPriority>,
    pub available_cycles: Vec<LinearCycle>,
    pub show_sub_issues: bool,

    // Modal navigation state
    pub selected_child_idx: Option<usize>,
    pub issue_navigation_stack: Vec<String>,
    pub modal_issue_id: Option<String>,
    pub sub_issues_scroll: usize,
    pub description_scroll: usize,
    pub modal_search_mode: bool,
    pub modal_search_query: String,

    /// Channel receiver for background refresh results
    pub refresh_rx: Option<mpsc::Receiver<RefreshResult>>,
    /// Progress tracking for incremental updates
    pub refresh_progress: Option<RefreshProgress>,
}

// Modal state accessors (backward-compatible interface)
impl App {
    pub fn show_help(&self) -> bool {
        matches!(self.modal, ModalState::Help { .. })
    }

    pub fn help_tab(&self) -> usize {
        match self.modal {
            ModalState::Help { tab } => tab,
            _ => 0,
        }
    }

    pub fn show_link_menu(&self) -> bool {
        matches!(self.modal, ModalState::LinkMenu { .. })
    }

    pub fn show_links_popup(&self) -> bool {
        matches!(self.modal, ModalState::LinkMenu { show_links_popup: true })
    }

    pub fn show_sort_menu(&self) -> bool {
        matches!(self.modal, ModalState::SortMenu)
    }

    pub fn show_filter_menu(&self) -> bool {
        matches!(self.modal, ModalState::FilterMenu)
    }

    pub fn show_description_modal(&self) -> bool {
        matches!(self.modal, ModalState::Description)
    }

    pub fn resize_mode(&self) -> bool {
        matches!(self.modal, ModalState::Resize)
    }
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            state: AppState::default(),
            filtered_indices: vec![],
            visual_items: vec![],
            visual_selected: 0,
            modal: ModalState::None,
            show_preview: false,
            error_message: None,
            is_loading: false,
            spinner_frame: 0,
            // Default widths: Status=1, Priority=3, ID=10, Title=26, PR=12, Agent=10, Vercel=3, Time=6
            column_widths: [1, 3, 10, 26, 12, 10, 3, 6],
            resize_column_idx: COL_IDX_TITLE,
            search_all: false,
            search_excerpts: HashMap::new(),
            filter_cycles: HashSet::new(),
            filter_priorities: HashSet::new(),
            available_cycles: Vec::new(),
            show_sub_issues: true,
            selected_child_idx: None,
            issue_navigation_stack: Vec::new(),
            modal_issue_id: None,
            sub_issues_scroll: 0,
            description_scroll: 0,
            modal_search_mode: false,
            modal_search_query: String::new(),
            refresh_rx: None,
            refresh_progress: None,
        }
    }

    /// Process a message and update app state (Elm Architecture update function).
    ///
    /// Returns `Ok(true)` if the app should quit, `Ok(false)` to continue.
    pub async fn update(&mut self, msg: super::Message) -> anyhow::Result<bool> {
        use super::Message;
        match msg {
            // ─────────────────────────────────────────────────────────────────
            // App lifecycle
            // ─────────────────────────────────────────────────────────────────
            Message::Quit => return Ok(true),
            Message::Refresh => self.start_background_refresh(),

            // ─────────────────────────────────────────────────────────────────
            // Navigation
            // ─────────────────────────────────────────────────────────────────
            Message::MoveUp => self.move_selection(-1),
            Message::MoveDown => self.move_selection(1),
            Message::GotoTop => self.go_to_top(),
            Message::GotoBottom => self.go_to_bottom(),
            Message::PageUp => self.page_up(),
            Message::PageDown => self.page_down(),

            // ─────────────────────────────────────────────────────────────────
            // Selection actions
            // ─────────────────────────────────────────────────────────────────
            Message::ExpandSection => self.expand_current_section(),
            Message::CollapseSection => self.collapse_current_section(),
            Message::OpenPrimaryLink => {
                self.open_primary_link().await?;
            }
            Message::OpenLinkMenu => self.open_link_menu(),
            Message::TeleportToSession => {
                self.teleport_to_session().await?;
                // Close modal and clear navigation if in link menu
                if self.show_link_menu() {
                    self.modal = ModalState::None;
                    self.clear_navigation();
                }
            }

            // ─────────────────────────────────────────────────────────────────
            // Search mode
            // ─────────────────────────────────────────────────────────────────
            Message::EnterSearch { search_all } => self.enter_search(search_all),
            Message::ExitSearch => self.exit_search(),
            Message::ConfirmSearch => self.confirm_search(),
            Message::SearchInput(c) => {
                self.state.search_query.push(c);
                self.update_search();
            }
            Message::SearchBackspace => {
                self.state.search_query.pop();
                self.update_search();
            }

            // ─────────────────────────────────────────────────────────────────
            // Modal toggles
            // ─────────────────────────────────────────────────────────────────
            Message::ToggleHelp => self.toggle_help(),
            Message::ToggleSortMenu => self.toggle_sort_menu(),
            Message::ToggleFilterMenu => self.toggle_filter_menu(),
            Message::TogglePreview => self.toggle_preview(),
            Message::ToggleResizeMode => self.toggle_resize_mode(),
            Message::CloseModal => self.modal = ModalState::None,

            // ─────────────────────────────────────────────────────────────────
            // Help modal
            // ─────────────────────────────────────────────────────────────────
            Message::SetHelpTab(tab) => {
                self.modal = ModalState::Help { tab };
            }

            // ─────────────────────────────────────────────────────────────────
            // Sort modal
            // ─────────────────────────────────────────────────────────────────
            Message::SetSortMode(mode) => self.set_sort_mode(mode),

            // ─────────────────────────────────────────────────────────────────
            // Filter modal
            // ─────────────────────────────────────────────────────────────────
            Message::ToggleCycleFilter(idx) => self.toggle_cycle_filter(idx),
            Message::TogglePriorityFilter(priority) => self.toggle_priority_filter(priority),
            Message::ToggleSubIssues => self.toggle_sub_issues(),
            Message::ClearAllFilters => self.clear_all_filters(),
            Message::SelectAllFilters => self.select_all_filters(),

            // ─────────────────────────────────────────────────────────────────
            // Resize mode
            // ─────────────────────────────────────────────────────────────────
            Message::ExitResizeMode => self.exit_resize_mode(),
            Message::ResizeColumnNarrower => self.resize_column_narrower(),
            Message::ResizeColumnWider => self.resize_column_wider(),
            Message::ResizeNextColumn => self.resize_next_column(),
            Message::ResizePrevColumn => self.resize_prev_column(),

            // ─────────────────────────────────────────────────────────────────
            // Link menu modal
            // ─────────────────────────────────────────────────────────────────
            Message::OpenLinksPopup => {
                self.modal = ModalState::LinkMenu { show_links_popup: true };
            }
            Message::CloseLinksPopup => {
                self.modal = ModalState::LinkMenu { show_links_popup: false };
            }
            Message::OpenLinearLink => {
                self.open_linear_link().await?;
                self.modal = ModalState::None;
                self.clear_navigation();
            }
            Message::OpenGithubLink => {
                self.open_github_link().await?;
                self.modal = ModalState::None;
                self.clear_navigation();
            }
            Message::OpenVercelLink => {
                self.open_vercel_link().await?;
                self.modal = ModalState::None;
                self.clear_navigation();
            }

            // ─────────────────────────────────────────────────────────────────
            // Link menu navigation
            // ─────────────────────────────────────────────────────────────────
            Message::NextChildIssue => self.next_child_issue(),
            Message::PrevChildIssue => self.prev_child_issue(),
            Message::NavigateToSelectedChild => {
                if self.selected_child_idx.is_some() {
                    if !self.navigate_to_selected_child() {
                        // Child not in workstreams, open in browser
                        self.open_selected_child_issue()?;
                        self.modal = ModalState::None;
                        self.clear_navigation();
                    }
                }
            }
            Message::NavigateToParent => {
                if !self.navigate_to_parent() {
                    // Parent not in workstreams, open in browser
                    self.open_parent_issue()?;
                    self.modal = ModalState::None;
                    self.clear_navigation();
                }
            }
            Message::OpenDocument(idx) => {
                self.open_document(idx)?;
                self.modal = ModalState::None;
                self.clear_navigation();
            }
            Message::OpenDescriptionModal => self.open_description_modal(),
            Message::NavigateToChild(idx) => {
                if !self.navigate_to_child(idx) {
                    // Child not in workstreams, open in browser
                    self.open_child_issue(idx)?;
                    self.modal = ModalState::None;
                    self.clear_navigation();
                }
            }

            // ─────────────────────────────────────────────────────────────────
            // Description modal
            // ─────────────────────────────────────────────────────────────────
            Message::ScrollDescription(delta) => {
                if delta == -10000 {
                    // Special value for "go to top"
                    self.description_scroll = 0;
                } else {
                    self.scroll_description(delta);
                }
            }
            Message::CloseDescriptionModal => self.close_description_modal(),

            // ─────────────────────────────────────────────────────────────────
            // Modal search
            // ─────────────────────────────────────────────────────────────────
            Message::EnterModalSearch => self.enter_modal_search(),
            Message::ExitModalSearch => self.exit_modal_search(),
            Message::ModalSearchInput(c) => {
                self.modal_search_query.push(c);
            }
            Message::ModalSearchBackspace => {
                self.modal_search_query.pop();
            }
            Message::ClearModalSearch => self.clear_modal_search(),

            // ─────────────────────────────────────────────────────────────────
            // Navigation stack
            // ─────────────────────────────────────────────────────────────────
            Message::NavigateBack => {
                if !self.navigate_back() {
                    // Stack empty, close the menu
                    self.modal = ModalState::None;
                    self.clear_navigation();
                }
            }

            // ─────────────────────────────────────────────────────────────────
            // No-op
            // ─────────────────────────────────────────────────────────────────
            Message::None => {}
        }
        Ok(false)
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

                // Calculate optimal column widths based on content
                self.calculate_optimal_widths();

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

    /// Start refresh in background (non-blocking)
    pub fn start_background_refresh(&mut self) {
        // Don't start another refresh if one is already in progress
        if self.refresh_rx.is_some() {
            return;
        }

        self.is_loading = true;
        self.state.workstreams.clear(); // Clear existing data for incremental loading
        self.refresh_progress = Some(RefreshProgress {
            total_issues: 0,
            completed: 0,
            current_stage: "Fetching Linear issues...".to_string(),
        });

        let (tx, rx) = mpsc::channel(100);
        self.refresh_rx = Some(rx);

        let config = Arc::clone(&self.config);

        // Spawn background task
        tokio::spawn(async move {
            if let Err(e) = integrations::fetch_workstreams_incremental(&config, tx.clone()).await {
                let _ = tx.send(RefreshResult::Error(e.to_string())).await;
            }
        });
    }

    /// Poll for refresh results (non-blocking, call from event loop tick)
    pub fn poll_refresh(&mut self) -> bool {
        // Take ownership of the receiver to avoid borrow issues
        let Some(mut rx) = self.refresh_rx.take() else {
            return false;
        };

        let mut completed = false;
        let mut should_restore = true;

        // Non-blocking receive of all available results
        while let Ok(result) = rx.try_recv() {
            match result {
                RefreshResult::Progress(progress) => {
                    self.refresh_progress = Some(progress);
                }
                RefreshResult::Workstream(ws) => {
                    self.state.workstreams.push(ws);
                    if let Some(ref mut p) = self.refresh_progress {
                        p.completed += 1;
                    }
                }
                RefreshResult::Complete => {
                    self.is_loading = false;
                    self.refresh_progress = None;
                    self.state.last_refresh = Some(Utc::now());
                    self.update_available_cycles();
                    self.calculate_optimal_widths();
                    self.apply_filters();
                    self.rebuild_visual_items();
                    self.error_message = None;
                    completed = true;
                    should_restore = false;
                }
                RefreshResult::Error(msg) => {
                    self.is_loading = false;
                    self.refresh_progress = None;
                    self.error_message = Some(format!("Refresh failed: {}", msg));
                    completed = true;
                    should_restore = false;
                }
            }
        }

        // Restore receiver if not completed
        if should_restore {
            self.refresh_rx = Some(rx);
        }

        completed
    }

    /// Calculate optimal column widths based on content
    pub fn calculate_optimal_widths(&mut self) {
        // Default min widths
        let mut max_id_len = 6usize; // "ID" header + padding
        let mut max_title_len = 8usize; // "Title" header + padding
        let mut max_pr_len = 8usize; // "PR" header + padding
        let mut max_agent_len = 8usize; // "Agent" header + padding

        for ws in &self.state.workstreams {
            let issue = &ws.linear_issue;

            // ID column
            max_id_len = max_id_len.max(issue.identifier.len());

            // Title column (cap at reasonable max to prevent single long title from dominating)
            max_title_len = max_title_len.max(issue.title.chars().count().min(50));

            // PR column
            if let Some(pr) = &ws.github_pr {
                let pr_text = format!("PR#{}", pr.number);
                max_pr_len = max_pr_len.max(pr_text.len() + 2); // +2 for icon
            }

            // Agent column
            if let Some(session) = &ws.agent_session {
                let agent_text = session.status.label();
                max_agent_len = max_agent_len.max(agent_text.len() + 2); // +2 for icon
            }
        }

        // Apply calculated widths with some padding
        self.column_widths[COL_IDX_ID] = max_id_len + 1;
        self.column_widths[COL_IDX_TITLE] = max_title_len.min(40); // Cap title at 40
        self.column_widths[COL_IDX_PR] = max_pr_len.min(15);
        self.column_widths[COL_IDX_AGENT] = max_agent_len.min(12);

        // Status, Priority, Vercel, and Time have fixed widths
        // (already set in defaults, no need to recalculate)
    }

    /// Recalculate column widths for given terminal width
    pub fn recalculate_column_widths(&mut self, terminal_width: u16) {
        // Calculate fixed widths (status, priority, vercel, time, separators)
        let fixed_widths = self.column_widths[COL_IDX_STATUS]
            + self.column_widths[COL_IDX_PRIORITY]
            + self.column_widths[COL_IDX_VERCEL]
            + self.column_widths[COL_IDX_TIME]
            + 24; // Separators and padding

        let available = (terminal_width as usize).saturating_sub(fixed_widths);

        // Distribute remaining space to ID, Title, PR, Agent
        // Title gets 50%, ID gets 20%, PR gets 15%, Agent gets 15%
        if available > 40 {
            let title_width = (available * 50 / 100).min(50).max(15);
            let id_width = (available * 20 / 100).min(15).max(6);
            let pr_width = (available * 15 / 100).min(15).max(8);
            let agent_width = (available * 15 / 100).min(12).max(8);

            self.column_widths[COL_IDX_ID] = id_width;
            self.column_widths[COL_IDX_TITLE] = title_width;
            self.column_widths[COL_IDX_PR] = pr_width;
            self.column_widths[COL_IDX_AGENT] = agent_width;
        }
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
            let mut fuzzy = FuzzySearch::new();
            let mut results: Vec<(usize, u32, Option<SearchMatch>)> = Vec::new();

            for (i, ws) in self.state.workstreams.iter().enumerate() {
                if let Some(result) = fuzzy.search_workstream(ws, query) {
                    // Only show excerpt for non-obvious matches (not identifier or title)
                    let search_match = if result.matched_field != "identifier"
                        && result.matched_field != "title"
                    {
                        Some(SearchMatch {
                            excerpt: result.excerpt,
                            match_in: result.matched_field,
                        })
                    } else {
                        None
                    };

                    results.push((i, result.score, search_match));
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
            // Move in the direction (stop on any item - workstream or section header)
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

    /// Get the issue currently being viewed in the modal
    /// Returns the navigated-to issue if set, otherwise the selected workstream
    pub fn modal_issue(&self) -> Option<&Workstream> {
        if let Some(ref issue_id) = self.modal_issue_id {
            self.state
                .workstreams
                .iter()
                .find(|ws| ws.linear_issue.id == *issue_id)
        } else {
            self.selected_workstream()
        }
    }

    /// Navigate to an issue in the modal (for in-modal parent/child navigation)
    /// Pushes current issue to stack and sets the new issue as current
    pub fn navigate_to_issue(&mut self, issue_id: &str) {
        // Push current issue to stack for back navigation
        if let Some(current) = self.modal_issue() {
            self.issue_navigation_stack
                .push(current.linear_issue.id.clone());
        }
        self.modal_issue_id = Some(issue_id.to_string());
        self.selected_child_idx = None;
        self.sub_issues_scroll = 0;
        self.description_scroll = 0;
    }

    /// Go back in the navigation stack
    /// Returns true if we went back, false if stack was empty
    pub fn navigate_back(&mut self) -> bool {
        if let Some(prev_id) = self.issue_navigation_stack.pop() {
            self.modal_issue_id = Some(prev_id);
            self.selected_child_idx = None;
            self.sub_issues_scroll = 0;
            self.description_scroll = 0;
            true
        } else {
            // Stack empty, clear modal_issue_id to return to selected workstream
            if self.modal_issue_id.is_some() {
                self.modal_issue_id = None;
                self.selected_child_idx = None;
                self.sub_issues_scroll = 0;
                self.description_scroll = 0;
                true
            } else {
                false
            }
        }
    }

    /// Clear navigation state (when closing modal)
    pub fn clear_navigation(&mut self) {
        self.issue_navigation_stack.clear();
        self.modal_issue_id = None;
        self.selected_child_idx = None;
        self.sub_issues_scroll = 0;
        self.modal_search_mode = false;
        self.modal_search_query.clear();
    }

    /// Enter modal search mode
    pub fn enter_modal_search(&mut self) {
        self.modal_search_mode = true;
        self.modal_search_query.clear();
    }

    /// Exit modal search mode
    pub fn exit_modal_search(&mut self) {
        self.modal_search_mode = false;
        // Keep query for showing results
    }

    /// Clear modal search
    pub fn clear_modal_search(&mut self) {
        self.modal_search_mode = false;
        self.modal_search_query.clear();
    }

    /// Check if an issue ID exists in our workstreams
    pub fn issue_exists(&self, issue_id: &str) -> bool {
        self.state
            .workstreams
            .iter()
            .any(|ws| ws.linear_issue.id == *issue_id)
    }

    /// Find workstream by identifier (e.g., "DRE-123")
    pub fn find_by_identifier(&self, identifier: &str) -> Option<&Workstream> {
        self.state
            .workstreams
            .iter()
            .find(|ws| ws.linear_issue.identifier == identifier)
    }

    pub async fn open_primary_link(&self) -> Result<()> {
        if let Some(ws) = self.selected_workstream() {
            open_linear_url(&ws.linear_issue.url)?;
        }
        Ok(())
    }

    pub async fn open_linear_link(&self) -> Result<()> {
        if let Some(ws) = self.modal_issue() {
            open_linear_url(&ws.linear_issue.url)?;
        }
        Ok(())
    }

    pub async fn open_github_link(&self) -> Result<()> {
        if let Some(ws) = self.modal_issue() {
            if let Some(pr) = &ws.github_pr {
                open_url(&pr.url)?;
            }
        }
        Ok(())
    }

    pub async fn open_vercel_link(&self) -> Result<()> {
        if let Some(ws) = self.modal_issue() {
            if let Some(deploy) = &ws.vercel_deployment {
                open_url(&deploy.url)?;
            }
        }
        Ok(())
    }

    pub fn open_link_menu(&mut self) {
        if self.show_link_menu() {
            self.modal = ModalState::None;
            self.clear_navigation();
        } else {
            self.modal = ModalState::LinkMenu { show_links_popup: false };
        }
    }

    pub async fn teleport_to_session(&self) -> Result<()> {
        if let Some(ws) = self.modal_issue() {
            if let Some(session) = &ws.agent_session {
                integrations::claude::focus_session_window(session).await?;
            }
        }
        Ok(())
    }

    /// Open a document attachment by index (0-based)
    pub fn open_document(&self, index: usize) -> Result<()> {
        if let Some(ws) = self.modal_issue() {
            if let Some(attachment) = ws.linear_issue.attachments.get(index) {
                open_url(&attachment.url)?;
            }
        }
        Ok(())
    }

    /// Open a child issue by index (0-based)
    pub fn open_child_issue(&self, index: usize) -> Result<()> {
        if let Some(ws) = self.modal_issue() {
            if let Some(child) = ws.linear_issue.children.get(index) {
                open_linear_url(&child.url)?;
            }
        }
        Ok(())
    }

    /// Maximum visible sub-issues in the link menu
    const SUB_ISSUES_VISIBLE_HEIGHT: usize = 8;

    /// Navigate to next sub-issue in link menu
    pub fn next_child_issue(&mut self) {
        if let Some(ws) = self.modal_issue() {
            let children_count = ws.linear_issue.children.len();
            if children_count == 0 {
                return;
            }

            let new_idx = match self.selected_child_idx {
                None => 0,
                Some(idx) => (idx + 1).min(children_count.saturating_sub(1)),
            };
            self.selected_child_idx = Some(new_idx);

            // Auto-scroll to keep selection visible
            let visible_end = self.sub_issues_scroll + Self::SUB_ISSUES_VISIBLE_HEIGHT;
            if new_idx >= visible_end {
                self.sub_issues_scroll = new_idx.saturating_sub(Self::SUB_ISSUES_VISIBLE_HEIGHT - 1);
            }
        }
    }

    /// Navigate to previous sub-issue in link menu
    pub fn prev_child_issue(&mut self) {
        if let Some(ws) = self.modal_issue() {
            if ws.linear_issue.children.is_empty() {
                return;
            }

            let new_idx = match self.selected_child_idx {
                None => 0,
                Some(idx) => idx.saturating_sub(1),
            };
            self.selected_child_idx = Some(new_idx);

            // Auto-scroll to keep selection visible
            if new_idx < self.sub_issues_scroll {
                self.sub_issues_scroll = new_idx;
            }
        }
    }

    /// Open currently selected sub-issue in browser
    pub fn open_selected_child_issue(&self) -> Result<()> {
        if let Some(idx) = self.selected_child_idx {
            self.open_child_issue(idx)?;
        }
        Ok(())
    }

    /// Open the parent issue in browser
    pub fn open_parent_issue(&self) -> Result<()> {
        if let Some(ws) = self.modal_issue() {
            if let Some(parent) = &ws.linear_issue.parent {
                open_linear_url(&parent.url)?;
            }
        }
        Ok(())
    }

    /// Navigate to parent issue in modal if it exists in workstreams
    /// Returns true if navigation happened (stayed in modal), false if not available
    pub fn navigate_to_parent(&mut self) -> bool {
        if let Some(ws) = self.modal_issue() {
            if let Some(parent) = &ws.linear_issue.parent {
                // Check if parent exists in our workstreams by identifier
                let parent_id = parent.identifier.clone();
                if let Some(parent_ws) = self
                    .state
                    .workstreams
                    .iter()
                    .find(|w| w.linear_issue.identifier == parent_id)
                {
                    let target_id = parent_ws.linear_issue.id.clone();
                    self.navigate_to_issue(&target_id);
                    return true;
                }
            }
        }
        false
    }

    /// Navigate to child issue by index in modal if it exists in workstreams
    /// Returns true if navigation happened, false if not available
    pub fn navigate_to_child(&mut self, child_idx: usize) -> bool {
        if let Some(ws) = self.modal_issue() {
            if let Some(child) = ws.linear_issue.children.get(child_idx) {
                // Check if child exists in our workstreams by identifier
                let child_identifier = child.identifier.clone();
                if let Some(child_ws) = self
                    .state
                    .workstreams
                    .iter()
                    .find(|w| w.linear_issue.identifier == child_identifier)
                {
                    let target_id = child_ws.linear_issue.id.clone();
                    self.navigate_to_issue(&target_id);
                    return true;
                }
            }
        }
        false
    }

    /// Navigate to selected child issue in modal
    pub fn navigate_to_selected_child(&mut self) -> bool {
        if let Some(idx) = self.selected_child_idx {
            return self.navigate_to_child(idx);
        }
        false
    }

    pub fn toggle_preview(&mut self) {
        self.show_preview = !self.show_preview;
    }

    pub fn toggle_help(&mut self) {
        if self.show_help() {
            self.modal = ModalState::None;
        } else {
            self.modal = ModalState::Help { tab: 0 };
        }
    }

    // Description modal
    pub fn open_description_modal(&mut self) {
        if let Some(ws) = self.modal_issue() {
            if ws.linear_issue.description.is_some() {
                self.modal = ModalState::Description;
                self.description_scroll = 0;
            }
        }
    }

    pub fn close_description_modal(&mut self) {
        self.modal = ModalState::None;
        self.description_scroll = 0;
    }

    pub fn scroll_description(&mut self, delta: i32) {
        let new_scroll = self.description_scroll as i32 + delta;
        self.description_scroll = new_scroll.max(0) as usize;
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
        if self.show_sort_menu() {
            self.modal = ModalState::None;
        } else {
            self.modal = ModalState::SortMenu;
        }
    }

    pub fn set_sort_mode(&mut self, mode: crate::data::SortMode) {
        self.state.sort_mode = mode;
        self.modal = ModalState::None;
        self.rebuild_visual_items();
    }

    // Resize mode
    pub fn toggle_resize_mode(&mut self) {
        if self.resize_mode() {
            self.modal = ModalState::None;
        } else {
            self.modal = ModalState::Resize;
        }
    }

    pub fn exit_resize_mode(&mut self) {
        self.modal = ModalState::None;
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
        if self.show_filter_menu() {
            self.modal = ModalState::None;
        } else {
            self.modal = ModalState::FilterMenu;
        }
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

    pub fn toggle_sub_issues(&mut self) {
        self.show_sub_issues = !self.show_sub_issues;
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

                // Sub-issue filter (if disabled, hide issues that have a parent)
                if !self.show_sub_issues && ws.linear_issue.parent.is_some() {
                    return false;
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
