//! UI dispatcher.
//!
//! Each screen lives in its own submodule and exposes a `render(frame, app)`
//! function. The top-level `render` here just looks at `app.screen` and calls
//! the right one. The Help overlay and any error modal are drawn on top of
//! whatever screen is underneath.
//!
//! Before any screen renders, we paint the whole frame area with the active
//! theme's background. This is what makes the `light` theme actually look
//! light on a dark terminal (and what gives `monokai` / `dracula` their
//! canonical backgrounds regardless of terminal config). The `dark` theme
//! uses `Color::Reset` here so the user's terminal background shines through
//! exactly as it did in v0.1.

pub mod help;
pub mod menu;
pub mod results;
pub mod stats;
pub mod typing;

use ratatui::{prelude::*, widgets::Block, Frame};

use crate::app::{App, Screen};

/// Single entry point called once per frame from the main event loop.
pub fn render(frame: &mut Frame, app: &App) {
    // 1. Paint the theme's background across the entire screen.
    let area = frame.area();
    let background_style = Style::default().bg(app.theme.background);
    frame.render_widget(Block::default().style(background_style), area);

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
