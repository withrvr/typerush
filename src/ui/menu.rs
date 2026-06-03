//! Main menu screen — ASCII banner up top, a centered list of modes in the
//! middle, and a one-line hint footer at the bottom.
//!
//! The list contains two row types: regular start-mode rows and
//! `MenuAction::Separator` rows. Separators render as muted section labels
//! (e.g. "── Time ──") and are skipped by the keyboard navigation handler in
//! `main.rs`, so the user never lands on one with `Enter`.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{App, MenuAction};

/// Render the menu screen. `app.menu_index` highlights the active row.
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    // Layout: a small breathing-room strip, then the banner, then the
    // mode list, then a small bottom margin, then the footer. The top
    // padding stops the banner from hugging the terminal's title-bar.
    let layout = Layout::vertical([
        Constraint::Length(2), // top padding
        Constraint::Length(5), // banner
        Constraint::Min(8),    // mode list
        Constraint::Length(3), // footer
    ])
    .split(area);

    let banner_style = Style::default()
        .fg(app.theme.accent)
        .add_modifier(Modifier::BOLD);
    let banner_text = vec![
        Line::from(Span::styled(
            "  ████████ ██    ██ ██████  ███████ ██████  ██    ██ ███████ ██   ██ ",
            banner_style,
        )),
        Line::from(Span::styled(
            "     ██     ██  ██  ██   ██ ██      ██   ██ ██    ██ ██      ██   ██ ",
            banner_style,
        )),
        Line::from(Span::styled(
            "     ██      ████   ██████  █████   ██████  ██    ██ ███████ ███████ ",
            banner_style,
        )),
        Line::from(Span::styled(
            "     ██       ██    ██      ██      ██   ██ ██    ██      ██ ██   ██ ",
            banner_style,
        )),
        Line::from(Span::styled(
            "     ██       ██    ██      ███████ ██   ██  ██████  ███████ ██   ██ ",
            banner_style,
        )),
    ];
    let banner = Paragraph::new(banner_text).alignment(Alignment::Center);
    f.render_widget(banner, layout[1]);

    let inner = centered_rect(60, 100, layout[2]);
    // Borrow each label (`&str`) rather than cloning a fresh `String` per
    // frame — the menu re-renders at 10 fps and dozens of small allocations
    // would be wasted churn for static text.
    let items: Vec<ListItem> = app
        .menu
        .iter()
        .map(|m| {
            if matches!(m.action, MenuAction::Separator) {
                // Section headers: italicised, muted, no highlight target.
                let span = Span::styled(
                    m.label.as_str(),
                    Style::default()
                        .fg(app.theme.pending)
                        .add_modifier(Modifier::ITALIC),
                );
                ListItem::new(Line::from(span))
            } else {
                ListItem::new(Line::from(m.label.as_str()))
            }
        })
        .collect();
    let list = List::new(items)
        // Unselected items inherit this fg. Without it, ratatui leaves cells
        // with fg=Reset and the terminal renders its default fg — which is
        // usually white on a dark terminal, invisible on the light theme's
        // white background.
        .style(Style::default().fg(app.theme.neutral))
        .block(
            Block::default().borders(Borders::ALL).title(Span::styled(
                " select mode ",
                Style::default()
                    .fg(app.theme.secondary)
                    .add_modifier(Modifier::BOLD),
            )),
        )
        // Black on accent gives high contrast on every built-in theme since
        // each one's `accent` is a bright color. A user who overrides accent
        // to something dark would lose readability here — at that point they
        // can override the slot.
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("➤ ");

    let mut state = ListState::default();
    state.select(Some(app.menu_index));
    f.render_stateful_widget(list, inner, &mut state);

    let footer = Paragraph::new("  ↑/↓ navigate  ·  Enter start  ·  q quit  ·  ? help")
        .style(Style::default().fg(app.theme.pending))
        .wrap(Wrap { trim: true });
    f.render_widget(footer, layout[3]);
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
