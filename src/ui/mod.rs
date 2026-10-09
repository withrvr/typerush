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

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{prelude::*, widgets::Paragraph, Frame};

use crate::app::{App, ClickAction, Screen};

/// Single entry point called once per frame from the main event loop.
pub fn render(frame: &mut Frame, app: &App) {
    // 0. Click targets are rebuilt from scratch by whatever draws this frame.
    app.click_targets.borrow_mut().clear();

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

/// A footer hint: its text and, when clicking it should do something, the key
/// press it stands for. Navigation hints like "↑/↓ category" carry `None`.
pub type Hint = (&'static str, Option<(KeyCode, KeyModifiers)>);

/// Shorthand for a clickable hint that acts like pressing `code`.
pub const fn key(code: KeyCode) -> Option<(KeyCode, KeyModifiers)> {
    Some((code, KeyModifiers::NONE))
}

/// Draw a one-line key-hint footer ("Enter start  ·  q quit") and register
/// each actionable hint as a click target, so every footer action works with
/// the keyboard *and* the mouse.
pub fn render_footer(frame: &mut Frame, app: &App, area: Rect, hints: &[Hint]) {
    let style = Style::default().fg(app.theme.pending);
    let mut spans = vec![Span::raw("  ")];
    let mut x = area.x + 2;
    let mut targets = app.click_targets.borrow_mut();
    for (i, (text, action)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  ·  "));
            x += 5;
        }
        let span = Span::styled(*text, style);
        let width = span.width() as u16;
        if let Some((code, mods)) = action {
            targets.push((
                Rect::new(x, area.y, width, 1).intersection(area),
                ClickAction::Key(*code, *mods),
            ));
        }
        spans.push(span);
        x += width;
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(style), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load::DefaultMode;
    use crate::theme::builtin;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// Render the menu screen into a fake terminal and return the cell at
    /// `(x, y)`. Far-corner cells aren't touched by any widget on the menu
    /// screen, so they reflect the theme's background paint directly.
    fn render_and_sample(palette: crate::theme::ThemePalette, x: u16, y: u16) -> (Color, Color) {
        let app = App::new(None, palette, DefaultMode::Time(15));
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let cell = &buffer[(x, y)];
        (cell.fg, cell.bg)
    }

    /// v0.1 compatibility: the dark theme must NOT force any explicit bg.
    /// `Color::Reset` lets the user's terminal background shine through —
    /// regressing this would change the look on every terminal.
    #[test]
    fn dark_theme_leaves_background_as_reset() {
        let (_fg, bg) = render_and_sample(builtin::DARK, 79, 23);
        assert_eq!(bg, Color::Reset);
    }

    /// Non-default themes paint their canonical background on every cell.
    /// Sampling a far-corner cell guarantees we're reading the painted bg
    /// rather than something a widget happened to render there.
    #[test]
    fn monokai_theme_paints_canonical_background() {
        let (_fg, bg) = render_and_sample(builtin::MONOKAI, 79, 23);
        assert_eq!(bg, Color::Rgb(0x27, 0x28, 0x22));
    }

    #[test]
    fn dracula_theme_paints_canonical_background() {
        let (_fg, bg) = render_and_sample(builtin::DRACULA, 79, 23);
        assert_eq!(bg, Color::Rgb(0x28, 0x2A, 0x36));
    }

    #[test]
    fn light_theme_paints_off_white_background() {
        let (_fg, bg) = render_and_sample(builtin::LIGHT, 79, 23);
        assert_eq!(bg, Color::Rgb(0xFA, 0xFA, 0xFA));
    }

    /// Regression (#8): a wrong key on a space must show as a red, underlined
    /// space — a plain colored blank would be invisible.
    #[test]
    fn wrong_space_renders_red_underlined() {
        let palette = builtin::DARK;
        let mut app = App::new(None, palette, DefaultMode::Time(15));
        app.mode = crate::app::Mode::Custom;
        app.words = vec![
            crate::app::Word::new("hi".into()),
            crate::app::Word::new("bye".into()),
        ];
        app.screen = crate::app::Screen::Typing;
        for ch in "hix".chars() {
            app.handle_char(ch);
        }

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, &app)).unwrap();
        let buffer = terminal.backend().buffer();

        let has_red_space = buffer.content().iter().any(|cell| {
            cell.symbol() == " "
                && cell.fg == palette.incorrect
                && cell.modifier.contains(Modifier::UNDERLINED)
        });
        assert!(has_red_space);
    }
}
