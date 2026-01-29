mod app;
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

pub use app::App;

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

    // Initial data fetch
    app.refresh().await?;

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
            if let Event::Key(key) = event::read()? {
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
                } else if app.show_help {
                    // Handle help modal key presses
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                            app.show_help = false;
                        }
                        KeyCode::Char('1') => {
                            app.help_tab = 0;
                        }
                        KeyCode::Char('2') => {
                            app.help_tab = 1;
                        }
                        _ => {}
                    }
                } else if app.resize_mode {
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
                } else if app.show_sort_menu {
                    // Handle sort menu key presses
                    use crate::data::SortMode;
                    match key.code {
                        KeyCode::Esc => {
                            app.show_sort_menu = false;
                        }
                        KeyCode::Char(c) if ('1'..='6').contains(&c) => {
                            let idx = c.to_digit(10).unwrap() as usize;
                            if let Some(mode) = SortMode::from_index(idx) {
                                app.set_sort_mode(mode);
                            }
                        }
                        _ => {}
                    }
                } else if app.show_link_menu {
                    // Handle link menu key presses
                    match key.code {
                        KeyCode::Esc => {
                            app.show_link_menu = false;
                        }
                        KeyCode::Char('1') => {
                            app.open_linear_link().await?;
                            app.show_link_menu = false;
                        }
                        KeyCode::Char('2') => {
                            app.open_github_link().await?;
                            app.show_link_menu = false;
                        }
                        KeyCode::Char('3') => {
                            app.open_vercel_link().await?;
                            app.show_link_menu = false;
                        }
                        KeyCode::Char('4') => {
                            app.teleport_to_session().await?;
                            app.show_link_menu = false;
                        }
                        _ => {}
                    }
                } else if app.show_filter_menu {
                    // Handle filter menu key presses
                    use crate::data::LinearPriority;
                    match key.code {
                        KeyCode::Esc => {
                            app.show_filter_menu = false;
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
                            app.refresh().await?;
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
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick().await;
            last_tick = std::time::Instant::now();
        }
    }
}
