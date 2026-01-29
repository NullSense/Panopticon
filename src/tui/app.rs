use crate::config::Config;
use crate::data::{AppState, Workstream};
use crate::integrations;
use anyhow::Result;
use chrono::Utc;

pub struct App {
    pub config: Config,
    pub state: AppState,
    pub filtered_indices: Vec<usize>,
    pub show_help: bool,
    pub show_link_menu: bool,
    pub show_preview: bool,
    pub search_all: bool,
    pub error_message: Option<String>,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: AppState::default(),
            filtered_indices: vec![],
            show_help: false,
            show_link_menu: false,
            show_preview: false,
            search_all: false,
            error_message: None,
        }
    }

    pub async fn refresh(&mut self) -> Result<()> {
        match integrations::fetch_workstreams(&self.config).await {
            Ok(workstreams) => {
                self.state.workstreams = workstreams;
                self.state.last_refresh = Some(Utc::now());
                self.filtered_indices = (0..self.state.workstreams.len()).collect();
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("Refresh failed: {}", e));
                tracing::error!("Failed to fetch workstreams: {}", e);
            }
        }
        Ok(())
    }

    pub async fn on_tick(&mut self) {
        // Could do incremental updates here
    }

    pub fn enter_search(&mut self, search_all: bool) {
        self.state.search_mode = true;
        self.state.search_query.clear();
        self.search_all = search_all;
    }

    pub fn exit_search(&mut self) {
        self.state.search_mode = false;
        self.state.search_query.clear();
        self.filtered_indices = (0..self.state.workstreams.len()).collect();
    }

    pub fn update_search(&mut self) {
        if self.state.search_query.is_empty() {
            self.filtered_indices = (0..self.state.workstreams.len()).collect();
            return;
        }

        let query = self.state.search_query.to_lowercase();
        self.filtered_indices = self
            .state
            .workstreams
            .iter()
            .enumerate()
            .filter(|(_, ws)| {
                let issue = &ws.linear_issue;
                issue.identifier.to_lowercase().contains(&query)
                    || issue.title.to_lowercase().contains(&query)
                    || issue
                        .description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&query))
                        .unwrap_or(false)
            })
            .map(|(i, _)| i)
            .collect();

        // Reset selection if out of bounds
        if self.state.selected_index >= self.filtered_indices.len() {
            self.state.selected_index = 0;
        }
    }

    pub fn confirm_search(&mut self) {
        self.state.search_mode = false;
        // Keep filtered results
    }

    pub fn move_selection(&mut self, delta: i32) {
        let len = self.filtered_indices.len();
        if len == 0 {
            return;
        }

        let new_index = if delta > 0 {
            (self.state.selected_index + delta as usize).min(len - 1)
        } else {
            self.state.selected_index.saturating_sub((-delta) as usize)
        };

        self.state.selected_index = new_index;
    }

    pub fn go_to_top(&mut self) {
        self.state.selected_index = 0;
    }

    pub fn go_to_bottom(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.state.selected_index = self.filtered_indices.len() - 1;
        }
    }

    pub fn page_down(&mut self) {
        self.move_selection(10);
    }

    pub fn page_up(&mut self) {
        self.move_selection(-10);
    }

    pub fn selected_workstream(&self) -> Option<&Workstream> {
        self.filtered_indices
            .get(self.state.selected_index)
            .and_then(|&i| self.state.workstreams.get(i))
    }

    pub async fn open_primary_link(&self) -> Result<()> {
        if let Some(ws) = self.selected_workstream() {
            open_url(&ws.linear_issue.url)?;
        }
        Ok(())
    }

    pub async fn open_linear_link(&self) -> Result<()> {
        if let Some(ws) = self.selected_workstream() {
            open_url(&ws.linear_issue.url)?;
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
        if let Some(status) = self.current_section_status() {
            self.state.collapsed_sections.insert(status);
        }
    }

    pub fn expand_current_section(&mut self) {
        if let Some(status) = self.current_section_status() {
            self.state.collapsed_sections.remove(&status);
        }
    }

    fn current_section_status(&self) -> Option<crate::data::LinearStatus> {
        self.selected_workstream().map(|ws| ws.linear_issue.status)
    }

    // Sorting
    pub fn cycle_sort_mode(&mut self) {
        self.state.sort_mode = self.state.sort_mode.next();
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
