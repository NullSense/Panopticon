mod app;
mod input;
mod message;
pub mod search;
mod ui;

use crate::config::Config;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

pub use app::{App, ModalState, RefreshProgress, RefreshResult};
pub use message::Message;

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
    let mut input_state = input::InputState::new();

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());

        if event::poll(timeout)? {
            match event::read()? {
                Event::Resize(width, _height) => {
                    app.recalculate_column_widths(width);
                }
                Event::Key(key) => {
                    let msg = input::dispatch(app, &mut input_state, key);
                    if app.update(msg).await? {
                        return Ok(()); // Quit requested
                    }
                }
                _ => {}
            }
        }

        // Handle pending chord timeout (non-blocking)
        if input_state.has_timed_out() {
            input_state.clear();
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick().await;

            // Poll for background refresh results (non-blocking)
            app.poll_refresh();

            last_tick = std::time::Instant::now();
        }
    }
}
