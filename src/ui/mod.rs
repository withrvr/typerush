//! UI dispatcher.
//!
//! Each screen lives in its own submodule and exposes a `render(frame, app)`
//! function. The top-level `render` here just looks at `app.screen` and calls
//! the right one. The Help overlay and any error modal are drawn on top of
//! whatever screen is underneath.
//!
//! Before any screen renders, we paint **every cell** in the frame buffer
//! with the active theme's background. We use `buffer_mut().set_style` rather
//! than rendering a Block widget so the bg fill is unconditional — no widget
//! render path gets a chance to skip cells. The `dark` theme uses
//! `Color::Reset` here so the user's terminal background shines through
//! exactly as it did in v0.1.
//!
//! Note: some terminals add a few pixels of padding *around* the character
//! grid (e.g. Windows Terminal defaults to 8px). That padding lives outside
//! anything ratatui can paint — if a user reports a thin stripe along the
//! edges in non-default themes, it's the terminal's padding setting, not
//! TypeRush.

pub mod help;
pub mod menu;
pub mod results;
pub mod stats;
pub mod typing;

use ratatui::{prelude::*, Frame};

use crate::app::{App, Screen};

/// Single entry point called once per frame from the main event loop.
pub fn render(frame: &mut Frame, app: &App) {
    // 1. Paint every cell in the frame with the theme background.
    let area = frame.area();
    frame
        .buffer_mut()
        .set_style(area, Style::default().bg(app.theme.background));

    // 2. Render the active screen on top of the painted background.
    match app.screen {
        Screen::Menu => menu::render(frame, app),
        Screen::Typing => typing::render(frame, app),
        Screen::Results => results::render(frame, app),
        Screen::Stats => stats::render(frame, app),
        Screen::Help => help::render(frame, app),
    }
    // 3. Error overlay sits on top of everything else when present.
    if let Some(message) = &app.error_message {
        help::render_error(frame, &app.theme, message);
    }
}
