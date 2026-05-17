//! Historical stats screen — pulls the full session list from disk
//! (`~/.typerush/stats.json`) and renders a summary card, a WPM sparkline,
//! and a table of the last 10 sessions.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Paragraph, Row, Sparkline, Table},
};

use crate::{app::App, storage};

/// Render the stats history screen. Doesn't take any state from `App` —
/// everything is read from disk.
pub fn render(f: &mut Frame, _app: &App) {
    let area = f.area();
    let layout = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(5),
        Constraint::Length(7),
        Constraint::Min(5),
        Constraint::Length(3),
    ])
    .split(area);

    let title = Paragraph::new(Span::styled(
        "  ◆ stats history",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));
    f.render_widget(title, layout[0]);

    let sessions = storage::load_sessions().unwrap_or_default();
    let pb = storage::personal_best(&sessions).unwrap_or(0.0);
    let avg_acc = storage::average_accuracy(&sessions).unwrap_or(0.0);
    let total = sessions.len();
    let last_wpm = sessions.last().map(|s| s.wpm).unwrap_or(0.0);

    let summary_lines = vec![
        Line::from(vec![
            Span::styled("  best wpm     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:>6.1}", pb),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("      "),
            Span::styled("avg accuracy ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:>5.1}%", avg_acc),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("  sessions    ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:>6}", total), Style::default().fg(Color::White)),
            Span::raw("      "),
            Span::styled("last wpm     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:>5.1}", last_wpm),
                Style::default().fg(Color::Cyan),
            ),
        ]),
    ];
    let summary = Paragraph::new(summary_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" summary "),
    );
    f.render_widget(summary, layout[1]);

    let recent: Vec<u64> = sessions
        .iter()
        .rev()
        .take(20)
        .rev()
        .map(|s| s.wpm.max(0.0) as u64)
        .collect();
    let spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" wpm trend (last 20) "),
        )
        .data(&recent)
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(spark, layout[2]);

    let recent_rows = sessions
        .iter()
        .rev()
        .take(10)
        .map(|s| {
            Row::new(vec![
                Cell::from(s.timestamp.format("%Y-%m-%d %H:%M").to_string()),
                Cell::from(s.mode.clone()).style(Style::default().fg(Color::Magenta)),
                Cell::from(format!("{:.1}", s.wpm)).style(Style::default().fg(Color::Yellow)),
                Cell::from(format!("{:.1}%", s.accuracy)).style(Style::default().fg(Color::Green)),
                Cell::from(format!("{:.1}s", s.duration_secs))
                    .style(Style::default().fg(Color::Cyan)),
            ])
        })
        .collect::<Vec<_>>();

    let header = Row::new(vec!["Date", "Mode", "WPM", "Acc", "Time"]).style(
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let table = Table::new(
        recent_rows,
        [
            Constraint::Length(18),
            Constraint::Length(14),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" recent sessions "),
    );
    f.render_widget(table, layout[3]);

    let footer =
        Paragraph::new("  m / esc menu  ·  q quit").style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, layout[4]);
}
