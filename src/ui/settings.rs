//! In-app Settings screen (v0.5.0).
//!
//! Three rows — theme picker, default-mode picker, reset-to-defaults —
//! navigated with ↑/↓ and changed with ←/→ or Enter (keymap in `main.rs`).
//! Value changes apply to the live UI instantly and persist to
//! `~/.typerush/config.toml` through the comment-preserving editor in
//! `config::edit`, so this screen and the `config` CLI subcommands can never
//! disagree about what a valid value is.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, SETTINGS_ROWS};

/// Render the settings screen. `app.settings_index` highlights the active row.
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let theme = &app.theme;
    let layout = Layout::vertical([
        Constraint::Length(2),                        // title
        Constraint::Length(SETTINGS_ROWS as u16 + 4), // rows card
        Constraint::Min(0),                           // spacer
        Constraint::Length(3),                        // footer
    ])
    .split(area);

    // "◆" matches the stats screen's title glyph and, unlike "⚙" (U+2699,
    // Emoji property), renders single-width on every mainstream terminal.
    let title = Paragraph::new(Span::styled(
        "  ◆ settings",
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ));
    f.render_widget(title, layout[0]);

    let rows = [
        ("theme", format!("◂ {} ▸", app.theme_name)),
        (
            "default mode",
            format!("◂ {} ▸", App::default_mode_label(app.default_mode)),
        ),
        ("reset to defaults", "(press Enter)".to_string()),
    ];
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(index, (label, value))| {
            let selected = index == app.settings_index;
            let marker = if selected { "➤ " } else { "  " };
            let label_style = if selected {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.neutral)
            };
            let value_style = if selected {
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.pending)
            };
            Line::from(vec![
                Span::styled(format!("  {marker}"), label_style),
                Span::styled(format!("{:<18}", label), label_style),
                Span::styled(value.clone(), value_style),
            ])
        })
        .collect();

    let card = Paragraph::new(lines)
        .style(Style::default().fg(theme.neutral))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.pending))
                .title(Span::styled(
                    " settings ",
                    Style::default()
                        .fg(theme.secondary)
                        .add_modifier(Modifier::BOLD),
                )),
        );
    f.render_widget(card, layout[1]);

    let note = Paragraph::new(
        "  changes apply immediately and are saved to ~/.typerush/config.toml\n  ↑/↓ select  ·  ←/→ change  ·  Enter apply  ·  esc menu",
    )
    .style(Style::default().fg(theme.pending));
    f.render_widget(note, layout[3]);
}
