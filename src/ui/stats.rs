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
    widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Sparkline, Table},
};

use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use unicode_width::UnicodeWidthStr;

use super::{key, label};
use crate::text::{printable, shorten};
use crate::{
    app::{App, ClickAction},
    storage,
};

/// Render the stats history screen.
/// Returns the footer hints.
pub fn render(f: &mut Frame, app: &App, area: Rect) -> &'static [super::Hint] {
    let theme = &app.theme;

    // Layout:
    //   0 — title bar with the category (2 rows)
    //   1 — summary (left) + key accuracy (right) (8 rows)
    //   2 — WPM sparkline (5 rows)
    //   3 — sessions or bests table (fills remaining space)
    let layout = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(8),
        Constraint::Length(5),
        Constraint::Min(4),
    ])
    .split(area);

    let sessions: &[storage::SessionRecord] = app.stats_cache.as_deref().unwrap_or(&[]);
    // Normally built on screen entry (main loop); built here only if
    // something renders Stats without going through it (e.g. tests).
    let fallback;
    let view = match &app.stats_view {
        Some(view) => view,
        None => {
            fallback = storage::StatsView::new(sessions, None, Default::default());
            &fallback
        }
    };

    // `◆ stats   [all]   ‹ ›   01/05   time-30s`: the buttons and counter
    // come before the category name, so they never move when it changes,
    // and the counter is zero-padded to a fixed width. [all], ‹ and › are
    // clickable: all sessions, previous / next category.
    let total = view.category_count();
    let digits = total.to_string().len().max(2);
    let counter = format!("{:0digits$}/{total:0digits$}", view.category + 1);
    let button = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let current = Style::default()
        .fg(theme.mode_tag)
        .add_modifier(Modifier::BOLD);
    let category = shorten(&printable(view.category_name()), 32).into_owned();
    let parts: [(String, Style, Option<KeyCode>); 10] = [
        ("  ◆ stats".into(), button, None),
        ("   ".into(), Style::default(), None),
        (
            "[all]".into(),
            if view.category == 0 { current } else { button },
            Some(KeyCode::Char('a')),
        ),
        ("   ".into(), Style::default(), None),
        ("‹".into(), button, Some(KeyCode::Left)),
        (" ".into(), Style::default(), None),
        ("›".into(), button, Some(KeyCode::Right)),
        ("   ".into(), Style::default(), None),
        (counter, Style::default().fg(theme.pending), None),
        (format!("   {category}"), current, None),
    ];
    let mut spans = Vec::with_capacity(parts.len());
    let mut x = layout[0].x;
    let mut targets = app.click_targets.borrow_mut();
    for (text, style, action) in parts {
        let width = text.width() as u16;
        if let Some(code) = action {
            let cell = Rect::new(x, layout[0].y, width, 1).intersection(layout[0]);
            targets.push((cell, ClickAction::Key(code, KeyModifiers::NONE)));
        }
        x += width;
        spans.push(Span::styled(text, style));
    }
    drop(targets);
    let title = Paragraph::new(Line::from(spans));
    f.render_widget(title, layout[0]);

    render_summary_and_heatmap(f, app, layout[1], &view.summary);
    render_sparkline(f, app, layout[2], sessions, view);
    if view.showing_bests() {
        render_bests_table(f, app, layout[3], sessions, view);
    } else {
        render_sessions_table(f, app, layout[3], sessions, view);
    }

    // What fits in 80 columns. ↑/↓, `a` and `q` still work (help lists
    // them); the rows, ‹ › and [all] are clickable.
    const HINTS: &[super::Hint] = &[
        // Two-way navigation hints aren't clickable: ‹ › in the title, the
        // rows themselves and the mouse wheel do it with the mouse.
        ("←/→ category", None),
        ("Enter replay", key(KeyCode::Enter)),
        ("e export", key(KeyCode::Char('e'))),
        ("Esc menu", key(KeyCode::Esc)),
        ("? help", key(KeyCode::Char('?'))),
    ];
    HINTS
}

/// `1h 05m`, `12m`, `45s`: time spent typing, at a glance.
pub(super) fn format_duration(secs: f64) -> String {
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
        None => "—".to_string(),
        Some(x) => format!("{x:.1}"),
    };

    // Two columns on the first rows, one below: every value in the left
    // column starts at the shared label width, and the right column's values
    // line up with each other too.
    // Width of a left-column value and of a right-column label: the right
    // column starts LABEL_WIDTH + VALUE_WIDTH cells in, and its values line
    // up VALUE_WIDTH after that.
    const VALUE_WIDTH: usize = 10;
    let value = |text: String, style: Style| Span::styled(format!("{text:<VALUE_WIDTH$}"), style);
    let right_label = |text: &str| {
        Span::styled(
            format!("{text:<VALUE_WIDTH$}"),
            Style::default().fg(theme.pending),
        )
    };
    let streak_style = if streak > 0 {
        Style::default()
            .fg(theme.secondary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.neutral)
    };
    let summary_lines = vec![
        Line::from(vec![
            label(app, "best wpm"),
            value(
                format!("{pb:.1}"),
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            ),
            right_label("avg acc"),
            Span::styled(format!("{avg_acc:.1}%"), Style::default().fg(theme.correct)),
        ]),
        Line::from(vec![
            label(app, "sessions"),
            value(total.to_string(), Style::default().fg(theme.neutral)),
            right_label("last wpm"),
            Span::styled(format!("{last_wpm:.1}"), Style::default().fg(theme.accent)),
        ]),
        Line::from(vec![
            label(app, "streak"),
            Span::styled(streak_text, streak_style),
        ]),
        Line::from(vec![
            label(app, "7-day avg"),
            Span::styled(
                format!("{} wpm", fmt_avg(avg7)),
                Style::default().fg(theme.accent),
            ),
        ]),
        Line::from(vec![
            label(app, "30-day avg"),
            Span::styled(
                format!("{} wpm", fmt_avg(avg30)),
                Style::default().fg(theme.accent),
            ),
        ]),
        Line::from(vec![
            label(app, "time typed"),
            Span::styled(
                format_duration(summary.time_typed_secs),
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

/// Render the WPM sparkline: as many of the category's latest sessions as
/// the box has room for — one bar per cell — titled with how many show.
fn render_sparkline(
    f: &mut Frame,
    app: &App,
    area: Rect,
    sessions: &[storage::SessionRecord],
    view: &storage::StatsView,
) {
    let theme = &app.theme;
    // Two cells of padding, as text has in every box.
    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(2))
        .border_style(Style::default().fg(theme.pending));
    let room = block.inner(area).width as usize;
    let shown = &view.indices[view.indices.len().saturating_sub(room)..];
    let recent: Vec<u64> = shown
        .iter()
        .map(|&i| sessions[i].wpm.max(0.0) as u64)
        .collect();
    let title = match shown.len() {
        0 => " wpm trend ".to_string(),
        1 => " wpm trend (1 session) ".to_string(),
        n if n == view.indices.len() => format!(" wpm trend (all {n}) "),
        n => format!(" wpm trend (last {n}) "),
    };
    let spark = Sparkline::default()
        .block(block.title(title))
        .data(&recent)
        .style(Style::default().fg(theme.accent));
    f.render_widget(spark, area);
}

/// Width of the Mode column in both tables: the sessions table's columns
/// then fill 80 columns exactly, padding included.
const MODE_WIDTH: usize = 17;

/// Rows of a bordered table with a header that fit in `area`, and the first
/// of `total` rows to show: the view's scroll, moved just enough to keep the
/// selected row in view and clamped so the last page is full (written back,
/// so the next frame starts from it).
fn visible_rows(area: Rect, total: usize, view: &storage::StatsView) -> (usize, usize) {
    let fits = area.height.saturating_sub(3) as usize;
    if fits == 0 {
        // Too short to show a row (a tiny window): keep the scroll as it is.
        return (view.scroll.get(), 0);
    }
    let first = view
        .scroll
        .get()
        .min(view.selected)
        .max((view.selected + 1).saturating_sub(fits))
        .min(total.saturating_sub(fits));
    view.scroll.set(first);
    (first, fits)
}

/// Make each drawn row of a table in `area` clickable: a click selects it.
/// Rows start under the top border and the header.
fn register_rows(app: &App, area: Rect, first: usize, shown: usize) {
    let inner = area.inner(Margin::new(1 + TABLE_PADDING, 1));
    let mut targets = app.click_targets.borrow_mut();
    for row in 0..shown {
        let y = inner.y + 1 + row as u16;
        let cell = Rect::new(inner.x, y, inner.width, 1).intersection(inner);
        if !cell.is_empty() {
            targets.push((cell, ClickAction::Select(first + row)));
        }
    }
}

/// `style`, or the highlight when the row is the selected one: one style
/// for the whole row, so it reads as a bar.
fn row_style(app: &App, selected: bool, style: Style) -> Style {
    if selected {
        super::selected_style(&app.theme)
    } else {
        style
    }
}

/// A bordered table block titled with the rows shown: ` sessions · 1–5 of 42 `,
/// the filter chips on its bottom border.
fn table_block<'a>(
    app: &App,
    view: &storage::StatsView,
    name: &str,
    (first, shown, total): (usize, usize, usize),
    chips: Line<'a>,
) -> Block<'a> {
    let title = if total == 0 && view.filters.filtering() {
        format!(" {name} · none match ")
    } else if total == 0 {
        format!(" {name} · none yet ")
    } else if shown == 0 {
        format!(" {name} · {total} ")
    } else {
        format!(" {name} · {}–{} of {total} ", first + 1, first + shown)
    };
    // Two cells of padding, as text has in every box: the ★ column keeps
    // off the border.
    Block::default()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(TABLE_PADDING))
        .border_style(Style::default().fg(app.theme.pending))
        .title(title)
        .title_bottom(chips)
}

/// Cells between a table's border and its first column.
const TABLE_PADDING: u16 = 2;

/// The filter chips on a table's bottom border — `d dates all time`,
/// `w words any`, and on the sessions list `s sort date ↓` and `r reverse` —
/// each clickable like its key. A chip that filters something out is
/// highlighted (and says what it keeps, so it isn't color alone).
fn filter_chips(
    app: &App,
    view: &storage::StatsView,
    area: Rect,
    with_sort: bool,
) -> Line<'static> {
    let theme = &app.theme;
    let filters = view.filters;
    let key_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let value = |active: bool| {
        if active {
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.pending)
        }
    };
    let arrow = if filters.ascending { "↑" } else { "↓" };
    let sort = format!("{} {arrow}", filters.sort.label());
    let mut chips = vec![
        (
            'd',
            "dates ",
            filters.range.label().to_string(),
            filters.range != Default::default(),
        ),
        (
            'w',
            "words ",
            filters.words.label().to_string(),
            filters.words != Default::default(),
        ),
    ];
    if with_sort {
        let sorted = filters.sort != Default::default() || filters.ascending;
        chips.push(('s', "sort ", sort, sorted));
        chips.push(('r', "reverse", String::new(), false));
    }
    let mut spans = vec![Span::raw(" ")];
    let mut x = area.x + 2;
    let border_y = area.bottom().saturating_sub(1);
    let mut targets = app.click_targets.borrow_mut();
    for (code, name, current, active) in chips {
        let chip = [
            Span::styled(format!("{code} "), key_style),
            Span::styled(name, Style::default().fg(theme.pending)),
            Span::styled(current, value(active)),
        ];
        let width: u16 = chip.iter().map(|span| span.width() as u16).sum();
        if area.height >= 2 {
            let cell = Rect::new(x, border_y, width, 1).intersection(area);
            if !cell.is_empty() {
                targets.push((
                    cell,
                    ClickAction::Key(KeyCode::Char(code), KeyModifiers::NONE),
                ));
            }
        }
        spans.extend(chip);
        spans.push(Span::raw("   "));
        x += width + 3;
    }
    Line::from(spans)
}

/// `61.0`, or `—` when there's no figure.
fn figure(value: Option<f64>, suffix: &str) -> String {
    value.map_or("—".to_string(), |v| format!("{v:.1}{suffix}"))
}

/// The category's sessions that pass the filters, in the chosen order
/// (newest first by default), ★ on each mode's best.
fn render_sessions_table(
    f: &mut Frame,
    app: &App,
    area: Rect,
    sessions: &[storage::SessionRecord],
    view: &storage::StatsView,
) {
    let theme = &app.theme;
    let total = view.order.len();
    let (first, fits) = visible_rows(area, total, view);

    let rows: Vec<Row> = view
        .order
        .iter()
        .enumerate()
        .skip(first)
        .take(fits)
        .map(|(row, &i)| {
            let s = &sessions[i];
            let star = if view.best_indices.contains(&i) {
                "★"
            } else {
                ""
            };
            let style = |style| row_style(app, row == view.selected, style);
            Row::new(vec![
                Cell::from(star).style(style(
                    Style::default()
                        .fg(theme.secondary)
                        .add_modifier(Modifier::BOLD),
                )),
                // Date column: neutral so it renders cleanly on all themes.
                Cell::from(s.timestamp.format("%Y-%m-%d %H:%M").to_string())
                    .style(style(Style::default().fg(theme.neutral))),
                // stats.json can be hand-edited, and custom labels carry file
                // names: made safe, and long ones shortened to the column.
                Cell::from(shorten(&printable(&s.mode), MODE_WIDTH).into_owned())
                    .style(style(Style::default().fg(theme.mode_tag))),
                Cell::from(format!("{:.1}", s.wpm))
                    .style(style(Style::default().fg(theme.secondary))),
                Cell::from(figure(s.raw_wpm(), ""))
                    .style(style(Style::default().fg(theme.neutral))),
                Cell::from(format!("{:.1}%", s.accuracy))
                    .style(style(Style::default().fg(theme.correct))),
                // Consistency as a whole percentage, `—` before v0.6.
                Cell::from(
                    s.consistency
                        .map_or("—".to_string(), |c| format!("{c:.0}%")),
                )
                .style(style(Style::default().fg(theme.accent))),
                Cell::from(format!("{:.1}s", s.duration_secs))
                    .style(style(Style::default().fg(theme.accent))),
            ])
            .style(style(Style::default()))
        })
        .collect();
    let shown = rows.len();
    register_rows(app, area, first, shown);

    let header = Row::new(vec![
        "", "Date", "Mode", "WPM", "Raw", "Acc", "Cons", "Time",
    ])
    .style(
        Style::default()
            .fg(theme.pending)
            .add_modifier(Modifier::BOLD),
    );
    let chips = filter_chips(app, view, area, true);
    let table = Table::new(
        rows,
        [
            Constraint::Length(1),
            Constraint::Length(16),
            Constraint::Length(MODE_WIDTH as u16), // fits "words-100+10k+p+n" (17)
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(5),
            Constraint::Length(7),
        ],
    )
    .header(header)
    .block(table_block(
        app,
        view,
        "sessions",
        (first, shown, total),
        chips,
    ));
    f.render_widget(table, area);
}

/// One row per mode: its best among the sessions that pass the filters
/// (with that run's accuracy and date), average and number of runs.
fn render_bests_table(
    f: &mut Frame,
    app: &App,
    area: Rect,
    sessions: &[storage::SessionRecord],
    view: &storage::StatsView,
) {
    let theme = &app.theme;
    let total = view.table_bests.len();
    let (first, fits) = visible_rows(area, total, view);

    let rows: Vec<Row> = view
        .table_bests
        .iter()
        .enumerate()
        .skip(first)
        .take(fits)
        .map(|(row, best)| {
            let record = &sessions[best.best_index];
            let style = |style| row_style(app, row == view.selected, style);
            Row::new(vec![
                // Empty gutter, like the sessions table's ★ column, so the
                // first column starts where it does in every other box.
                Cell::from(""),
                Cell::from(shorten(&printable(&best.mode), MODE_WIDTH).into_owned())
                    .style(style(Style::default().fg(theme.mode_tag))),
                Cell::from(format!("★ {:.1}", record.wpm)).style(style(
                    Style::default()
                        .fg(theme.secondary)
                        .add_modifier(Modifier::BOLD),
                )),
                Cell::from(format!("{:.1}%", record.accuracy))
                    .style(style(Style::default().fg(theme.correct))),
                Cell::from(format!("{:.1}", best.avg_wpm))
                    .style(style(Style::default().fg(theme.accent))),
                Cell::from(best.sessions.to_string())
                    .style(style(Style::default().fg(theme.neutral))),
                Cell::from(record.timestamp.format("%Y-%m-%d").to_string())
                    .style(style(Style::default().fg(theme.neutral))),
            ])
            .style(style(Style::default()))
        })
        .collect();
    let shown = rows.len();
    register_rows(app, area, first, shown);

    let header = Row::new(vec!["", "Mode", "Best", "Acc", "Avg", "Runs", "Set on"]).style(
        Style::default()
            .fg(theme.pending)
            .add_modifier(Modifier::BOLD),
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(1),
            Constraint::Length(MODE_WIDTH as u16),
            Constraint::Length(9),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(5),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(table_block(
        app,
        view,
        "bests per mode",
        (first, shown, total),
        filter_chips(app, view, area, false),
    ));
    f.render_widget(table, area);
}
