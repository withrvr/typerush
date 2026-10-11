//! Results screen — shown immediately after a session ends.
//!
//! Displays final WPM, accuracy, elapsed time, character counts, the chosen
//! mode, and (since v0.3.0) the personal best for that mode — which replaced
//! the all-time best across all modes; the all-time best is on the Stats
//! screen. Includes a +/- delta against the previous session, raw WPM,
//! consistency, a character breakdown and (v0.6.0) a chart of the run
//! second by second.

use ratatui::{
    prelude::*,
    symbols::Marker,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph},
};

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use super::{key, label};
use crate::{app::App, storage};

/// Render the post-session results screen.
/// Returns the footer hints.
pub fn render(f: &mut Frame, app: &App, area: Rect) -> &'static [super::Hint] {
    let theme = &app.theme;
    // Title row and a blank row (as on every screen), the results box sized
    // to its eight lines, then the run's chart in the rest.
    let layout = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(10),
        Constraint::Min(0),
    ])
    .split(area);

    let sessions: &[storage::SessionRecord] = app.stats_cache.as_deref().unwrap_or(&[]);
    let mode_label = app.session_label();
    // Normally computed once on screen entry (main loop); computed here only
    // if something renders Results without going through it (e.g. tests).
    let comparison = app.results_comparison.unwrap_or_else(|| {
        storage::ResultsComparison::new(
            sessions,
            app.session_just_saved,
            &mode_label,
            app.best_text().as_deref(),
        )
    });

    // The title row also says what the run earned: "+31 xp · level 7 · 62%",
    // or a level-up. Unsaved runs (zen, under a second) earn nothing.
    let mut title = vec![Span::styled(
        "  ✓ session complete",
        Style::default()
            .fg(theme.correct)
            .add_modifier(Modifier::BOLD),
    )];
    if app.session_just_saved {
        let (before, after) = (
            crate::progress::Level::of(comparison.xp_before),
            crate::progress::Level::of(comparison.xp_after),
        );
        let dim = Style::default().fg(theme.pending);
        title.push(Span::styled(
            format!("   +{} xp", comparison.xp_after - comparison.xp_before),
            Style::default().fg(theme.accent),
        ));
        if after.level > before.level {
            title.push(Span::styled(
                format!("  ·  ★ level {}!", after.level),
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            title.push(Span::styled(
                format!(
                    "  ·  level {}  ·  {:.0}%",
                    after.level,
                    after.ratio() * 100.0
                ),
                dim,
            ));
        }
    }
    f.render_widget(Paragraph::new(Line::from(title)), layout[0]);

    let wpm = app.wpm();
    let acc = app.accuracy();
    let elapsed = app.elapsed().as_secs_f64();

    // Delta vs the previous any-mode session — or, for a replay, vs the run
    // it replays.
    let (reference, against) = match &app.replay {
        Some(replay) => (Some(replay.wpm), "original"),
        None => (comparison.last_wpm, "last"),
    };
    let delta = match reference {
        Some(prev) => {
            let diff = wpm - prev;
            let sign = if diff >= 0.0 { "+" } else { "" };
            let color = if diff >= 0.0 {
                theme.correct
            } else {
                theme.incorrect
            };
            Span::styled(
                format!("  ({sign}{:.0} vs {against})", diff),
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
            label(app, "mode best"),
            Span::styled("— (zen not saved)", Style::default().fg(theme.pending)),
        ])
    } else {
        // Best including this session if it was recorded. No recorded session
        // for this mode yet (e.g. this one was too short to save) → `—`.
        let mode_pb_now = match (app.session_just_saved, prev_mode_pb) {
            (true, previous) => Some(previous.map_or(wpm, |best| best.max(wpm))),
            (false, previous) => previous,
        };
        let pb_value = Span::styled(
            mode_pb_now.map_or("— wpm".to_string(), |pb| format!("{pb:.1} wpm")),
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
        // The daily challenge's best is today's: other days had other words.
        let name = if app.best_text().is_some() {
            "today best"
        } else {
            "mode best"
        };
        Line::from(vec![label(app, name), pb_value, badge])
    };

    let body_lines = vec![
        Line::from(vec![
            label(app, "wpm"),
            Span::styled(
                format!("{wpm:.1}"),
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            ),
            delta,
        ]),
        Line::from(vec![
            label(app, "raw wpm"),
            Span::styled(
                format!("{:.1}", app.raw_wpm()),
                Style::default().fg(theme.neutral),
            ),
        ]),
        Line::from(vec![
            label(app, "accuracy"),
            Span::styled(format!("{acc:.1}%"), Style::default().fg(theme.correct)),
        ]),
        Line::from(vec![
            label(app, "consistency"),
            Span::styled(
                app.consistency()
                    .map_or("—".to_string(), |c| format!("{c:.0}%")),
                Style::default().fg(theme.accent),
            ),
        ]),
        Line::from(vec![
            label(app, "time"),
            Span::styled(format!("{elapsed:.1}s"), Style::default().fg(theme.accent)),
        ]),
        // Keys right, keys wrong, and characters of the text never reached.
        Line::from(vec![
            label(app, "chars"),
            Span::styled(
                format!("{} correct", app.correct_chars),
                Style::default().fg(theme.correct),
            ),
            Span::styled("  ·  ", Style::default().fg(theme.pending)),
            Span::styled(
                format!(
                    "{} incorrect",
                    app.total_typed_chars.saturating_sub(app.correct_chars)
                ),
                Style::default().fg(theme.incorrect),
            ),
            Span::styled("  ·  ", Style::default().fg(theme.pending)),
            Span::styled(
                format!("{} missed", app.missed),
                Style::default().fg(theme.neutral),
            ),
        ]),
        Line::from(vec![
            label(app, "mode"),
            Span::styled(&mode_label, Style::default().fg(theme.mode_tag)),
            Span::styled(
                app.replay.as_ref().map_or(String::new(), |replay| {
                    replay.timestamp.format("  of %Y-%m-%d %H:%M").to_string()
                }),
                Style::default().fg(theme.pending),
            ),
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

    render_chart(f, app, layout[2]);

    const HINTS: &[super::Hint] = &[
        ("Enter / F5 restart", key(KeyCode::Enter)),
        ("Esc menu", key(KeyCode::Esc)),
        ("Tab stats", key(KeyCode::Tab)),
        ("F1 help", key(KeyCode::F(1))),
        (
            "Ctrl+C quit",
            Some((KeyCode::Char('c'), KeyModifiers::CONTROL)),
        ),
    ];
    HINTS
}

/// The run second by second: WPM so far (the line that ends at the final
/// WPM), raw speed in each second, and a dot on each second with a mistake.
/// Drawn from `App::timeline`, whole seconds only (a last part-second would
/// read as a spike).
fn render_chart(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    // The legend lives in the title: the chart's own hides itself on a
    // chart this short, and here it never covers a line. Names, not only
    // colors, tell the lines apart.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.pending))
        .title(Line::from(vec![
            Span::raw(" this run, second by second   "),
            Span::styled("━ wpm", Style::default().fg(theme.accent)),
            Span::styled("   ━ raw", Style::default().fg(theme.pending)),
            Span::styled("   • errors ", Style::default().fg(theme.incorrect)),
        ]));
    let seconds = app.elapsed().as_secs() as usize;
    let note = if matches!(app.mode, crate::app::Mode::Zen) {
        Some("zen keeps no score, so no chart")
    } else if seconds < 2 {
        Some("too short to chart (under two seconds)")
    } else {
        None
    };
    if let Some(note) = note {
        let text = Paragraph::new(Line::styled(
            format!("  {note}"),
            Style::default().fg(theme.pending),
        ))
        .block(block);
        return f.render_widget(text, area);
    }

    let [wpm, raw, errors] = chart_points(&app.timeline, seconds);
    let top = wpm.iter().chain(&raw).map(|&(_, y)| y).fold(0.0, f64::max);
    // Room above the highest point, on a round number.
    let top = ((top * 1.1 / 10.0).ceil() * 10.0).max(10.0);
    let label = |text: String| Span::styled(text, Style::default().fg(theme.pending));
    let datasets = vec![
        Dataset::default()
            .name("raw")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.pending))
            .data(&raw),
        Dataset::default()
            .name("wpm")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.accent))
            .data(&wpm),
        Dataset::default()
            .name("errors")
            .marker(Marker::Dot)
            .graph_type(GraphType::Scatter)
            .style(Style::default().fg(theme.incorrect))
            .data(&errors),
    ];
    let chart = Chart::new(datasets)
        .block(block)
        .legend_position(None)
        .x_axis(
            Axis::default()
                .style(Style::default().fg(theme.pending))
                .bounds([1.0, seconds as f64])
                .labels([
                    label("1s".into()),
                    label(format!("{}s", seconds.div_ceil(2))),
                    label(format!("{seconds}s")),
                ]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(theme.pending))
                .bounds([0.0, top])
                .labels([
                    label("0".into()),
                    label(format!("{:.0}", top / 2.0)),
                    label(format!("{top:.0}")),
                ]),
        );
    f.render_widget(chart, area);
}

/// (WPM so far, raw speed, mistakes) as chart points for the first
/// `seconds` whole seconds of `timeline`, x being the second's end (1, 2, …).
/// A second nothing was typed in is 0 raw, and the WPM line carries on.
fn chart_points(timeline: &[crate::app::Second], seconds: usize) -> [Points; 3] {
    let (mut wpm, mut raw, mut errors) = (Vec::new(), Vec::new(), Vec::new());
    let mut correct = 0u64;
    for i in 0..seconds {
        let second = timeline.get(i).copied().unwrap_or_default();
        let x = (i + 1) as f64;
        correct += u64::from(second.correct);
        // Keys per second × 60 / 5 characters a word.
        let speed = f64::from(second.typed) * 12.0;
        wpm.push((x, correct as f64 / 5.0 / (x / 60.0)));
        raw.push((x, speed));
        if second.errors > 0 {
            errors.push((x, speed));
        }
    }
    [wpm, raw, errors]
}

/// (x, y) points of one line on the chart.
type Points = Vec<(f64, f64)>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Second;

    /// The WPM line is the running average (it ends at the run's WPM over
    /// whole seconds), raw is each second's own speed, and mistakes sit on
    /// the raw line of their second; an idle second is 0 raw.
    #[test]
    fn chart_points_per_second() {
        let second = |typed, correct| Second {
            typed,
            correct,
            errors: typed - correct,
        };
        let timeline = [second(5, 5), second(6, 4), Second::default(), second(5, 5)];
        let [wpm, raw, errors] = chart_points(&timeline, 5);
        assert_eq!(
            raw,
            [
                (1.0, 60.0),
                (2.0, 72.0),
                (3.0, 0.0),
                (4.0, 60.0),
                (5.0, 0.0)
            ]
        );
        assert_eq!(errors, [(2.0, 72.0)]);
        assert_eq!(wpm[0], (1.0, 60.0));
        assert_eq!(wpm[1], (2.0, 54.0)); // 9 correct in 2 s
        assert_eq!(wpm[4], (5.0, 33.6)); // 14 correct in 5 s
    }

    fn screen(app: &App) -> String {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| crate::ui::render(f, app)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..24)
            .map(|y| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// At 80×24 the chart fits under the results box with its legend and
    /// axis labels, the footer still on the last row; a run under two
    /// seconds, or zen, says why there is no chart.
    #[test]
    fn chart_is_drawn_under_the_results() {
        use crate::app::{Mode, Screen};
        use std::time::{Duration, Instant};
        let mut app = App::new(None, crate::theme::builtin::DARK, Mode::Time(15));
        app.start_game(Mode::Words(10)).unwrap();
        app.screen = Screen::Results;
        app.stats_cache = Some(Vec::new());
        let end = Instant::now();
        app.ended_at = Some(end);
        app.started_at = Some(end - Duration::from_secs(10));
        app.timeline = (0..10)
            .map(|i| Second {
                typed: 4 + i % 3,
                correct: 4,
                errors: i % 3,
            })
            .collect();
        let shown = screen(&app);
        assert!(shown.contains(" this run, second by second "), "{shown}");
        for text in ["━ wpm   ━ raw   • errors", "1s", "10s"] {
            assert!(shown.contains(text), "{text:?} missing:\n{shown}");
        }
        assert!(
            shown.contains('⣿')
                || shown
                    .chars()
                    .any(|c| ('\u{2800}'..='\u{28ff}').contains(&c))
        );
        assert!(shown
            .lines()
            .last()
            .unwrap()
            .starts_with("  Enter / F5 restart"));

        app.started_at = Some(end - Duration::from_millis(1500));
        assert!(screen(&app).contains("too short to chart"));
        app.mode = Mode::Zen;
        assert!(screen(&app).contains("zen keeps no score"));
    }
}
