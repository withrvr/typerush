//! Two floating overlays drawn on top of any screen:
//!  - the keybindings help (toggled with `?`)
//!  - a transient error modal (dismissed by any keypress)

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{app::App, theme::ThemePalette};

/// Render the keybindings overlay.
pub fn render(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 70, f.area());
    f.render_widget(Clear, area);
    let theme = &app.theme;

    let lines = vec![
        Line::from(Span::styled(
            "  TypeRush — keybindings",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::from("  ↑/↓ or j/k     navigate menu"),
        Line::from("  Enter          start / restart"),
        Line::from("  Esc            back to menu / end session"),
        Line::from("  Ctrl+P         pause / resume the session"),
        Line::from("  Ctrl+R         restart current mode"),
        Line::from("  Ctrl+Backspace delete previous word"),
        Line::from("  Ctrl+C         quit immediately"),
        Line::from("  Tab            stats screen (from menu/results)"),
        Line::from("  p              replay last session (results screen)"),
        Line::from("  Enter          replay selected session (stats screen)"),
        Line::from("  ?              toggle this help"),
        Line::raw(""),
        Line::from(Span::styled(
            "  Modes",
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  Daily          seed-of-the-day challenge — same words for everyone"),
        Line::from("  Time           type as many words as you can"),
        Line::from("  Words          type a fixed number of words"),
        Line::from("  Quote          a famous programming quote"),
        Line::from("  Code           real Rust / Python / JS / Go / Java / SQL / Shell snippets"),
        Line::from("  Symbols        drill punctuation: (){};=> and friends"),
        Line::from("  Zen            no timer, no stats — just flow"),
        Line::from(
            "  Custom         your own text file — drop .txt files in ~/.typerush/snippets/",
        ),
        Line::raw(""),
        Line::from(Span::styled(
            "  Stats saved to ~/.typerush/stats.json",
            Style::default().fg(theme.pending),
        )),
        Line::from(Span::styled(
            "  Config: ~/.typerush/config.toml",
            Style::default().fg(theme.pending),
        )),
        Line::from(Span::styled(
            "  Snippets: ~/.typerush/snippets/*.txt",
            Style::default().fg(theme.pending),
        )),
    ];

    let p = Paragraph::new(lines)
        // Plain Line::from(string) entries inherit this fg — otherwise they
        // render with terminal default which is invisible on the light theme.
        // Explicitly-styled spans (titles, subtitles, dim hints) keep their
        // own colors because Span style overrides Paragraph style.
        .style(Style::default().fg(theme.neutral))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent))
                .title(" help "),
        );
    f.render_widget(p, area);
}

/// Render the transient error modal. Dismissed by any keypress from the main loop.
pub fn render_error(f: &mut Frame, theme: &ThemePalette, message: &str) {
    let area = centered_rect(50, 20, f.area());
    f.render_widget(Clear, area);
    let p = Paragraph::new(format!("  {}\n\n  press any key", message))
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
