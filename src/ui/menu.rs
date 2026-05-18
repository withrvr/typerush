//! Main menu screen — ASCII banner up top, a centered list of modes in the
//! middle, and a one-line hint footer at the bottom.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::App;

/// Render the menu screen. `app.menu_index` highlights the active row.
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let layout = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(8),
        Constraint::Length(3),
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
    f.render_widget(banner, layout[0]);

    let inner = centered_rect(60, 100, layout[1]);
    let items: Vec<ListItem> = app
        .menu
        .iter()
        .map(|m| ListItem::new(Line::from(m.label)))
        .collect();
    let list = List::new(items)
        .block(
            Block::default().borders(Borders::ALL).title(Span::styled(
                " select mode ",
                Style::default()
                    .fg(app.theme.secondary)
                    .add_modifier(Modifier::BOLD),
            )),
        )
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
    f.render_widget(footer, layout[2]);
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
