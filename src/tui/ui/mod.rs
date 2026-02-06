//! TUI rendering module.
//!
//! This module handles all UI rendering for the terminal interface.
//! It's organized into submodules for maintainability:
//!
//! - `icons` - Nerd Font icons used throughout the UI
//! - `layout` - Layout calculations and text utilities
//! - `status` - Status configuration and status bar rendering
//! - `table` - Issue table rendering (header, workstreams)
//! - `modals` - Modal popup rendering (help, links, description)
//! - `menus` - Menu rendering (sort, filter)

pub mod icons;
pub mod layout;
mod menus;
mod modals;
mod status;
mod table;

// Re-export the main draw function
pub use self::draw::draw;

mod draw {

    use super::menus::{draw_filter_menu, draw_sort_menu};
    use super::modals::{draw_description_modal, draw_link_menu, draw_links_popup};
    use super::status::{draw_help_popup, draw_status_bar};
    use super::table::{draw_header, draw_workstreams};
    use crate::tui::App;
    use ratatui::{
        layout::{Constraint, Direction, Layout},
        Frame,
    };

    /// Main draw function - renders the entire TUI.
    pub fn draw(f: &mut Frame, app: &App) {
        let has_tabs = app.view_configs.len() > 1;

        let chunks = if has_tabs {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Header/search
                    Constraint::Length(1), // Tab bar
                    Constraint::Min(0),    // Main content
                    Constraint::Length(1), // Status bar
                ])
                .split(f.area())
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Header/search
                    Constraint::Min(0),    // Main content
                    Constraint::Length(1), // Status bar
                ])
                .split(f.area())
        };

        if has_tabs {
            draw_header(f, app, chunks[0]);
            super::status::draw_view_tabs(f, app, chunks[1]);
            draw_workstreams(f, app, chunks[2]);
            draw_status_bar(f, app, chunks[3]);
        } else {
            draw_header(f, app, chunks[0]);
            draw_workstreams(f, app, chunks[1]);
            draw_status_bar(f, app, chunks[2]);
        }

        // Overlays
        if app.show_help() {
            draw_help_popup(f, app);
        }

        if app.show_link_menu() {
            draw_link_menu(f, app);
            if app.show_links_popup() {
                draw_links_popup(f, app);
            }
        }

        if app.show_sort_menu() {
            draw_sort_menu(f, app);
        }

        if app.show_filter_menu() {
            draw_filter_menu(f, app);
        }

        if app.show_description_modal() {
            draw_description_modal(f, app);
        }
    }
}
