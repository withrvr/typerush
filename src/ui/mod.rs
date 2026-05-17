pub mod menu;
pub mod typing;
pub mod results;
pub mod stats;
pub mod help;

use ratatui::Frame;

use crate::app::{App, Screen};

pub fn render(f: &mut Frame, app: &App) {
    match app.screen {
        Screen::Menu => menu::render(f, app),
        Screen::Typing => typing::render(f, app),
        Screen::Results => results::render(f, app),
        Screen::Stats => stats::render(f, app),
        Screen::Help => help::render(f, app),
    }
    if let Some(err) = &app.error_message {
        help::render_error(f, err);
    }
}
