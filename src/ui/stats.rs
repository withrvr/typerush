//! Historical stats screen — reads session data from `App`'s in-memory cache
//! (populated once on screen entry from `~/.typerush/stats.json`). No disk I/O
//! in the render path.
//!
//! v0.3.0: daily streak, 7/30-day averages, per-key accuracy heatmap.
//! v0.4.1: categories — ←/→ shows every session, a table of each mode's
//! best, or one mode's own sessions, summary, trend and keys. Sessions that
//! hold their mode's best are marked ★; the sessions list scrolls.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Paragraph, Row, Sparkline, Table},
};

use ratatui::crossterm::event::KeyCode;

use super::key;
use crate::text::{printable, shorten};
use crate::{app::App, storage};

/// Render the stats history screen.
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let theme = &app.theme;

    // Layout:
    //   0 — title bar with the category (2 rows)
    //   1 — summary (left) + key accuracy (right) (8 rows)
    //   2 — WPM sparkline (5 rows)
    //   3 — sessions or bests table (fills remaining space)
    //   4 — footer (1 row — the table gets the rest, 5 rows at 24 rows)
    let layout = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(8),
        Constraint::Length(5),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .split(area);

    let sessions: &[storage::SessionRecord] = app.stats_cache.as_deref().unwrap_or(&[]);
    // Normally built on screen entry (main loop); built here only if
    // something renders Stats without going through it (e.g. tests).
    let fallback;
    let view = match &app.stats_view {
        Some(view) => view,
        None => {
            fallback = storage::StatsView::new(sessions, None);
            &fallback
        }
    };

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "  ◆ stats   ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("‹ ", Style::default().fg(theme.pending)),
        Span::styled(
            shorten(&printable(view.category_name()), 32).into_owned(),
            Style::default()
                .fg(theme.mode_tag)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ›", Style::default().fg(theme.pending)),
        Span::styled(
            format!("   {}/{}", view.category + 1, view.category_count()),
            Style::default().fg(theme.pending),
        ),
    ]));
    f.render_widget(title, layout[0]);

    render_summary_and_heatmap(f, app, layout[1], &view.summary);
    render_sparkline(f, app, layout[2], sessions, view);
    if view.showing_bests() {
        render_bests_table(f, app, layout[3], sessions, view);
    } else {
        render_sessions_table(f, app, layout[3], sessions, view);
    }

    super::render_footer(
        f,
        app,
        layout[4],
        &[
            ("←/→ category", key(KeyCode::Right)),
            ("↑/↓ scroll", key(KeyCode::Down)),
            ("m / esc menu", key(KeyCode::Char('m'))),
            ("q quit", key(KeyCode::Char('q'))),
            ("? help", key(KeyCode::Char('?'))),
        ],
    );
}

/// `1h 05m`, `12m`, `45s`: time spent typing, at a glance.
fn format_duration(secs: f64) -> String {
    let secs = secs as u64;
    match (secs / 3600, secs / 60 % 60) {
        (0, 0) => format!("{secs}s"),
        (0, minutes) => format!("{minutes}m"),
        (hours, minutes) => format!("{hours}h {minutes:02}m"),
    }
}

/// Renders the top row: summary card on the left, key-accuracy heatmap on the right.
fn render_summary_and_heatmap(
    f: &mut Frame,
    app: &App,
    area: Rect,
    summary: &storage::StatsSummary,
) {
    let theme = &app.theme;

    let cols =
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).split(area);

    // ── Summary card ─────────────────────────────────────────────────────────

    let pb = summary.personal_best.unwrap_or(0.0);
    let avg_acc = summary.average_accuracy.unwrap_or(0.0);
    let total = summary.sessions;
    let last_wpm = summary.last_wpm.unwrap_or(0.0);

    let streak = summary.streak;
    let streak_text = match streak {
        0 => "—".to_string(),
        1 => "1 day".to_string(),
        n => format!("{} days", n),
    };

    let avg7 = summary.avg_wpm_7_days;
    let avg30 = summary.avg_wpm_30_days;

    let fmt_avg = |v: Option<f64>| match v {
        None => "  —".to_string(),
        Some(x) => format!("{:>5.1}", x),
    };

    let summary_lines = vec![
        Line::from(vec![
            Span::styled("  best wpm     ", Style::default().fg(theme.pending)),
            Span::styled(
                format!("{:>6.1}", pb),
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("avg acc  ", Style::default().fg(theme.pending)),
            Span::styled(
                format!("{:>5.1}%", avg_acc),
                Style::default().fg(theme.correct),
            ),
        ]),
        Line::from(vec![
            Span::styled("  sessions     ", Style::default().fg(theme.pending)),
            Span::styled(format!("{:>6}", total), Style::default().fg(theme.neutral)),
            Span::raw("   "),
            Span::styled("last wpm ", Style::default().fg(theme.pending)),
            Span::styled(
                format!("{:>5.1}", last_wpm),
                Style::default().fg(theme.accent),
            ),
        ]),
        Line::from(vec![
            Span::styled("  streak       ", Style::default().fg(theme.pending)),
            Span::styled(
                format!("{:>9}", streak_text),
                Style::default()
                    .fg(if streak > 0 {
                        theme.secondary
                    } else {
                        theme.neutral
                    })
                    .add_modifier(if streak > 0 {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        ]),
        Line::from(vec![
            Span::styled("  7-day avg    ", Style::default().fg(theme.pending)),
            Span::styled(
                format!("{} wpm", fmt_avg(avg7)),
                Style::default().fg(theme.accent),
            ),
        ]),
        Line::from(vec![
            Span::styled("  30-day avg   ", Style::default().fg(theme.pending)),
            Span::styled(
                format!("{} wpm", fmt_avg(avg30)),
                Style::default().fg(theme.accent),
            ),
        ]),
        Line::from(vec![
            Span::styled("  time typed   ", Style::default().fg(theme.pending)),
            Span::styled(
                format!("{:>6}", format_duration(summary.time_typed_secs)),
                Style::default().fg(theme.neutral),
            ),
        ]),
    ];
    let summary_card = Paragraph::new(summary_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.pending))
            .title(" summary "),
    );
    f.render_widget(summary_card, cols[0]);

    // ── Key accuracy heatmap ─────────────────────────────────────────────────

    render_key_heatmap(f, app, cols[1], &summary.worst_keys);
}

/// Render the key-accuracy panel (worst keys, sorted by accuracy ascending).
fn render_key_heatmap(
    f: &mut Frame,
    app: &App,
    area: Rect,
    worst_keys: &[storage::KeyAccuracyStat],
) {
    let theme = &app.theme;

    if worst_keys.is_empty() {
        let msg = Paragraph::new(vec![
            Line::raw(""),
            Line::from(Span::styled(
                "  no key data yet",
                Style::default().fg(theme.pending),
            )),
            Line::from(Span::styled(
                "  (keep typing!)",
                Style::default().fg(theme.pending),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.pending))
                .title(" key accuracy "),
        );
        f.render_widget(msg, area);
        return;
    }

    // Show up to 5 worst keys in a mini-table.
    let rows: Vec<Row> = worst_keys
        .iter()
        .take(5)
        .map(|stat| {
            let acc_color = if stat.accuracy < 80.0 {
                theme.incorrect
            } else if stat.accuracy < 93.0 {
                theme.secondary
            } else {
                theme.correct
            };
            Row::new(vec![
                Cell::from(format!("  {:?}", stat.key)).style(Style::default().fg(theme.neutral)),
                // "hits/total" format uses both KeyAccuracyStat::hits and ::total.
                Cell::from(format!("{}/{}", stat.hits, stat.total))
                    .style(Style::default().fg(theme.neutral)),
                Cell::from(format!("{:>5.1}%", stat.accuracy))
                    .style(Style::default().fg(acc_color)),
            ])
        })
        .collect();

    let header = Row::new(vec!["  key", "ok/total", " acc"]).style(
        Style::default()
            .fg(theme.pending)
            .add_modifier(Modifier::BOLD),
    );

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(9),
            Constraint::Length(7),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.pending))
            .title(" key accuracy "),
    );
    f.render_widget(table, area);
}

/// Render the WPM sparkline: the category's last 20 sessions.
fn render_sparkline(
    f: &mut Frame,
    app: &App,
    area: Rect,
    sessions: &[storage::SessionRecord],
    view: &storage::StatsView,
) {
    let theme = &app.theme;
    let recent: Vec<u64> = view.indices[view.indices.len().saturating_sub(20)..]
        .iter()
        .map(|&i| sessions[i].wpm.max(0.0) as u64)
        .collect();
    let spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.pending))
                .title(" wpm trend (last 20) "),
        )
        .data(&recent)
        .style(Style::default().fg(theme.accent));
    f.render_widget(spark, area);
}

/// Width of the Mode column in both tables.
const MODE_WIDTH: usize = 18;

/// Rows of a bordered table with a header that fit in `area`, and the first
/// of `total` rows to show: the view's scroll, clamped so the last page is
/// full (and written back, so scrolling back up responds at once).
fn visible_rows(area: Rect, total: usize, view: &storage::StatsView) -> (usize, usize) {
    let fits = area.height.saturating_sub(3) as usize;
    let first = view.scroll.get().min(total.saturating_sub(fits));
    view.scroll.set(first);
    (first, fits)
}

/// A bordered table block titled with the rows shown: ` sessions · 1–5 of 42 `.
fn table_block<'a>(app: &App, name: &str, first: usize, shown: usize, total: usize) -> Block<'a> {
    let title = if total == 0 {
        format!(" {name} · none yet ")
    } else {
        format!(" {name} · {}–{} of {total} ", first + 1, first + shown)
    };
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.pending))
        .title(title)
}

/// The category's sessions, newest first, ★ on each mode's best.
fn render_sessions_table(
    f: &mut Frame,
    app: &App,
    area: Rect,
    sessions: &[storage::SessionRecord],
    view: &storage::StatsView,
) {
    let theme = &app.theme;
    let total = view.indices.len();
    let (first, fits) = visible_rows(area, total, view);

    let rows: Vec<Row> = view
        .indices
        .iter()
        .rev()
        .skip(first)
        .take(fits)
        .map(|&i| {
            let s = &sessions[i];
            let star = if view.best_indices.contains(&i) {
                "★"
            } else {
                ""
            };
            Row::new(vec![
                Cell::from(star).style(
                    Style::default()
                        .fg(theme.secondary)
                        .add_modifier(Modifier::BOLD),
                ),
                // Date column: neutral so it renders cleanly on all themes.
                Cell::from(s.timestamp.format("%Y-%m-%d %H:%M").to_string())
                    .style(Style::default().fg(theme.neutral)),
                // stats.json can be hand-edited, and custom labels carry file
                // names: made safe, and long ones shortened to the column.
                Cell::from(shorten(&printable(&s.mode), MODE_WIDTH).into_owned())
                    .style(Style::default().fg(theme.mode_tag)),
                Cell::from(format!("{:.1}", s.wpm)).style(Style::default().fg(theme.secondary)),
                Cell::from(format!("{:.1}%", s.accuracy)).style(Style::default().fg(theme.correct)),
                Cell::from(format!("{:.1}s", s.duration_secs))
                    .style(Style::default().fg(theme.accent)),
            ])
        })
        .collect();
    let shown = rows.len();

    let header = Row::new(vec!["", "Date", "Mode", "WPM", "Acc", "Time"]).style(
        Style::default()
            .fg(theme.pending)
            .add_modifier(Modifier::BOLD),
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(1),
            Constraint::Length(16),
            Constraint::Length(MODE_WIDTH as u16), // fits "words-100+10k+p+n"
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(table_block(app, "sessions", first, shown, total));
    f.render_widget(table, area);
}

/// One row per mode: its best (with that run's accuracy and date), average
/// and number of runs.
fn render_bests_table(
    f: &mut Frame,
    app: &App,
    area: Rect,
    sessions: &[storage::SessionRecord],
    view: &storage::StatsView,
) {
    let theme = &app.theme;
    let total = view.bests.len();
    let (first, fits) = visible_rows(area, total, view);

    let rows: Vec<Row> = view
        .bests
        .iter()
        .skip(first)
        .take(fits)
        .map(|best| {
            let record = &sessions[best.best_index];
            Row::new(vec![
                Cell::from(shorten(&printable(&best.mode), MODE_WIDTH).into_owned())
                    .style(Style::default().fg(theme.mode_tag)),
                Cell::from(format!("★ {:.1}", record.wpm)).style(
                    Style::default()
                        .fg(theme.secondary)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(format!("{:.1}%", record.accuracy))
                    .style(Style::default().fg(theme.correct)),
                Cell::from(format!("{:.1}", best.avg_wpm)).style(Style::default().fg(theme.accent)),
                Cell::from(best.sessions.to_string()).style(Style::default().fg(theme.neutral)),
                Cell::from(record.timestamp.format("%Y-%m-%d").to_string())
                    .style(Style::default().fg(theme.neutral)),
            ])
        })
        .collect();
    let shown = rows.len();

    let header = Row::new(vec!["Mode", "Best", "Acc", "Avg", "Runs", "Set on"]).style(
        Style::default()
            .fg(theme.pending)
            .add_modifier(Modifier::BOLD),
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(MODE_WIDTH as u16),
            Constraint::Length(9),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(5),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(table_block(app, "bests per mode", first, shown, total));
    f.render_widget(table, area);
}
