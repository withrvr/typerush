//! Two floating overlays drawn on top of any screen:
//!  - the keybindings help (toggled with `?` / F1, closed by a click too)
//!  - a transient error modal (dismissed by any keypress or click)

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{app::App, theme::ThemePalette};

/// Render the keybindings overlay. Sized to its content (23 rows, so it fits
/// an 80×24 window whole). On a smaller terminal the box shrinks and the
/// bottom of the list is cut, but the close hint lives in the bottom border
/// and is always visible.
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
        Line::from("  p  n  b          menu: punctuation, numbers,"),
        Line::from("                   10k words (time & words runs)"),
        Line::from("  Tab  s           stats history"),
        Line::from("  ←/→ ↑/↓ PgUp/Dn  stats: category, scroll"),
        Line::from("  ?  F1            toggle this help"),
        Line::from("  Ctrl+C           quit"),
        Line::from("  Mouse            click options and footer"),
        Line::from("                   hints; wheel scrolls lists"),
        Line::raw(""),
        dim("  Zen sessions are not saved"),
        dim("  Stats   ~/.typerush/stats.json"),
        dim("  Config  ~/.typerush/config.toml"),
        dim("  Snippets ~/.typerush/snippets/*.txt"),
    ];

    const WIDTH: u16 = 50;
    // Narrower than the box, lines wrap and need more rows: take the full
    // height instead of guessing how many.
    let height = if f.area().width < WIDTH {
        f.area().height
    } else {
        lines.len() as u16 + 2
    };
    let area = f
        .area()
        .centered(Constraint::Length(WIDTH), Constraint::Length(height));
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
}

/// Render the transient error modal. Dismissed by any keypress or click from
/// the main loop. Tall enough for the wrapped message plus the hint line.
pub fn render_error(f: &mut Frame, theme: &ThemePalette, message: &str) {
    use unicode_width::UnicodeWidthStr;
    // Messages can carry file paths, which may contain control characters.
    // Made safe and indented line by line, so a multi-line message (a TOML
    // parse error points at the bad line with a caret) keeps its layout.
    let lines: Vec<String> = message
        .lines()
        .map(|line| format!("  {}", crate::text::printable(line)))
        .collect();
    const WIDTH: u16 = 60;
    let text_width = WIDTH.min(f.area().width).saturating_sub(2).max(1) as usize;
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
    let p = Paragraph::new(format!("{}\n\n  press any key or click", lines.join("\n")))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.error))
                .title(" error "),
        );
    f.render_widget(p, area);
}
