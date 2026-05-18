//! UI dispatcher.
//!
//! Each screen lives in its own submodule and exposes a `render(frame, app)`
//! function. The top-level `render` here just looks at `app.screen` and calls
//! the right one. The Help overlay and any error modal are drawn on top of
//! whatever screen is underneath.

pub mod help;
pub mod menu;
pub mod results;
pub mod stats;
pub mod typing;

use ratatui::Frame;

use crate::app::{App, Screen};

/// Single entry point called once per frame from the main event loop.
pub fn render(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Menu => menu::render(frame, app),
        Screen::Typing => typing::render(frame, app),
        Screen::Results => results::render(frame, app),
        Screen::Stats => stats::render(frame, app),
        Screen::Help => help::render(frame, app),
    }
    // Error overlay sits on top of everything else when present.
    if let Some(message) = &app.error_message {
        help::render_error(frame, &app.theme, message);
    }
}
