//! Results screen — shown immediately after a session ends.
//!
//! Displays final WPM, accuracy, elapsed time, character counts, the chosen
//! mode, the user's all-time best, and (v0.3.0) the per-mode personal best.
//! Includes a +/- delta against the previous session and a mini sparkline of
//! recent WPM scores.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Sparkline},
};

use crate::{app::App, storage};

/// Render the post-session results screen.
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let theme = &app.theme;
    let layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    let title = Paragraph::new(Span::styled(
        "  ✓ session complete",
        Style::default()
            .fg(theme.correct)
            .add_modifier(Modifier::BOLD),
    ));
    f.render_widget(title, layout[0]);

    let wpm = app.wpm();
    let acc = app.accuracy();
    let elapsed = app.elapsed().as_secs_f64();
    let mode_label = app.mode.label();

    let sessions = storage::load_sessions().unwrap_or_default();

    // Delta vs the previous any-mode session (skip the one we just saved).
    let last_wpm = sessions.iter().rev().nth(1).map(|s| s.wpm);
    let delta = match last_wpm {
        Some(prev) => {
            let diff = wpm - prev;
            let sign = if diff >= 0.0 { "+" } else { "" };
            let color = if diff >= 0.0 {
                theme.correct
            } else {
                theme.incorrect
            };
            Span::styled(
                format!(" ({sign}{:.0} vs last)", diff),
                Style::default().fg(color),
            )
        }
        None => Span::raw(""),
    };

    // Per-mode personal best (v0.3.0).
    // Compare current session's WPM against all *previous* sessions of the
    // same mode (skipping the one just saved) to detect a new mode PB.
    let prev_mode_pb = sessions
        .iter()
        .rev()
        .skip(1) // skip the session we just saved
        .filter(|s| s.mode == mode_label)
        .map(|s| s.wpm)
        .fold(None::<f64>, |best, w| {
            Some(best.map_or(w, |b: f64| b.max(w)))
        });

    let is_new_mode_pb = match prev_mode_pb {
        None => true, // first session for this mode
        Some(prev) => wpm > prev,
    };
    let mode_pb_now = storage::personal_best_for_mode(&sessions, &mode_label).unwrap_or(wpm);

    let pb_label = format!("  {:>13}", format!("{} best", mode_label));
    let pb_value = Span::styled(
        format!("{:>6.1} wpm", mode_pb_now),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    );
    let new_pb_badge = if is_new_mode_pb {
        Span::styled(
            "  ★ new best!",
            Style::default()
                .fg(theme.correct)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("")
    };

    let body_lines = vec![
        Line::from(vec![
            Span::styled("  wpm        ", Style::default().fg(theme.pending)),
            Span::styled(
                format!("{:>6.1}", wpm),
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            ),
            delta,
        ]),
        Line::from(vec![
            Span::styled("  accuracy   ", Style::default().fg(theme.pending)),
            Span::styled(format!("{:>6.1}%", acc), Style::default().fg(theme.correct)),
        ]),
        Line::from(vec![
            Span::styled("  time       ", Style::default().fg(theme.pending)),
            Span::styled(
                format!("{:>5.1}s", elapsed),
                Style::default().fg(theme.accent),
            ),
        ]),
        Line::from(vec![
            Span::styled("  chars      ", Style::default().fg(theme.pending)),
            Span::styled(
                format!("{}/{}", app.correct_chars, app.total_typed_chars),
                Style::default().fg(theme.neutral),
            ),
        ]),
        Line::from(vec![
            Span::styled("  mode       ", Style::default().fg(theme.pending)),
            Span::styled(&mode_label, Style::default().fg(theme.mode_tag)),
        ]),
        Line::from(vec![
            Span::styled(pb_label, Style::default().fg(theme.pending)),
            pb_value,
            new_pb_badge,
        ]),
    ];
    let body = Paragraph::new(body_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.pending))
            .title(" results "),
    );
    f.render_widget(body, layout[1]);

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
                .border_style(Style::default().fg(theme.pending))
                .title(" recent wpm trend "),
        )
        .data(&recent)
        .style(Style::default().fg(theme.accent));
    f.render_widget(spark, layout[2]);

    let footer = Paragraph::new("  Enter / r restart  ·  m menu  ·  s stats  ·  q quit")
        .style(Style::default().fg(theme.pending));
    f.render_widget(footer, layout[4]);
}
