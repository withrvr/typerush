//! Profile screen — your level, what you have typed so far, the best for
//! every standard time and word count side by side, and (below) the
//! activity calendar. Reads only `App::profile`, added up when the screen
//! opens; never the disk.

use ratatui::crossterm::event::KeyCode;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use super::{key, label};
use crate::app::App;
use crate::progress::{Profile, StandardBest, STANDARD_TIMES, STANDARD_WORDS};
use crate::text::{printable, shorten};

/// Render the Profile screen. Returns the footer hints.
pub fn render(f: &mut Frame, app: &App, area: Rect) -> &'static [super::Hint] {
    let theme = &app.theme;
    let [title_area, top, calendar] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(11),
        Constraint::Min(0),
    ])
    .areas(area);
    f.render_widget(
        Paragraph::new(Span::styled(
            "  ◆ profile",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        title_area,
    );

    // Normally added up on screen entry (main loop); an empty profile only
    // if something draws it without going through it (tests).
    let fallback;
    let profile = match &app.profile {
        Some(profile) => profile,
        None => {
            fallback = Profile::default();
            &fallback
        }
    };
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(top);
    render_summary(f, app, left, profile);
    render_bests(f, app, right, profile);
    render_calendar(f, app, calendar, profile);

    const HINTS: &[super::Hint] = &[
        // The days themselves are clickable.
        ("←/→ week", None),
        ("↑/↓ day", None),
        ("Esc menu", key(KeyCode::Esc)),
        ("? help", key(KeyCode::Char('?'))),
        ("q quit", key(KeyCode::Char('q'))),
    ];
    HINTS
}

/// Cells left of the calendar's first week: the box's two-cell inset and
/// the day names ("Mon ").
const DAY_GUTTER: u16 = 6;

/// How much a day was typed, 0–4, against the busiest day shown: 0 is
/// none, 4 the busiest quarter.
fn shade(secs: f64, busiest: f64) -> usize {
    if secs <= 0.0 || busiest <= 0.0 {
        return 0;
    }
    ((secs / busiest * 4.0).ceil() as usize).clamp(1, 4)
}

/// The activity calendar: a column per week (Monday on top), the current
/// week last, as many weeks as fit up to a year; each day shaded by how
/// long you typed, in glyphs that get denser (`· ░ ▒ ▓ █`, not color
/// alone). The picked day is highlighted and described on the bottom
/// border; a click picks a day.
fn render_calendar(f: &mut Frame, app: &App, area: Rect, profile: &Profile) {
    use chrono::{Datelike, Days};
    if area.height < 4 {
        return;
    }
    let theme = &app.theme;
    let today = chrono::Local::now().date_naive();
    let inner_width = area.width.saturating_sub(2);
    let weeks = u64::from(inner_width.saturating_sub(DAY_GUTTER)).clamp(1, 53);
    let this_monday = today - Days::new(u64::from(today.weekday().num_days_from_monday()));
    let first_day = this_monday - Days::new(7 * (weeks - 1));
    let shown = profile.days.range(first_day..=today);
    let busiest = shown.clone().map(|(_, day)| day.secs).fold(0.0, f64::max);
    let typed_days = shown.filter(|(_, day)| day.runs > 0).count();

    // Month names over the week a month starts in, when there's room.
    let mut months = " ".repeat(DAY_GUTTER as usize);
    for week in 0..weeks {
        let monday = first_day + Days::new(7 * week);
        let starts_month = week == 0 || (monday - Days::new(7)).month() != monday.month();
        let column = DAY_GUTTER as usize + week as usize;
        // Only if the last name (and its space) ends before this column.
        if starts_month && months.chars().count() <= column {
            let name = monday.format("%b").to_string();
            months.push_str(&" ".repeat(column - months.chars().count()));
            months.push_str(&name);
            months.push(' ');
        }
    }
    let mut lines = vec![Line::styled(months, Style::default().fg(theme.pending))];

    let glyphs = ["·", "░", "▒", "▓", "█"];
    let inner = Block::default().borders(Borders::ALL).inner(area);
    let mut targets = app.click_targets.borrow_mut();
    for weekday in 0..7u64 {
        let name = match weekday {
            0 => "Mon ",
            2 => "Wed ",
            4 => "Fri ",
            _ => "    ",
        };
        let mut spans = vec![Span::styled(
            format!("  {name}"),
            Style::default().fg(theme.pending),
        )];
        for week in 0..weeks {
            let day = first_day + Days::new(7 * week + weekday);
            if day > today {
                spans.push(Span::raw(" "));
                continue;
            }
            let level = profile.days.get(&day).map_or(0, |d| shade(d.secs, busiest));
            let style = if day == profile.selected_day {
                super::selected_style(theme)
            } else if level == 0 {
                Style::default().fg(theme.pending)
            } else {
                Style::default().fg(theme.accent)
            };
            spans.push(Span::styled(glyphs[level], style));
            let cell = Rect::new(
                inner.x + DAY_GUTTER + week as u16,
                inner.y + 1 + weekday as u16,
                1,
                1,
            )
            .intersection(inner);
            if !cell.is_empty() {
                let back = (today - day).num_days() as usize;
                targets.push((cell, crate::app::ClickAction::Select(back)));
            }
        }
        lines.push(Line::from(spans));
    }
    drop(targets);

    let picked = profile.selected_day;
    let details = match profile.days.get(&picked) {
        Some(day) => format!(
            " {} · {} run{} · {} · best {:.1} ",
            picked.format("%a %Y-%m-%d"),
            day.runs,
            if day.runs == 1 { "" } else { "s" },
            super::stats::format_duration(day.secs),
            day.best_wpm
        ),
        None => format!(" {} · no typing ", picked.format("%a %Y-%m-%d")),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.pending))
        .title(format!(
            " activity · {typed_days} day{} typed ",
            if typed_days == 1 { "" } else { "s" }
        ))
        .title_bottom(Span::styled(details, Style::default().fg(theme.neutral)))
        .title_bottom(
            Line::styled(" less · ░ ▒ ▓ █ more ", Style::default().fg(theme.pending))
                .right_aligned(),
        );
    f.render_widget(Paragraph::new(lines).block(block), area);
}

/// `81,234`: a count with thousands separated.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// The left box: level and progress, then everything typed so far.
fn render_summary(f: &mut Frame, app: &App, area: Rect, profile: &Profile) {
    let theme = &app.theme;
    let value = |text: String, color: Color| Span::styled(text, Style::default().fg(color));
    let level = profile.level;
    // A ten-cell bar (filled and empty in different glyphs, not color
    // alone) and the percentage.
    let filled = ((level.ratio() * 10.0) as usize).min(10);
    let days = |n: u32| {
        if n == 1 {
            "1 day".to_string()
        } else {
            format!("{n} days")
        }
    };
    // Saved sessions were all started, whatever the count says.
    let started = profile.started.max(profile.sessions as u64);
    let completed = match started {
        0 => "0".to_string(),
        started => format!(
            "{} ({:.0}%)",
            thousands(profile.completed as u64),
            profile.completed as f64 * 100.0 / started as f64
        ),
    };
    let lines = vec![
        Line::from(vec![
            label(app, "level"),
            Span::styled(
                format!("{:<4}", level.level),
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            ),
            value("█".repeat(filled), theme.accent),
            value("░".repeat(10 - filled), theme.pending),
            value(format!(" {:.0}%", level.ratio() * 100.0), theme.pending),
        ]),
        Line::from(vec![
            label(app, "xp"),
            value(thousands(profile.xp), theme.accent),
            value(
                format!(" ({} to go)", thousands(level.needed - level.into)),
                theme.pending,
            ),
        ]),
        Line::from(vec![
            label(app, "since"),
            value(
                profile
                    .since
                    .map_or("—".into(), |day| day.format("%Y-%m-%d").to_string()),
                theme.neutral,
            ),
        ]),
        Line::from(vec![
            label(app, "days typed"),
            value(profile.days.len().to_string(), theme.neutral),
        ]),
        Line::from(vec![
            label(app, "streak"),
            value(
                if profile.streak == 0 {
                    "—".into()
                } else {
                    days(profile.streak)
                },
                theme.secondary,
            ),
            value(
                format!("  (best {})", days(profile.best_streak)),
                theme.pending,
            ),
        ]),
        Line::from(vec![
            label(app, "started"),
            value(thousands(started), theme.neutral),
        ]),
        Line::from(vec![
            label(app, "completed"),
            value(completed, theme.correct),
        ]),
        Line::from(vec![
            label(app, "time typed"),
            value(
                super::stats::format_duration(profile.time_typed_secs),
                theme.neutral,
            ),
        ]),
        Line::from(vec![
            label(app, "keys typed"),
            value(thousands(profile.chars_typed), theme.neutral),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.pending))
                .title(" you "),
        ),
        area,
    );
}

/// The right box: the plain time and words bests side by side, and the
/// mode played most.
fn render_bests(f: &mut Frame, app: &App, area: Rect, profile: &Profile) {
    let theme = &app.theme;
    let header = Style::default()
        .fg(theme.pending)
        .add_modifier(Modifier::BOLD);
    // One side: "15s   72.1  98% " — the count, its best, that run's
    // accuracy: 17 cells, so both sides fit a half of 80 columns.
    let side = |name: String, best: StandardBest| -> Vec<Span<'static>> {
        let name = Span::styled(format!("{name:<6}"), Style::default().fg(theme.mode_tag));
        match best {
            Some((wpm, accuracy, _)) => vec![
                name,
                Span::styled(format!("{wpm:<6.1}"), Style::default().fg(theme.secondary)),
                Span::styled(
                    format!("{:<5}", format!("{accuracy:.0}%")),
                    Style::default().fg(theme.correct),
                ),
            ],
            None => vec![
                name,
                Span::styled(format!("{:<11}", "—"), Style::default().fg(theme.pending)),
            ],
        }
    };
    // The header uses the same widths as the rows, so it can't drift.
    let mut lines = vec![Line::from(Span::styled(
        format!(
            "  {:<6}{:<6}{:<5}  {:<6}{:<6}{:<5}",
            "time", "wpm", "acc", "words", "wpm", "acc"
        ),
        header,
    ))];
    for i in 0..4 {
        let mut spans = vec![Span::raw("  ")];
        spans.extend(side(format!("{}s", STANDARD_TIMES[i]), profile.times[i]));
        spans.push(Span::raw("  "));
        spans.extend(side(STANDARD_WORDS[i].to_string(), profile.words[i]));
        lines.push(Line::from(spans));
    }
    lines.push(Line::raw(""));
    let room = (area.width as usize).saturating_sub(26);
    lines.push(Line::from(vec![
        Span::styled("  most played  ", Style::default().fg(theme.pending)),
        match &profile.favourite {
            Some((mode, runs)) => Span::styled(
                format!(
                    "{} ({})",
                    shorten(&printable(mode), room),
                    thousands(*runs as u64)
                ),
                Style::default().fg(theme.mode_tag),
            ),
            None => Span::styled("—", Style::default().fg(theme.pending)),
        },
    ]));
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.pending))
                .title(" bests (plain runs) "),
        ),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A day's shade is its share of the busiest day, in quarters; any
    /// typing at all is at least the lightest shade.
    #[test]
    fn shades_by_quarter_of_the_busiest_day() {
        assert_eq!(shade(0.0, 100.0), 0);
        assert_eq!(shade(1.0, 100.0), 1);
        assert_eq!(shade(25.0, 100.0), 1);
        assert_eq!(shade(26.0, 100.0), 2);
        assert_eq!(shade(75.0, 100.0), 3);
        assert_eq!(shade(100.0, 100.0), 4);
        assert_eq!(shade(5.0, 0.0), 0);
    }

    #[test]
    fn thousands_are_separated() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(81_234), "81,234");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
