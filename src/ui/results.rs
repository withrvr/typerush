//! Results screen — shown immediately after a session ends.
//!
//! Displays final WPM, accuracy, elapsed time, character counts, the chosen
//! mode, and the user's all-time best WPM. Includes a +/- delta against the
//! previous session and a mini sparkline of recent WPM scores.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Sparkline},
};

use crate::{app::App, storage};

/// Render the post-session results screen.
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(9),
        Constraint::Length(6),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    let title = Paragraph::new(Span::styled(
        "  ✓ session complete",
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    ));
    f.render_widget(title, layout[0]);

    let wpm = app.wpm();
    let acc = app.accuracy();
    let elapsed = app.elapsed().as_secs_f64();

    let sessions = storage::load_sessions().unwrap_or_default();
    let pb = storage::personal_best(&sessions).unwrap_or(0.0);
    let last_wpm = sessions.iter().rev().nth(1).map(|s| s.wpm);
    let delta = match last_wpm {
        Some(prev) => {
            let diff = wpm - prev;
            let sign = if diff >= 0.0 { "+" } else { "" };
            let color = if diff >= 0.0 {
                Color::Green
            } else {
                Color::Red
            };
            Span::styled(
                format!(" ({sign}{:.0} vs last)", diff),
                Style::default().fg(color),
            )
        }
        None => Span::raw(""),
    };

    let body_lines = vec![
        Line::from(vec![
            Span::styled("  wpm        ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:>6.1}", wpm),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            delta,
        ]),
        Line::from(vec![
            Span::styled("  accuracy   ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:>6.1}%", acc), Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  time       ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:>5.1}s", elapsed),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled("  chars      ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}/{}", app.correct_chars, app.total_typed_chars),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  mode       ", Style::default().fg(Color::DarkGray)),
            Span::styled(app.mode.label(), Style::default().fg(Color::Magenta)),
        ]),
        Line::from(vec![
            Span::styled("  best ever  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:>6.1} wpm", pb),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];
    let body = Paragraph::new(body_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" results "),
    );
    f.render_widget(body, layout[1]);

    // Sparkline of recent WPM
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
                .title(" recent wpm trend "),
        )
        .data(&recent)
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(spark, layout[2]);

    let footer = Paragraph::new("  Enter / r restart  ·  m menu  ·  s stats  ·  q quit")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, layout[4]);
}
