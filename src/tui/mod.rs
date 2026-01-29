mod app;
pub mod search;
mod ui;

use crate::config::Config;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

pub use app::{App, ModalState, RefreshProgress, RefreshResult};

pub async fn run(config: Config) -> Result<()> {
    // Check if stdout is a terminal
    if !std::io::IsTerminal::is_terminal(&io::stdout()) {
        anyhow::bail!("panopticon requires an interactive terminal");
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(config);

    // Initial data fetch (non-blocking - UI shows immediately with loading state)
    app.start_background_refresh();

    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = std::time::Instant::now();

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());

        if event::poll(timeout)? {
            match event::read()? {
            Event::Resize(width, _height) => {
                app.recalculate_column_widths(width);
            }
            Event::Key(key) => {
                if app.state.search_mode {
                    match key.code {
                        KeyCode::Esc => {
                            app.exit_search();
                        }
                        KeyCode::Enter => {
                            app.confirm_search();
                        }
                        KeyCode::Backspace => {
                            app.state.search_query.pop();
                            app.update_search();
                        }
                        KeyCode::Char(c) => {
                            app.state.search_query.push(c);
                            app.update_search();
                        }
                        _ => {}
                    }
                } else if app.show_description_modal() {
                    // Handle description modal key presses
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            app.close_description_modal();
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            app.scroll_description(1);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            app.scroll_description(-1);
                        }
                        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.scroll_description(10);
                        }
                        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.scroll_description(-10);
                        }
                        KeyCode::Char('G') => {
                            app.scroll_description(1000); // Jump to bottom
                        }
                        KeyCode::Char('g') => {
                            // gg to go to top
                            if event::poll(Duration::from_millis(500))? {
                                if let Event::Key(k) = event::read()? {
                                    if k.code == KeyCode::Char('g') {
                                        app.description_scroll = 0;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                } else if app.show_help() {
                    // Handle help modal key presses
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                            app.modal = ModalState::None;
                        }
                        KeyCode::Char('1') => {
                            app.modal = ModalState::Help { tab: 0 };
                        }
                        KeyCode::Char('2') => {
                            app.modal = ModalState::Help { tab: 1 };
                        }
                        _ => {}
                    }
                } else if app.resize_mode() {
                    // Handle resize mode key presses
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('R') => {
                            app.exit_resize_mode();
                        }
                        KeyCode::Char('h') | KeyCode::Left => {
                            app.resize_column_narrower();
                        }
                        KeyCode::Char('l') | KeyCode::Right => {
                            app.resize_column_wider();
                        }
                        KeyCode::Tab => {
                            app.resize_next_column();
                        }
                        KeyCode::BackTab => {
                            app.resize_prev_column();
                        }
                        _ => {}
                    }
                } else if app.show_sort_menu() {
                    // Handle sort menu key presses
                    use crate::data::SortMode;
                    match key.code {
                        KeyCode::Esc => {
                            app.modal = ModalState::None;
                        }
                        KeyCode::Char(c) if ('1'..='6').contains(&c) => {
                            let idx = c.to_digit(10).unwrap() as usize;
                            if let Some(mode) = SortMode::from_index(idx) {
                                app.set_sort_mode(mode);
                            }
                        }
                        _ => {}
                    }
                } else if app.show_link_menu() {
                    // Handle links popup first (nested modal)
                    if app.show_links_popup() {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('l') => {
                                app.modal = ModalState::LinkMenu { show_links_popup: false };
                            }
                            KeyCode::Char('1') => {
                                app.open_linear_link().await?;
                                app.modal = ModalState::None;
                                app.clear_navigation();
                            }
                            KeyCode::Char('2') => {
                                app.open_github_link().await?;
                                app.modal = ModalState::None;
                                app.clear_navigation();
                            }
                            KeyCode::Char('3') => {
                                app.open_vercel_link().await?;
                                app.modal = ModalState::None;
                                app.clear_navigation();
                            }
                            KeyCode::Char('4') => {
                                app.teleport_to_session().await?;
                                app.modal = ModalState::None;
                                app.clear_navigation();
                            }
                            _ => {}
                        }
                    } else if app.modal_search_mode {
                        // Handle modal search mode
                        match key.code {
                            KeyCode::Esc => {
                                app.exit_modal_search();
                            }
                            KeyCode::Enter => {
                                app.exit_modal_search();
                            }
                            KeyCode::Backspace => {
                                app.modal_search_query.pop();
                            }
                            KeyCode::Char(c) => {
                                app.modal_search_query.push(c);
                            }
                            _ => {}
                        }
                    } else {
                        // Handle issue details modal key presses
                        match key.code {
                            KeyCode::Esc => {
                                // Clear search first if active
                                if !app.modal_search_query.is_empty() {
                                    app.clear_modal_search();
                                } else if !app.navigate_back() {
                                    // Stack empty, close the menu
                                    app.modal = ModalState::None;
                                    app.clear_navigation();
                                }
                            }
                            // Search in modal
                            KeyCode::Char('/') => {
                                app.enter_modal_search();
                            }
                            // Open links popup
                            KeyCode::Char('l') => {
                                app.modal = ModalState::LinkMenu { show_links_popup: true };
                            }
                            // j/k navigation for sub-issues
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.next_child_issue();
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.prev_child_issue();
                            }
                            // Enter on selected sub-issue: navigate in modal if available, else browser
                            KeyCode::Enter => {
                                if app.selected_child_idx.is_some() {
                                    if !app.navigate_to_selected_child() {
                                        // Child not in workstreams, open in browser
                                        app.open_selected_child_issue()?;
                                        app.modal = ModalState::None;
                                        app.clear_navigation();
                                    }
                                    // If navigated, stay in modal (don't close)
                                }
                            }
                            // Open parent issue: navigate in modal if available, else browser
                            KeyCode::Char('p') => {
                                if !app.navigate_to_parent() {
                                    // Parent not in workstreams, open in browser
                                    app.open_parent_issue()?;
                                    app.modal = ModalState::None;
                                    app.clear_navigation();
                                }
                                // If navigated, stay in modal (don't close)
                            }
                            // Document shortcuts: d + number, or d alone for full description
                            KeyCode::Char('d') => {
                                // Wait for digit or open description if timeout
                                if event::poll(Duration::from_millis(500))? {
                                    if let Event::Key(k) = event::read()? {
                                        if let KeyCode::Char(c) = k.code {
                                            if let Some(digit) = c.to_digit(10) {
                                                if digit >= 1 && digit <= 9 {
                                                    app.open_document((digit - 1) as usize)?;
                                                    app.modal = ModalState::None;
                                                    app.clear_navigation();
                                                }
                                            } else {
                                                // Not a digit, open description modal
                                                app.open_description_modal();
                                            }
                                        } else {
                                            // Not a char key, open description modal
                                            app.open_description_modal();
                                        }
                                    }
                                } else {
                                    // Timeout, open description modal
                                    app.open_description_modal();
                                }
                            }
                            // Child issue shortcuts: c + number - navigate in modal if available
                            KeyCode::Char('c') => {
                                // Wait for digit
                                if event::poll(Duration::from_millis(500))? {
                                    if let Event::Key(k) = event::read()? {
                                        if let KeyCode::Char(c) = k.code {
                                            if let Some(digit) = c.to_digit(10) {
                                                if digit >= 1 && digit <= 9 {
                                                    let idx = (digit - 1) as usize;
                                                    if !app.navigate_to_child(idx) {
                                                        // Child not in workstreams, open in browser
                                                        app.open_child_issue(idx)?;
                                                        app.modal = ModalState::None;
                                                        app.clear_navigation();
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                } else if app.show_filter_menu() {
                    // Handle filter menu key presses
                    use crate::data::LinearPriority;
                    match key.code {
                        KeyCode::Esc => {
                            app.modal = ModalState::None;
                        }
                        // Cycle filters (0-9)
                        KeyCode::Char(c) if ('0'..='9').contains(&c) => {
                            let idx = c.to_digit(10).unwrap() as usize;
                            app.toggle_cycle_filter(idx);
                        }
                        // Priority filters
                        KeyCode::Char('u') => {
                            app.toggle_priority_filter(LinearPriority::Urgent);
                        }
                        KeyCode::Char('h') => {
                            app.toggle_priority_filter(LinearPriority::High);
                        }
                        KeyCode::Char('m') => {
                            app.toggle_priority_filter(LinearPriority::Medium);
                        }
                        KeyCode::Char('l') => {
                            app.toggle_priority_filter(LinearPriority::Low);
                        }
                        KeyCode::Char('n') => {
                            app.toggle_priority_filter(LinearPriority::NoPriority);
                        }
                        // Toggle sub-issues visibility
                        KeyCode::Char('t') => {
                            app.toggle_sub_issues();
                        }
                        // Select all / clear all
                        KeyCode::Char('a') => {
                            app.clear_all_filters();
                        }
                        KeyCode::Char('c') => {
                            app.select_all_filters();
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => {
                            return Ok(());
                        }
                        KeyCode::Char('/') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.enter_search(true); // Search all
                        }
                        KeyCode::Char('/') => {
                            app.enter_search(false);
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            app.move_selection(1);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            app.move_selection(-1);
                        }
                        KeyCode::Char('g') => {
                            // Wait for second 'g' for gg
                            if event::poll(Duration::from_millis(500))? {
                                if let Event::Key(k) = event::read()? {
                                    if k.code == KeyCode::Char('g') {
                                        app.go_to_top();
                                    }
                                }
                            }
                        }
                        KeyCode::Char('G') => {
                            app.go_to_bottom();
                        }
                        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.page_down();
                        }
                        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.page_up();
                        }
                        KeyCode::Enter => {
                            app.open_primary_link().await?;
                        }
                        KeyCode::Char('o') => {
                            app.open_link_menu();
                        }
                        KeyCode::Char('t') => {
                            app.teleport_to_session().await?;
                        }
                        KeyCode::Char('p') => {
                            app.toggle_preview();
                        }
                        KeyCode::Char('r') => {
                            // Start non-blocking background refresh
                            app.start_background_refresh();
                        }
                        KeyCode::Char('?') => {
                            app.toggle_help();
                        }
                        // Section collapse/expand
                        KeyCode::Char('h') | KeyCode::Left => {
                            app.collapse_current_section();
                        }
                        KeyCode::Char('l') | KeyCode::Right => {
                            app.expand_current_section();
                        }
                        // Sorting
                        KeyCode::Char('s') => {
                            app.toggle_sort_menu();
                        }
                        // Filtering
                        KeyCode::Char('f') => {
                            app.toggle_filter_menu();
                        }
                        // Resize columns
                        KeyCode::Char('R') => {
                            app.toggle_resize_mode();
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick().await;

            // Poll for background refresh results (non-blocking)
            app.poll_refresh();

            last_tick = std::time::Instant::now();
        }
    }
}
