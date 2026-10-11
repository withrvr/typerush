//! Two floating overlays drawn on top of any screen:
//!  - the keybindings help (toggled with `?` / F1, closed by a click too)
//!  - a transient error or notice modal (dismissed by any keypress or click)

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};

use ratatui::crossterm::event::KeyCode;

use crate::app::App;

/// Render the keybindings overlay. Sized to its content (23 rows, so it fits
/// an 80×24 window whole). On a smaller terminal the box shrinks and the
/// bottom of the list is cut, but the close hint lives in the bottom border
/// and is always visible.
/// Returns the footer hints.
pub fn render(f: &mut Frame, app: &App, screen: Rect) -> &'static [super::Hint] {
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
        Line::from("  ↑/↓ ←/→  hjkl    move: menu, stats, settings"),
        Line::from("  Enter  Space     start, choose, change"),
        Line::from("  Ctrl+R  F5       restart (results: Enter too)"),
        Line::from("  Ctrl+P  F2       pause / resume a run"),
        Line::from("  Esc              finish a run / back"),
        Line::from("  Ctrl+Bksp Ctrl+W delete word"),
        Line::from("  p  n  b          menu: punctuation, numbers,"),
        Line::from("                   10k words (time & words runs)"),
        Line::from("  Tab  s           stats (only Tab on results)"),
        Line::from("  c                menu: settings"),
        Line::from("  Enter  e         stats: replay, export CSV"),
        Line::from("  a  PgUp/Dn       stats: all sessions, scroll"),
        Line::from("  ?  F1            help (only F1 on results)"),
        Line::from("  Ctrl+C           quit"),
        Line::from("  Mouse            click options, rows, hints;"),
        Line::from("                   the wheel scrolls lists"),
        Line::raw(""),
        dim("  Stats   ~/.typerush/stats.json (not zen)"),
        dim("  Config  ~/.typerush/config.toml"),
        dim("  Snippets ~/.typerush/snippets/*.txt"),
    ];

    const WIDTH: u16 = 50;
    // Narrower than the box, lines wrap and need more rows: take the full
    // height instead of guessing how many.
    let height = if screen.width < WIDTH {
        screen.height
    } else {
        lines.len() as u16 + 2
    };
    let area = screen.centered(Constraint::Length(WIDTH), Constraint::Length(height));
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
                .title(" help ")
                .title_bottom(Span::styled(
                    " Esc / ? / F1 / click to close ",
                    Style::default().fg(theme.pending),
                )),
        );
    f.render_widget(p, area);
    const HINTS: &[super::Hint] = &[("Esc / F1 close", super::key(KeyCode::Esc))];
    HINTS
}

/// Render a transient modal — an error, or a notice — titled `title` with a
/// `border` color. Dismissed by any keypress or click from the main loop.
/// Tall enough for the wrapped message plus the hint line.
pub fn render_message(f: &mut Frame, title: &'static str, border: Color, message: &str) {
    use unicode_width::UnicodeWidthStr;
    // Messages can carry file paths, which may contain control characters:
    // made safe line by line, so a multi-line message (a TOML parse error
    // points at the bad line with a caret) keeps its layout. The text is
    // inset by the box's padding, so a wrapped line keeps the inset too.
    let lines: Vec<String> = message
        .lines()
        .map(|line| crate::text::printable(line).into_owned())
        .collect();
    const WIDTH: u16 = 60;
    // Inside the borders and two cells of padding each side.
    let text_width = WIDTH.min(f.area().width).saturating_sub(6).max(1) as usize;
    // Rows the message takes once each line wraps; slightly over is fine,
    // the box just has a gap.
    let message_rows: usize = lines
        .iter()
        .map(|line| line.width().div_ceil(text_width).max(1))
        .sum::<usize>()
        .max(1);
    let height = message_rows as u16 + 4; // + blank + hint + borders
    let area = f
        .area()
        .centered(Constraint::Length(WIDTH), Constraint::Length(height));
    f.render_widget(Clear, area);
    let p = Paragraph::new(format!("{}\n\npress any key or click", lines.join("\n")))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .padding(Padding::horizontal(2))
                .border_style(Style::default().fg(border))
                .title(title),
        );
    f.render_widget(p, area);
}
