use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::App;

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let layout = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(8),
        Constraint::Length(3),
    ])
    .split(area);

    // Banner
    let banner_text = vec![
        Line::from(Span::styled(
            "  ████████ ██    ██ ██████  ███████ ██████  ██    ██ ███████ ██   ██ ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "     ██     ██  ██  ██   ██ ██      ██   ██ ██    ██ ██      ██   ██ ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "     ██      ████   ██████  █████   ██████  ██    ██ ███████ ███████ ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "     ██       ██    ██      ██      ██   ██ ██    ██      ██ ██   ██ ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "     ██       ██    ██      ███████ ██   ██  ██████  ███████ ██   ██ ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
    ];
    let banner = Paragraph::new(banner_text).alignment(Alignment::Center);
    f.render_widget(banner, layout[0]);

    // Centered list
    let inner = centered_rect(60, 100, layout[1]);
    let items: Vec<ListItem> = app
        .menu
        .iter()
        .map(|m| ListItem::new(Line::from(m.label)))
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    " select mode ",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("➤ ");

    let mut state = ListState::default();
    state.select(Some(app.menu_index));
    f.render_stateful_widget(list, inner, &mut state);

    // Footer
    let footer = Paragraph::new(
        "  ↑/↓ navigate  ·  Enter start  ·  q quit  ·  ? help",
    )
    .style(Style::default().fg(Color::DarkGray))
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
