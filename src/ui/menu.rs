//! Main menu screen — ASCII banner up top, the modes grouped one category
//! per row in the middle (↑/↓ picks a row, ←/→ an option), and a one-line hint
//! footer at the bottom.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{menu_row, App};

/// Render the menu screen. `app.menu_index` highlights the active option.
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
    let theme = &app.theme;
    // Black on accent gives high contrast on every built-in theme since each
    // one's `accent` is a bright color. A user who overrides accent to
    // something dark would lose readability here — they can override the slot.
    let selected_style = Style::default()
        .fg(Color::Black)
        .bg(theme.accent)
        .add_modifier(Modifier::BOLD);
    // Explicit fg everywhere: Reset would render the terminal default, which
    // is invisible on the light theme's white background.
    let option_style = Style::default().fg(theme.neutral);

    let selected_row = menu_row(&app.menu, app.menu_index);
    let mut lines = Vec::new();
    let mut start = 0;
    while start < app.menu.len() {
        let row = menu_row(&app.menu, start);
        let is_selected_row = row == selected_row;
        let group = app.menu[start].group;
        // Breathing room between categories; quote/zen and stats/quit pair up.
        if start > 0 && !matches!(group, "zen" | "quit") {
            lines.push(Line::raw(""));
        }

        let mut spans = vec![Span::styled(
            if is_selected_row { " ➤ " } else { "   " },
            Style::default().fg(theme.accent),
        )];
        let chip = |index: usize, text: &str| {
            let style = if index == app.menu_index {
                selected_style
            } else {
                option_style
            };
            // Pad to 4 so the time and words options line up in columns.
            Span::styled(format!(" {:<4} ", text), style)
        };
        if row.len() == 1 {
            // Single-option row: the category name is the option itself.
            spans.push(chip(start, group));
        } else {
            let heading_style = if is_selected_row {
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.pending)
            };
            spans.push(Span::styled(format!(" {:<8}", group), heading_style));
            for index in row.clone() {
                spans.push(chip(index, app.menu[index].label));
                spans.push(Span::raw(" "));
            }
        }
        lines.push(Line::from(spans));
        start = row.end;
    }

    let menu = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.neutral))
            .title(Span::styled(
                " select mode ",
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            )),
    );
    f.render_widget(menu, inner);

    let footer =
        Paragraph::new("  ↑/↓ category  ·  ←/→ option  ·  Enter start  ·  q quit  ·  ? help")
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
