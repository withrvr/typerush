//! Main menu screen — ASCII banner up top, the modes grouped one category
//! per row in the middle (↑/↓ picks a row, ←/→ an option), and a one-line hint
//! footer at the bottom.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use crossterm::event::KeyCode;

use super::{key, render_footer};
use crate::app::{menu_row, App, ClickAction};

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
    // Selected option: the theme background on accent. On the light theme that
    // is off-white on deep blue (~5.3:1, WCAG AA); black would be ~3.8:1. The
    // dark theme's background is `Reset` (terminal default), so use black there.
    let chip_fg = match theme.background {
        Color::Reset => Color::Black,
        background => background,
    };
    let selected_style = Style::default()
        .fg(chip_fg)
        .bg(theme.accent)
        .add_modifier(Modifier::BOLD);
    // Explicit fg everywhere: Reset would render the terminal default, which
    // is invisible on the light theme's white background.
    let option_style = Style::default().fg(theme.neutral);

    let selected_row = menu_row(&app.menu, app.menu_index);
    // Text area inside the border — clicks outside it can't hit an option.
    let menu_area = Rect::new(
        inner.x + 1,
        inner.y + 1,
        inner.width.saturating_sub(2),
        inner.height.saturating_sub(2),
    );
    let mut targets = app.click_targets.borrow_mut();
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
        // Inside the border: one column of border + the text so far.
        let line_y = inner.y + 1 + lines.len() as u16;
        let mut chip = |spans: &mut Vec<Span<'static>>, index: usize, text: &str| {
            let style = if index == app.menu_index {
                selected_style
            } else {
                option_style
            };
            // Pad to 4 so the time and words options line up in columns.
            let span = Span::styled(format!(" {:<4} ", text), style);
            let x = inner.x + 1 + spans.iter().map(Span::width).sum::<usize>() as u16;
            let area = Rect::new(x, line_y, span.width() as u16, 1).intersection(menu_area);
            if !area.is_empty() {
                targets.push((area, ClickAction::Menu(index)));
            }
            spans.push(span);
        };
        if row.len() == 1 {
            // Single-option row: the category name is the option itself.
            chip(&mut spans, start, group);
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
                chip(&mut spans, index, app.menu[index].label);
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
    drop(targets);

    render_footer(
        f,
        app,
        layout[3],
        &[
            ("↑/↓ category", None),
            ("←/→ option", None),
            ("Enter start", key(KeyCode::Enter)),
            ("s stats", key(KeyCode::Char('s'))),
            ("q quit", key(KeyCode::Char('q'))),
            ("? help", key(KeyCode::Char('?'))),
        ],
    );
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
