//! Two floating overlays drawn on top of any screen:
//!  - the keybindings help (toggled with `?` / F1, closed by a click too)
//!  - a transient error modal (dismissed by any keypress or click)

use ratatui::{
    layout::Flex,
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{app::App, theme::ThemePalette};

/// Render the keybindings overlay. Sized to its content (clamped to the
/// terminal) so it fits a standard 80×24 window without clipping.
pub fn render(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let heading = |text| {
        Line::from(Span::styled(
            text,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
    };
    let dim = |text| Line::from(Span::styled(text, Style::default().fg(theme.pending)));

    let lines = vec![
        heading("  TypeRush — keybindings"),
        Line::raw(""),
        Line::from("  ↑/↓  j/k         menu: pick a category"),
        Line::from("  ←/→  h/l         menu: pick an option"),
        Line::from("  Enter  Space     start the selected mode"),
        Line::from("  Enter  r         restart (results screen)"),
        Line::from("  Ctrl+R  F5       restart while typing"),
        Line::from("  Esc              finish session / back"),
        Line::from("  Ctrl+Bksp Ctrl+W delete word"),
        Line::from("  Tab  s           stats history"),
        Line::from("  ?  F1            toggle this help"),
        Line::from("  Ctrl+C           quit"),
        Line::from("  Mouse            click options and footer"),
        Line::from("                   hints; wheel moves the menu"),
        Line::raw(""),
        dim("  Zen sessions are not saved"),
        dim("  Stats   ~/.typerush/stats.json"),
        dim("  Config  ~/.typerush/config.toml"),
        Line::raw(""),
        dim("  Esc / ? / F1 / click to close"),
    ];

    const WIDTH: u16 = 50;
    // Narrower than the box, lines wrap and need more rows: take the full
    // height instead of guessing how many.
    let height = if f.area().width < WIDTH {
        f.area().height
    } else {
        lines.len() as u16 + 2
    };
    let area = centered(f.area(), WIDTH, height);
    f.render_widget(Clear, area);
    let p = Paragraph::new(lines)
        // Plain Line::from(string) entries inherit this fg — otherwise they
        // render with terminal default which is invisible on the light theme.
        // Explicitly-styled spans (titles, dim hints) keep their own colors
        // because Span style overrides Paragraph style.
        .style(Style::default().fg(theme.neutral))
        // Narrower than 50 columns, the box shrinks: wrap rather than cut
        // lines off (including the "how to close" line).
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent))
                .title(" help "),
        );
    f.render_widget(p, area);
}

/// A `width`×`height` rect centered in `r`, shrunk to fit if `r` is smaller.
fn centered(r: Rect, width: u16, height: u16) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(r);
    let [area] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    area
}

/// Render the transient error modal. Dismissed by any keypress from the main loop.
pub fn render_error(f: &mut Frame, theme: &ThemePalette, message: &str) {
    let area = centered_rect(50, 20, f.area());
    f.render_widget(Clear, area);
    let p = Paragraph::new(format!("  {}\n\n  press any key or click", message))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.error))
                .title(" error "),
        );
    f.render_widget(p, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
