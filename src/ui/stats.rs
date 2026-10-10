//! Historical stats screen — reads session data from `App`'s in-memory cache
//! (populated once on screen entry from `~/.typerush/stats.json`). No disk I/O
//! in the render path.
//!
//! v0.3.0 additions:
//!  - Daily streak counter (top summary card)
//!  - Average WPM over the last 7 and 30 days (top summary card)
//!  - Per-key accuracy heatmap (worst 5 keys, min 3 presses each)

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Paragraph, Row, Sparkline, Table},
};

use crossterm::event::KeyCode;

use super::key;
use crate::text::{printable, shorten};
use crate::{app::App, storage};

/// Render the stats history screen.
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let theme = &app.theme;

    // Layout:
    //   0 — title bar (2 rows)
    //   1 — summary (left) + key accuracy (right) (8 rows)
    //   2 — WPM sparkline (5 rows)
    //   3 — recent sessions table (fills remaining space)
    //   4 — footer (1 row — the table gets the rest, 5 sessions at 24 rows)
    let layout = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(8),
        Constraint::Length(5),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .split(area);

    let title = Paragraph::new(Span::styled(
        "  ◆ stats history",
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ));
    f.render_widget(title, layout[0]);

    let sessions: &[storage::SessionRecord] = app.stats_cache.as_deref().unwrap_or(&[]);
    // Normally built once on screen entry (main loop); computed here only if
    // something renders Stats without going through it (e.g. tests).
    let fallback;
    let summary = match &app.stats_summary {
        Some(summary) => summary,
        None => {
            fallback = storage::StatsSummary::new(sessions);
            &fallback
        }
    };

    render_summary_and_heatmap(f, app, layout[1], summary);
    render_sparkline(f, app, layout[2], sessions);
    render_sessions_table(f, app, layout[3], sessions);

    super::render_footer(
        f,
        app,
        layout[4],
        &[
            ("m / esc menu", key(KeyCode::Char('m'))),
            ("q quit", key(KeyCode::Char('q'))),
            ("? help", key(KeyCode::Char('?'))),
        ],
    );
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

/// Render the WPM sparkline (last 20 sessions).
fn render_sparkline(f: &mut Frame, app: &App, area: Rect, sessions: &[storage::SessionRecord]) {
    let theme = &app.theme;
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
                .title(" wpm trend (last 20) "),
        )
        .data(&recent)
        .style(Style::default().fg(theme.accent));
    f.render_widget(spark, area);
}

/// Render the recent sessions table (last 10 sessions, newest first).
/// Width of the recent-sessions Mode column.
const MODE_WIDTH: usize = 18;

fn render_sessions_table(
    f: &mut Frame,
    app: &App,
    area: Rect,
    sessions: &[storage::SessionRecord],
) {
    let theme = &app.theme;

    let recent_rows = sessions
        .iter()
        .rev()
        .take(10)
        .map(|s| {
            Row::new(vec![
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
        .collect::<Vec<_>>();

    let header = Row::new(vec!["Date", "Mode", "WPM", "Acc", "Time"]).style(
        Style::default()
            .fg(theme.pending)
            .add_modifier(Modifier::BOLD),
    );

    let table = Table::new(
        recent_rows,
        [
            Constraint::Length(18),
            Constraint::Length(MODE_WIDTH as u16), // fits "words-100+10k+p+n"
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.pending))
            .title(" recent sessions "),
    );
    f.render_widget(table, area);
}
