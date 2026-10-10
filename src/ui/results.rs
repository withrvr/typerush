//! Results screen — shown immediately after a session ends.
//!
//! Displays final WPM, accuracy, elapsed time, character counts, the chosen
//! mode, and (since v0.3.0) the personal best for that mode — which replaced
//! the all-time best across all modes; the all-time best is on the Stats
//! screen. Includes a +/- delta against the previous session and a mini
//! sparkline of recent WPM scores.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Sparkline},
};

use ratatui::crossterm::event::KeyCode;

use super::key;
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
    let mode_label = app.session_label();

    let sessions: &[storage::SessionRecord] = app.stats_cache.as_deref().unwrap_or(&[]);
    // Normally computed once on screen entry (main loop); computed here only
    // if something renders Results without going through it (e.g. tests).
    let comparison = app.results_comparison.unwrap_or_else(|| {
        storage::ResultsComparison::new(sessions, app.session_just_saved, &mode_label)
    });

    // Delta vs the previous any-mode session.
    let delta = match comparison.last_wpm {
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
    // Zen sessions are never saved, so there's no meaningful per-mode PB for
    // Zen — show a "no record kept" placeholder instead.
    let is_zen = matches!(app.mode, crate::app::Mode::Zen);

    // Compare current session's WPM against all *previous* sessions of the
    // same mode to detect a new mode PB. Only a session that was actually
    // recorded can claim a new best — otherwise a 0.9-second sprint could
    // flash "new best!" for a record that was never kept.
    let prev_mode_pb = comparison.previous_mode_best;

    let is_new_mode_pb = app.session_just_saved
        && match prev_mode_pb {
            None => true, // first session for this mode
            Some(prev) => wpm > prev,
        };

    let pb_line = if is_zen {
        Line::from(vec![
            Span::styled("  mode best  ", Style::default().fg(theme.pending)),
            Span::styled("  — (zen not saved)", Style::default().fg(theme.pending)),
        ])
    } else {
        // Best including this session if it was recorded. No recorded session
        // for this mode yet (e.g. this one was too short to save) → `—`.
        let mode_pb_now = match (app.session_just_saved, prev_mode_pb) {
            (true, previous) => Some(previous.map_or(wpm, |best| best.max(wpm))),
            (false, previous) => previous,
        };
        let pb_value = Span::styled(
            mode_pb_now.map_or("     — wpm".to_string(), |pb| format!("{:>6.1} wpm", pb)),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        );
        let badge = if is_new_mode_pb {
            Span::styled(
                "  ★ new best!",
                Style::default()
                    .fg(theme.correct)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::raw("")
        };
        Line::from(vec![
            // The mode is named on the row above; a fixed-width label keeps
            // the value in the same column as wpm/accuracy for every mode.
            Span::styled("  mode best  ", Style::default().fg(theme.pending)),
            pb_value,
            badge,
        ])
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
        pb_line,
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

    super::render_footer(
        f,
        app,
        layout[4],
        &[
            ("Enter / r restart", key(KeyCode::Enter)),
            ("m / esc menu", key(KeyCode::Char('m'))),
            ("s / tab stats", key(KeyCode::Char('s'))),
            ("q quit", key(KeyCode::Char('q'))),
            ("? help", key(KeyCode::Char('?'))),
        ],
    );
}
