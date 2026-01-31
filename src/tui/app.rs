use crate::config::Config;
use crate::data::{
    AgentSession, AppState, LinearChildRef, LinearCycle, LinearPriority, LinearStatus,
    SectionType, SortMode, VisualItem, Workstream,
};
use crate::integrations;
use crate::integrations::cache;
use crate::integrations::claude::watcher::ClaudeWatcher;
use crate::integrations::linear::{ProjectInfo, TeamMemberInfo};
use crate::tui::search::FuzzySearch;
use anyhow::Result;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use unicode_width::UnicodeWidthStr;

/// Timeout for refresh operations (60 seconds)
const REFRESH_TIMEOUT: Duration = Duration::from_secs(60);

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

/// Metadata loaded alongside workstreams during refresh
#[derive(Clone, Debug, Default)]
pub struct RefreshMetadata {
    pub projects: Option<Vec<ProjectInfo>>,
    pub team_members: Option<Vec<TeamMemberInfo>>,
    pub current_user_id: Option<String>,
}

/// Result from background refresh task
pub enum RefreshResult {
    /// Progress update
    Progress(RefreshProgress),
    /// Single workstream completed
    Workstream(Box<Workstream>),
    /// Metadata update (projects, team members, current user)
    Metadata(RefreshMetadata),
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
    "Status", "Priority", "ID", "Title", "PR", "Agent", "Vercel", "Time",
];

/// Active modal state - only one modal can be active at a time
/// This enum consolidates the previous 7 boolean modal flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModalState {
    #[default]
    None,
    Help {
        tab: usize,
    },
    LinkMenu {
        show_links_popup: bool,
    },
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
    /// Cached section counts (agent sessions, issues) - computed once per rebuild
    pub section_counts: HashMap<SectionType, usize>,

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
    pub filter_projects: HashSet<String>,
    pub filter_assignees: HashSet<String>, // "me", "unassigned", or user IDs
    pub available_cycles: Vec<LinearCycle>,
    pub available_projects: Vec<ProjectInfo>,
    pub available_team_members: Vec<TeamMemberInfo>,
    pub current_user_id: Option<String>,
    pub show_sub_issues: bool,
    pub show_completed: bool,
    pub show_canceled: bool,

    // Modal navigation state
    pub parent_selected: bool,
    pub selected_child_idx: Option<usize>,
    pub issue_navigation_stack: Vec<String>,
    pub modal_issue_id: Option<String>,
    pub sub_issues_scroll: usize,
    /// Dynamic visible height for sub-issues (UI sets this based on terminal size)
    sub_issues_visible_height: usize,
    pub description_scroll: usize,
    pub modal_search_mode: bool,
    pub modal_search_query: String,

    /// Channel receiver for background refresh results
    pub refresh_rx: Option<mpsc::Receiver<RefreshResult>>,
    /// Progress tracking for incremental updates
    pub refresh_progress: Option<RefreshProgress>,

    // Refresh robustness fields
    /// Shadow storage for incoming workstreams during refresh
    /// On success: swap with state.workstreams
    /// On error: discard and keep original data
    shadow_workstreams: Vec<Workstream>,
    /// Shadow metadata for refresh (projects, team members, current user)
    shadow_metadata: Option<RefreshMetadata>,
    /// Timestamp when refresh started (for timeout detection)
    refresh_started_at: Option<Instant>,
    /// File watcher for real-time Claude session updates
    claude_watcher: Option<ClaudeWatcher>,
    /// Cached current time for render frame (avoids repeated syscalls)
    pub frame_now: chrono::DateTime<chrono::Utc>,
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
        matches!(
            self.modal,
            ModalState::LinkMenu {
                show_links_popup: true
            }
        )
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
        let config = Arc::new(config);
        let mut state = AppState::default();
        if let Some(mode) = SortMode::from_config_str(&config.ui.default_sort) {
            state.sort_mode = mode;
        }

        let mut app = Self {
            config: Arc::clone(&config),
            state,
            filtered_indices: vec![],
            visual_items: vec![],
            visual_selected: 0,
            section_counts: HashMap::new(),
            modal: ModalState::None,
            show_preview: config.ui.show_preview,
            error_message: None,
            is_loading: false,
            spinner_frame: 0,
            // Default widths: Status=1, Priority=3, ID=10, Title=26, PR=12, Agent=10, Vercel=3, Time=6
            column_widths: config.ui.column_widths,
            resize_column_idx: COL_IDX_TITLE,
            search_all: false,
            search_excerpts: HashMap::new(),
            filter_cycles: HashSet::new(),
            filter_priorities: HashSet::new(),
            filter_projects: HashSet::new(),
            filter_assignees: HashSet::new(),
            available_cycles: Vec::new(),
            available_projects: Vec::new(),
            available_team_members: Vec::new(),
            current_user_id: None,
            show_sub_issues: config.ui.show_sub_issues,
            show_completed: config.ui.show_completed,
            show_canceled: config.ui.show_canceled,
            parent_selected: false,
            selected_child_idx: None,
            issue_navigation_stack: Vec::new(),
            modal_issue_id: None,
            sub_issues_scroll: 0,
            sub_issues_visible_height: 8, // Default, UI updates based on terminal size
            description_scroll: 0,
            modal_search_mode: false,
            modal_search_query: String::new(),
            refresh_rx: None,
            refresh_progress: None,
            shadow_workstreams: Vec::new(),
            shadow_metadata: None,
            refresh_started_at: None,
            claude_watcher: ClaudeWatcher::new().ok(),
            frame_now: chrono::Utc::now(),
        };

        app.load_cached_state();
        app
    }

    /// Load cached workstreams on startup (if enabled).
    /// Cached data is marked as stale until the first refresh completes.
    fn load_cached_state(&mut self) {
        let Ok(Some(cache_data)) = cache::load_cache(&self.config) else {
            return;
        };

        let mut workstreams = cache_data.workstreams;
        for ws in &mut workstreams {
            ws.stale = true;
        }

        self.state.workstreams = workstreams;
        self.state.last_refresh = Some(cache_data.last_sync);
        self.update_available_cycles();
        self.calculate_optimal_widths();
        self.apply_filters();
        self.rebuild_visual_items();
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
            Message::JumpNextSection => self.jump_next_section(),
            Message::JumpPrevSection => self.jump_prev_section(),
            Message::ScrollViewport(delta) => self.scroll_viewport(delta),

            // ─────────────────────────────────────────────────────────────────
            // Selection actions
            // ─────────────────────────────────────────────────────────────────
            Message::ExpandSection => self.expand_current_section(),
            Message::CollapseSection => self.collapse_current_section(),
            Message::ToggleSectionFold => self.toggle_section_fold(),
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
            Message::ToggleProjectFilter(idx) => self.toggle_project_filter(idx),
            Message::ClearProjectFilters => self.clear_project_filters(),
            Message::ToggleAssigneeFilter(idx) => self.toggle_assignee_filter(idx),
            Message::ClearAssigneeFilters => self.clear_assignee_filters(),
            Message::ToggleSubIssues => self.toggle_sub_issues(),
            Message::ToggleCompletedFilter => self.toggle_completed_filter(),
            Message::ToggleCanceledFilter => self.toggle_canceled_filter(),
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
                // Works from both normal mode and issue details - opens links popup directly
                self.modal = ModalState::LinkMenu {
                    show_links_popup: true,
                };
            }
            Message::CloseLinksPopup => {
                self.modal = ModalState::LinkMenu {
                    show_links_popup: false,
                };
            }
            Message::OpenLinearLink => {
                self.open_linear_link().await?;
                // Only clear modal if we're in link menu
                if self.show_link_menu() {
                    self.modal = ModalState::None;
                    self.clear_navigation();
                }
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
                // Navigate to parent or child depending on what's selected
                // Does nothing if issue not found in workstreams (use 'l' for links)
                if self.parent_selected {
                    self.navigate_to_parent();
                } else if self.selected_child_idx.is_some() {
                    self.navigate_to_selected_child();
                }
            }
            Message::NavigateToParent => {
                // Direct parent navigation (kept for compatibility)
                self.navigate_to_parent();
            }
            Message::OpenDocument(idx) => {
                self.open_document(idx)?;
                self.modal = ModalState::None;
                self.clear_navigation();
            }
            Message::OpenDescriptionModal => self.open_description_modal(),
            Message::NavigateToChild(idx) => {
                // Only navigate if child exists in workstreams
                // Don't open browser unexpectedly - user can use 'l' for links
                self.navigate_to_child(idx);
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

        // In search mode with active query, preserve score order (no grouping)
        let preserve_order = self.state.search_mode && !self.state.search_query.is_empty();
        self.visual_items = self
            .state
            .build_visual_items(&self.filtered_indices, preserve_order);

        // Cache section counts (O(n) instead of O(n²) per render frame)
        self.section_counts.clear();
        for &idx in &self.filtered_indices {
            if let Some(ws) = self.state.workstreams.get(idx) {
                let section = if ws.agent_session.is_some() {
                    SectionType::AgentSessions
                } else {
                    SectionType::Issues
                };
                *self.section_counts.entry(section).or_insert(0) += 1;
            }
        }

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

        let (workstreams_res, projects_res, members_res, current_user_res) = tokio::join!(
            integrations::fetch_workstreams(&self.config),
            integrations::linear::fetch_projects(&self.config),
            integrations::linear::fetch_team_members(&self.config),
            integrations::linear::fetch_current_user_id(&self.config)
        );

        match workstreams_res {
            Ok(workstreams) => {
                self.state.workstreams = workstreams;
                self.state.last_refresh = Some(Utc::now());

                if let Ok(projects) = projects_res {
                    self.available_projects = projects;
                }
                if let Ok(members) = members_res {
                    self.available_team_members = members;
                }
                if let Ok(user_id) = current_user_res {
                    self.current_user_id = Some(user_id);
                }

                // Extract available cycles from workstreams
                self.update_available_cycles();

                // Calculate optimal column widths based on content
                self.calculate_optimal_widths();

                // Apply filters
                self.apply_filters();
                self.rebuild_visual_items();
                self.error_message = None;

                if let Err(err) = cache::save_cache(
                    &self.config,
                    &cache::WorkstreamCache::new(self.state.workstreams.clone()),
                ) {
                    tracing::debug!("Failed to save cache: {}", err);
                }
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
    ///
    /// Uses shadow refresh pattern: new data goes to shadow_workstreams,
    /// only replacing main data on successful completion. This prevents
    /// data loss on transient errors.
    pub fn start_background_refresh(&mut self) {
        // Don't start another refresh if one is already in progress
        if self.refresh_rx.is_some() {
            return;
        }

        self.is_loading = true;
        self.refresh_started_at = Some(Instant::now()); // Track for timeout detection
        self.shadow_workstreams.clear(); // Clear shadow for new data (keep main data!)
        self.shadow_metadata = None;
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

    /// Trigger a background refresh if enough time has passed since last refresh.
    /// Used for user-action triggered refreshes (opening issue details, etc.)
    /// This is "optimistic" - shows cached data immediately, refreshes in background.
    pub fn trigger_user_action_refresh(&mut self) {
        // Skip if not in a Tokio runtime (e.g., during tests)
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }

        // Skip if refresh already in progress
        if self.refresh_rx.is_some() {
            return;
        }

        // Check if enough time has passed since last refresh
        let cooldown = Duration::from_secs(self.config.polling.user_action_cooldown_secs);
        let should_refresh = match self.state.last_refresh {
            Some(last) => {
                let elapsed = Utc::now().signed_duration_since(last);
                elapsed.num_seconds() >= cooldown.as_secs() as i64
            }
            None => true, // Never refreshed, definitely should refresh
        };

        if should_refresh {
            self.start_background_refresh();
        }
    }

    /// Poll for refresh results (non-blocking, call from event loop tick)
    ///
    /// Implements:
    /// - Shadow refresh pattern (new data in shadow, swap on success)
    /// - Timeout detection (fails refresh after REFRESH_TIMEOUT)
    /// - Monotonic progress tracking (derived from received workstream count)
    pub fn poll_refresh(&mut self) -> bool {
        // Check for timeout first
        if let Some(started) = self.refresh_started_at {
            if started.elapsed() > REFRESH_TIMEOUT {
                tracing::warn!("Refresh timed out after {:?}", REFRESH_TIMEOUT);
                self.is_loading = false;
                self.refresh_rx = None;
                self.refresh_started_at = None;
                self.refresh_progress = None;
                self.shadow_workstreams.clear();
                self.shadow_metadata = None;
                self.error_message = Some("Refresh timed out".to_string());
                return true;
            }
        }

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
                    // Only update stage and total, completed is derived from workstream count
                    if let Some(ref mut p) = self.refresh_progress {
                        p.total_issues = progress.total_issues;
                        p.current_stage = progress.current_stage;
                        // Don't overwrite completed - it's derived from shadow_workstreams.len()
                    } else {
                        self.refresh_progress = Some(progress);
                    }
                }
                RefreshResult::Workstream(ws) => {
                    // Add to shadow storage, not main data
                    self.shadow_workstreams.push(*ws);
                    // Monotonic progress: always derived from received count
                    if let Some(ref mut p) = self.refresh_progress {
                        p.completed = self.shadow_workstreams.len();
                    }
                }
                RefreshResult::Metadata(metadata) => {
                    self.shadow_metadata = Some(metadata);
                }
                RefreshResult::Complete => {
                    // Success: swap shadow with main data
                    std::mem::swap(&mut self.state.workstreams, &mut self.shadow_workstreams);
                    self.shadow_workstreams.clear();

                    if let Some(metadata) = self.shadow_metadata.take() {
                        if let Some(projects) = metadata.projects {
                            self.available_projects = projects;
                        }
                        if let Some(members) = metadata.team_members {
                            self.available_team_members = members;
                        }
                        if let Some(user_id) = metadata.current_user_id {
                            self.current_user_id = Some(user_id);
                        }
                    }

                    self.is_loading = false;
                    self.refresh_started_at = None;
                    self.refresh_progress = None;
                    self.state.last_refresh = Some(Utc::now());
                    self.update_available_cycles();
                    self.calculate_optimal_widths();
                    self.apply_filters();
                    self.rebuild_visual_items();
                    self.error_message = None;

                    if let Err(err) = cache::save_cache(
                        &self.config,
                        &cache::WorkstreamCache::new(self.state.workstreams.clone()),
                    ) {
                        tracing::debug!("Failed to save cache: {}", err);
                    }
                    completed = true;
                    should_restore = false;
                }
                RefreshResult::Error(msg) => {
                    // Error: discard shadow, keep original data
                    self.shadow_workstreams.clear();
                    self.shadow_metadata = None;

                    self.is_loading = false;
                    self.refresh_started_at = None;
                    self.refresh_progress = None;
                    // Include note about preserving previous data
                    self.error_message =
                        Some(format!("Refresh failed: {} (keeping previous data)", msg));
                    completed = true;
                    should_restore = false;
                }
            }
        }

        // Detect channel disconnect (sender dropped without Complete/Error)
        // This can happen if the background task panics
        if rx.is_closed() && !completed {
            tracing::warn!("Refresh channel closed unexpectedly");
            self.shadow_workstreams.clear();
            self.shadow_metadata = None;
            self.is_loading = false;
            self.refresh_started_at = None;
            self.refresh_progress = None;
            self.error_message = Some("Refresh interrupted (keeping previous data)".to_string());
            return true;
        }

        // Note: With shadow refresh pattern, we DON'T update the display during refresh.
        // The user sees the old data with a loading indicator, and new data appears
        // all at once on completion. This is safer - if refresh fails, the old data
        // is still visible. The progress bar shows "X/Y completed" for feedback.
        //
        // Progressive display is intentionally disabled to maintain data safety.
        // The tradeoff: less visual feedback during refresh, but no data loss on error.

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
        let mut has_sub_issues = false;

        for ws in &self.state.workstreams {
            let issue = &ws.linear_issue;

            // ID column
            max_id_len = max_id_len.max(issue.identifier.width());

            // Check if this is a sub-issue (has parent) - needs extra width for "└ " prefix
            if issue.parent.is_some() {
                has_sub_issues = true;
            }

            // Title column (cap at reasonable max to prevent single long title from dominating)
            max_title_len = max_title_len.max(issue.title.width().min(50));

            // PR column
            if let Some(pr) = &ws.github_pr {
                let pr_text = format!("PR#{}", pr.number);
                max_pr_len = max_pr_len.max(pr_text.width() + 2); // +2 for icon
            }

            // Agent column
            if let Some(session) = &ws.agent_session {
                let label_len = match session.status {
                    crate::data::AgentStatus::Running => 3,         // RUN
                    crate::data::AgentStatus::Idle => 4,            // IDLE
                    crate::data::AgentStatus::WaitingForInput => 4, // WAIT
                    crate::data::AgentStatus::Done => 4,            // DONE
                    crate::data::AgentStatus::Error => 3,           // ERR
                };
                // "CC <icon><ascii> <label>"
                max_agent_len = max_agent_len.max(label_len + 5);
            }
        }

        // Apply calculated widths with some padding
        // Add 2 extra chars for sub-issue prefix "└ " if any sub-issues exist
        let sub_issue_padding = if has_sub_issues { 2 } else { 0 };
        self.column_widths[COL_IDX_ID] = max_id_len + 1 + sub_issue_padding;
        self.column_widths[COL_IDX_TITLE] = max_title_len.min(40); // Cap title at 40
        self.column_widths[COL_IDX_PR] = max_pr_len.min(15);
        self.column_widths[COL_IDX_AGENT] = max_agent_len.min(24);

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
        // Title gets 45%, Agent gets 25%, ID gets 15%, PR gets 15%
        if available > 40 {
            let title_width = (available * 45 / 100).clamp(15, 50);
            let agent_width = (available * 25 / 100).clamp(12, 28);
            let id_width = (available * 15 / 100).clamp(6, 15);
            let pr_width = (available * 15 / 100).clamp(8, 15);

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
        self.available_cycles
            .sort_by(|a, b| b.number.cmp(&a.number));
    }

    pub async fn on_tick(&mut self) {
        // Advance spinner animation
        self.tick_spinner();

        // Periodic background refresh based on polling interval
        if tokio::runtime::Handle::try_current().is_ok() && self.refresh_rx.is_none() {
            let interval_secs = self
                .config
                .polling
                .linear_interval_secs
                .min(self.config.polling.github_interval_secs)
                .min(self.config.polling.vercel_interval_secs);
            let interval = Duration::from_secs(interval_secs);
            if interval.as_secs() > 0 {
                let should_refresh = match self.state.last_refresh {
                    Some(last) => {
                        let elapsed = self.frame_now.signed_duration_since(last);
                        elapsed.num_seconds() >= interval.as_secs() as i64
                    }
                    None => true,
                };

                if should_refresh {
                    self.start_background_refresh();
                }
            }
        }
    }

    /// Poll file watcher for Claude session changes (real-time updates)
    ///
    /// This is much more efficient than polling - it only updates when the
    /// file actually changes, using OS-level file system notifications (inotify on Linux).
    pub fn poll_claude_watcher(&mut self) -> bool {
        let Some(watcher) = &self.claude_watcher else {
            return false;
        };

        if !watcher.poll() {
            return false;
        }

        // File changed - update agent sessions in workstreams
        let sessions = watcher.get_sessions_snapshot();
        self.update_agent_sessions_from_watcher(&sessions);
        true
    }

    /// Update agent sessions in workstreams from watcher data
    ///
    /// This updates existing agent sessions with fresh status from the watcher.
    /// New sessions (not yet linked to a workstream) will be picked up on next full refresh.
    fn update_agent_sessions_from_watcher(&mut self, sessions: &[AgentSession]) {
        // Build a map of git_branch -> session for O(1) lookup
        let session_by_branch: HashMap<&str, &AgentSession> = sessions
            .iter()
            .filter_map(|s| s.git_branch.as_deref().map(|b| (b, s)))
            .collect();

        let mut any_changed = false;

        // Update agent sessions in existing workstreams
        for ws in &mut self.state.workstreams {
            if let Some(current_session) = &ws.agent_session {
                // Try to find updated session by git_branch
                if let Some(branch) = &current_session.git_branch {
                    if let Some(updated) = session_by_branch.get(branch.as_str()) {
                        // Only update if status actually changed
                        if current_session.status != updated.status {
                            ws.agent_session = Some((*updated).clone());
                            any_changed = true;
                        }
                    }
                }
            }
        }

        // Only rebuild visual items if something changed
        if any_changed {
            self.rebuild_visual_items();
        }
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

            // Always include unlinked agent sessions (no identifier), even in search mode.
            // Append them after scored results to keep relevance ordering intact.
            let mut included: HashSet<usize> = self.filtered_indices.iter().copied().collect();
            for (idx, ws) in self.state.workstreams.iter().enumerate() {
                if ws.agent_session.is_some()
                    && ws.linear_issue.identifier.is_empty()
                    && included.insert(idx)
                {
                    self.filtered_indices.push(idx);
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

    /// Jump to next section header
    pub fn jump_next_section(&mut self) {
        let len = self.visual_items.len();
        if len == 0 {
            return;
        }

        // Find next section header after current position
        for i in (self.visual_selected + 1)..len {
            if matches!(self.visual_items.get(i), Some(VisualItem::SectionHeader(_))) {
                self.visual_selected = i;
                return;
            }
        }
        // If no section found, go to end
        self.visual_selected = len - 1;
    }

    /// Jump to previous section header
    pub fn jump_prev_section(&mut self) {
        if self.visual_items.is_empty() || self.visual_selected == 0 {
            return;
        }

        // Find previous section header before current position
        for i in (0..self.visual_selected).rev() {
            if matches!(self.visual_items.get(i), Some(VisualItem::SectionHeader(_))) {
                self.visual_selected = i;
                return;
            }
        }
        // If no section found, go to start
        self.visual_selected = 0;
    }

    /// Scroll viewport by delta lines (vim Ctrl+e/y style)
    /// Moves selection to keep relative position as viewport scrolls
    pub fn scroll_viewport(&mut self, delta: i32) {
        // In a TUI with auto-scroll-to-selection, scrolling viewport
        // effectively means moving selection while the view follows
        self.move_selection(delta);
    }

    /// Toggle fold of the section containing the current item
    pub fn toggle_section_fold(&mut self) {
        let section = match self.visual_items.get(self.visual_selected) {
            Some(VisualItem::SectionHeader(section)) => Some(*section),
            Some(VisualItem::Workstream(idx)) => self.state.workstreams.get(*idx).map(|ws| {
                if ws.agent_session.is_some() {
                    SectionType::AgentSessions
                } else {
                    SectionType::Issues
                }
            }),
            None => None,
        };

        if let Some(section) = section {
            if self.state.collapsed_sections.contains(&section) {
                self.state.collapsed_sections.remove(&section);
            } else {
                self.state.collapsed_sections.insert(section);
            }
            self.rebuild_visual_items();
        }
    }

    /// Get the currently selected workstream
    pub fn selected_workstream(&self) -> Option<&Workstream> {
        match self.visual_items.get(self.visual_selected) {
            Some(VisualItem::Workstream(idx)) => self.state.workstreams.get(*idx),
            _ => None,
        }
    }

    /// Get the currently selected section
    pub fn selected_section(&self) -> Option<SectionType> {
        match self.visual_items.get(self.visual_selected) {
            Some(VisualItem::SectionHeader(section)) => Some(*section),
            Some(VisualItem::Workstream(idx)) => self.state.workstreams.get(*idx).map(|ws| {
                if ws.agent_session.is_some() {
                    SectionType::AgentSessions
                } else {
                    SectionType::Issues
                }
            }),
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
        self.sub_issues_scroll = 0;
        self.description_scroll = 0;

        // Pre-select parent if exists, otherwise first child
        // Note: must check after setting modal_issue_id so modal_issue() returns the new issue
        self.pre_select_for_modal_issue();
    }

    /// Go back in the navigation stack
    /// Returns true if we went back, false if stack was empty
    pub fn navigate_back(&mut self) -> bool {
        if let Some(prev_id) = self.issue_navigation_stack.pop() {
            self.modal_issue_id = Some(prev_id);
            self.sub_issues_scroll = 0;
            self.description_scroll = 0;
            // Re-run pre-selection for the issue we're returning to
            self.pre_select_for_modal_issue();
            true
        } else {
            // Stack empty, clear modal_issue_id to return to selected workstream
            if self.modal_issue_id.is_some() {
                self.modal_issue_id = None;
                self.sub_issues_scroll = 0;
                self.description_scroll = 0;
                // Re-run pre-selection for the original selected workstream
                self.pre_select_for_modal_issue();
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
        self.parent_selected = false;
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

    pub async fn open_linear_link(&self) -> Result<()> {
        if let Some(ws) = self.modal_issue() {
            if !ws.linear_issue.url.is_empty() {
                open_linear_url(&ws.linear_issue.url)?;
            }
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
            self.modal = ModalState::LinkMenu {
                show_links_popup: false,
            };
            // Pre-select parent if exists, otherwise first child
            self.pre_select_in_link_menu();
            // Trigger optimistic background refresh when opening issue details
            self.trigger_user_action_refresh();
        }
    }

    /// Pre-select the first navigable item in link menu (parent or first child)
    fn pre_select_in_link_menu(&mut self) {
        self.pre_select_for_modal_issue();
    }

    /// Pre-select parent or first child for the current modal issue
    /// Uses modal_issue() which respects modal_issue_id if set
    fn pre_select_for_modal_issue(&mut self) {
        let has_parent = self
            .modal_issue()
            .map(|ws| ws.linear_issue.parent.is_some())
            .unwrap_or(false);
        // Use sorted/filtered children count (matches UI display)
        let children_count = self.get_sorted_filtered_children().len();

        if has_parent {
            self.parent_selected = true;
            self.selected_child_idx = None;
        } else if children_count > 0 {
            self.parent_selected = false;
            self.selected_child_idx = Some(0);
            self.sub_issues_scroll = 0;
        } else {
            self.parent_selected = false;
            self.selected_child_idx = None;
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

    /// Get the current visible height for sub-issues scroll
    pub fn sub_issues_visible_height(&self) -> usize {
        self.sub_issues_visible_height
    }

    /// Set the visible height for sub-issues (called by UI based on terminal size)
    pub fn set_sub_issues_visible_height(&mut self, height: usize) {
        self.sub_issues_visible_height = height.clamp(3, 10);
    }

    /// Set cached frame time (call once per render frame to avoid repeated syscalls)
    pub fn set_frame_time(&mut self, now: chrono::DateTime<chrono::Utc>) {
        self.frame_now = now;
    }

    /// Navigate to next item in link menu (parent → children cycle)
    /// Uses sorted/filtered children count to match what's displayed in UI
    pub fn next_child_issue(&mut self) {
        let has_parent = self
            .modal_issue()
            .map(|ws| ws.linear_issue.parent.is_some())
            .unwrap_or(false);

        // Use sorted/filtered children count (matches UI display)
        let children_count = self.get_sorted_filtered_children().len();

        if self.parent_selected {
            // Move from parent to first child (if any)
            if children_count > 0 {
                self.parent_selected = false;
                self.selected_child_idx = Some(0);
                self.sub_issues_scroll = 0;
            }
        } else if let Some(idx) = self.selected_child_idx {
            // Move to next child
            if idx + 1 < children_count {
                self.selected_child_idx = Some(idx + 1);
                // Auto-scroll to keep selection visible (uses dynamic visible height)
                let visible_height = self.sub_issues_visible_height;
                let visible_end = self.sub_issues_scroll + visible_height;
                if idx + 1 >= visible_end {
                    self.sub_issues_scroll = (idx + 1).saturating_sub(visible_height - 1);
                }
            }
        } else {
            // Nothing selected - select parent first if exists, else first child
            if has_parent {
                self.parent_selected = true;
            } else if children_count > 0 {
                self.selected_child_idx = Some(0);
                self.sub_issues_scroll = 0;
            }
        }
    }

    /// Navigate to previous item in link menu (children → parent cycle)
    /// Uses sorted/filtered children to match what's displayed in UI
    pub fn prev_child_issue(&mut self) {
        let has_parent = self
            .modal_issue()
            .map(|ws| ws.linear_issue.parent.is_some())
            .unwrap_or(false);

        if self.parent_selected {
            // Already at top (parent), do nothing
        } else if let Some(idx) = self.selected_child_idx {
            if idx == 0 && has_parent {
                // Move from first child to parent
                self.selected_child_idx = None;
                self.parent_selected = true;
            } else if idx > 0 {
                // Move to previous child
                self.selected_child_idx = Some(idx - 1);
                // Auto-scroll to keep selection visible
                if idx - 1 < self.sub_issues_scroll {
                    self.sub_issues_scroll = idx - 1;
                }
            }
        } else {
            // Nothing selected - select parent if exists
            if has_parent {
                self.parent_selected = true;
            }
        }
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

    /// Get sorted/filtered children for the modal issue (matches UI rendering logic)
    fn get_sorted_filtered_children(&self) -> Vec<&LinearChildRef> {
        let Some(ws) = self.modal_issue() else {
            return Vec::new();
        };

        // Filter children based on modal search query
        let filtered: Vec<&LinearChildRef> = if self.modal_search_query.is_empty() {
            ws.linear_issue.children.iter().collect()
        } else {
            let mut fuzzy = FuzzySearch::new();
            ws.linear_issue
                .children
                .iter()
                .filter(|child| {
                    let text = format!(
                        "{} {} {} {}",
                        child.identifier,
                        child.title,
                        child.status.display_name(),
                        child.priority.label()
                    );
                    fuzzy
                        .multi_term_match(&self.modal_search_query, &text)
                        .is_some()
                })
                .collect()
        };

        // Sort filtered children using shared sorting logic
        crate::data::sort_children(filtered, self.state.sort_mode)
    }

    /// Navigate to selected child issue in modal
    /// Uses the sorted/filtered list to find the correct child
    pub fn navigate_to_selected_child(&mut self) -> bool {
        let Some(idx) = self.selected_child_idx else {
            return false;
        };

        // Get the sorted/filtered children list (matches UI rendering)
        let sorted_children = self.get_sorted_filtered_children();
        let Some(child) = sorted_children.get(idx) else {
            return false;
        };

        // Find the child in workstreams by identifier
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
        // Go back to link menu (issue details), not main view
        self.modal = ModalState::LinkMenu {
            show_links_popup: false,
        };
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

    pub fn set_sort_mode(&mut self, mode: SortMode) {
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

    pub fn toggle_project_filter(&mut self, idx: usize) {
        if let Some(project) = self.available_projects.get(idx) {
            let id = project.id.clone();
            if self.filter_projects.contains(&id) {
                self.filter_projects.remove(&id);
            } else {
                self.filter_projects.insert(id);
            }
            self.apply_filters();
            self.rebuild_visual_items();
        }
    }

    pub fn clear_project_filters(&mut self) {
        self.filter_projects.clear();
        self.apply_filters();
        self.rebuild_visual_items();
    }

    pub fn toggle_assignee_filter(&mut self, idx: usize) {
        // Index 0: "Me", Index 1: "Unassigned", Rest: team members
        let id = match idx {
            0 => "me".to_string(),
            1 => "unassigned".to_string(),
            _ => {
                if let Some(member) = self.available_team_members.get(idx - 2) {
                    member.id.clone()
                } else {
                    return;
                }
            }
        };

        if self.filter_assignees.contains(&id) {
            self.filter_assignees.remove(&id);
        } else {
            self.filter_assignees.insert(id);
        }
        self.apply_filters();
        self.rebuild_visual_items();
    }

    pub fn clear_assignee_filters(&mut self) {
        self.filter_assignees.clear();
        self.apply_filters();
        self.rebuild_visual_items();
    }

    pub fn clear_all_filters(&mut self) {
        self.filter_cycles.clear();
        self.filter_priorities.clear();
        self.filter_projects.clear();
        self.filter_assignees.clear();
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
        ]
        .into_iter()
        .collect();
        // Show all projects/assignees
        self.filter_projects.clear();
        self.filter_assignees.clear();
        self.apply_filters();
        self.rebuild_visual_items();
    }

    pub fn toggle_sub_issues(&mut self) {
        self.show_sub_issues = !self.show_sub_issues;
        self.apply_filters();
        self.rebuild_visual_items();
    }

    pub fn toggle_completed_filter(&mut self) {
        self.show_completed = !self.show_completed;
        self.apply_filters();
        self.rebuild_visual_items();
    }

    pub fn toggle_canceled_filter(&mut self) {
        self.show_canceled = !self.show_canceled;
        self.apply_filters();
        self.rebuild_visual_items();
    }

    /// Apply filters to create filtered_indices
    pub fn apply_filters(&mut self) {
        // Pre-build project name → ID lookup map (O(1) instead of O(n) per workstream)
        let project_name_to_id: HashMap<&str, &str> = self
            .available_projects
            .iter()
            .map(|p| (p.name.as_str(), p.id.as_str()))
            .collect();

        self.filtered_indices = self
            .state
            .workstreams
            .iter()
            .enumerate()
            .filter(|(_, ws)| {
                // Always show unlinked agent sessions regardless of filters
                if ws.agent_session.is_some() && ws.linear_issue.identifier.is_empty() {
                    return true;
                }

                let status = ws.linear_issue.status;

                // Completed filter (hide completed unless explicitly shown)
                if !self.show_completed && status == LinearStatus::Done {
                    return false;
                }

                // Canceled filter (hide canceled/duplicate unless explicitly shown)
                if !self.show_canceled
                    && (status == LinearStatus::Canceled || status == LinearStatus::Duplicate)
                {
                    return false;
                }

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
                if !self.filter_priorities.is_empty()
                    && !self.filter_priorities.contains(&ws.linear_issue.priority)
                {
                    return false;
                }

                // Project filter (empty = show all) - O(1) lookup via pre-built map
                if !self.filter_projects.is_empty() {
                    match &ws.linear_issue.project {
                        Some(project_name) => {
                            match project_name_to_id.get(project_name.as_str()) {
                                Some(id) if self.filter_projects.contains(*id) => {}
                                _ => return false,
                            }
                        }
                        None => return false, // No project, filtered out
                    }
                }

                // Assignee filter (empty = show all)
                if !self.filter_assignees.is_empty() {
                    let assignee_id = ws.linear_issue.assignee_id.as_deref();
                    let mut matched = false;

                    // Unassigned
                    if self.filter_assignees.contains("unassigned") && assignee_id.is_none() {
                        matched = true;
                    }

                    // Me
                    if self.filter_assignees.contains("me") {
                        if let Some(me) = self.current_user_id.as_deref() {
                            if assignee_id == Some(me) {
                                matched = true;
                            }
                        }
                    }

                    // Specific user IDs
                    if let Some(id) = assignee_id {
                        if self.filter_assignees.contains(id) {
                            matched = true;
                        }
                    }

                    if !matched {
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
        !self.filter_cycles.is_empty()
            || !self.filter_priorities.is_empty()
            || !self.filter_projects.is_empty()
            || !self.filter_assignees.is_empty()
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
