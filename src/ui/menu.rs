//! Main menu screen — ASCII banner up top, the modes grouped by category in
//! the middle (heading line, options on the line below; ↑/↓ picks a row,
//! ←/→ an option), the word settings on the list's bottom border, and a
//! one-line hint footer at the bottom.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use super::key;
use crate::app::{menu_row, App, ClickAction, MenuAction};
use crate::words::WordPool;

/// Narrowest box (borders included) that fits the seven code options on one
/// line, so the code row doesn't wrap on a standard 80-column terminal.
const MIN_BOX_WIDTH: u16 = 64;

/// The big banner's width; narrower terminals get the compact one.
const BANNER_WIDTH: u16 = 70;

/// How the title is drawn. The menu picks the biggest that still lets every
/// option fit on screen (see [`render`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Banner {
    /// One row of breathing room, then the five-row block-letter banner.
    Full,
    /// The two-row half-block banner.
    Compact,
    /// No title at all — only when nothing else fits.
    Hidden,
}

impl Banner {
    /// Rows the banner takes, padding included.
    fn rows(self) -> u16 {
        match self {
            Banner::Full => 6,
            Banner::Compact => 2,
            Banner::Hidden => 0,
        }
    }

    fn text(self) -> &'static [&'static str] {
        match self {
            Banner::Full => &[
                "",
                "  ████████ ██    ██ ██████  ███████ ██████  ██    ██ ███████ ██   ██ ",
                "     ██     ██  ██  ██   ██ ██      ██   ██ ██    ██ ██      ██   ██ ",
                "     ██      ████   ██████  █████   ██████  ██    ██ ███████ ███████ ",
                "     ██       ██    ██      ██      ██   ██ ██    ██      ██ ██   ██ ",
                "     ██       ██    ██      ███████ ██   ██  ██████  ███████ ██   ██ ",
            ],
            Banner::Compact => &[
                "▀█▀ ▀▄▀ █▀█ █▀▀ █▀█ █ █ █▀▀ █ █",
                " █   █  █▀▀ ██▄ █▀▄ █▄█ ▄▄█ █▀█",
            ],
            Banner::Hidden => &[],
        }
    }
}

/// The mode list laid out as lines of text, before scrolling.
struct MenuLines {
    lines: Vec<Line<'static>>,
    /// Clickable options as (line, x offset, width, menu index); turned into
    /// screen rects once the scroll offset is known.
    chips: Vec<(usize, u16, u16, usize)>,
    /// Line holding the highlighted option.
    selected_line: usize,
    /// Blank lines between categories in `lines`.
    gaps: usize,
}

/// Render the menu screen. `app.menu_index` highlights the active option.
///
/// Every option should be on screen at once, on a standard 80×24 terminal
/// too. The layout steps down only as far as it must: the full banner with a
/// blank line between categories, else the compact banner with as many of
/// those blank lines as fit (the top ones go first), else no banner. Only
/// when even that doesn't fit does the list scroll, with "more" hints in the
/// border so nothing is hidden without a sign.
/// Returns the footer hints.
pub fn render(f: &mut Frame, app: &App, area: Rect) -> &'static [super::Hint] {
    let box_width = (area.width * 3 / 5).max(MIN_BOX_WIDTH).min(area.width);
    let text_width = box_width.saturating_sub(2) as usize;
    // Box borders.
    const CHROME: usize = 2;
    let spaced = menu_lines(app, text_width, 0);
    let dense_rows = spaced.lines.len() - spaced.gaps;
    // Blank lines that fit with `banner` above the list (`None`: not even
    // the list without any fits).
    let gaps_that_fit = |banner: Banner| {
        (area.height as usize).checked_sub(banner.rows() as usize + CHROME + dense_rows)
    };
    let (banner, menu) = if area.width >= BANNER_WIDTH
        && gaps_that_fit(Banner::Full).is_some_and(|fit| fit >= spaced.gaps)
    {
        (Banner::Full, spaced)
    } else {
        let banner = [Banner::Compact, Banner::Hidden]
            .into_iter()
            .find(|banner| gaps_that_fit(*banner).is_some())
            .unwrap_or(Banner::Hidden);
        let fit = gaps_that_fit(banner).unwrap_or(0);
        let menu = if fit >= spaced.gaps {
            spaced
        } else {
            menu_lines(app, text_width, spaced.gaps - fit)
        };
        (banner, menu)
    };

    let layout = Layout::vertical([
        Constraint::Length(banner.rows()),
        Constraint::Min(0), // mode list
    ])
    .split(area);

    let theme = &app.theme;
    let banner_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let banner_lines: Vec<Line> = banner
        .text()
        .iter()
        .map(|line| Line::from(Span::styled(*line, banner_style)))
        .collect();
    f.render_widget(
        Paragraph::new(banner_lines).alignment(Alignment::Center),
        layout[0],
    );

    let inner = layout[1].centered_horizontally(Constraint::Length(box_width));
    // Text area inside the border — clicks outside it can't hit an option.
    let menu_area = Rect::new(
        inner.x + 1,
        inner.y + 1,
        inner.width.saturating_sub(2),
        inner.height.saturating_sub(2),
    );

    // Too short even so: scroll so the selected row (and its heading, just
    // above it) stays visible.
    let visible = menu_area.height as usize;
    let scroll = (menu.selected_line + 1).saturating_sub(visible);
    let more_above = scroll > 0;
    // The trailing blank line is padding, not an option: it never counts
    // as "more".
    let more_below = scroll + visible < menu.lines.len() - 1;
    let mut targets = app.click_targets.borrow_mut();
    for (line, x, width, index) in menu.chips {
        if line < scroll || line - scroll >= visible {
            continue;
        }
        let y = menu_area.y + (line - scroll) as u16;
        let area = Rect::new(menu_area.x + x, y, width, 1).intersection(menu_area);
        if !area.is_empty() {
            targets.push((area, ClickAction::Menu(index)));
        }
    }

    // Word settings for time / words runs, on the bottom border: each is a
    // clickable chip that acts like its key.
    let (settings, chips) = word_settings(app);
    let border_row = Rect::new(
        inner.x + 1,
        inner.bottom().saturating_sub(1),
        inner.width.saturating_sub(2),
        1,
    );
    if inner.height >= 2 {
        for (x, width, code) in chips {
            let area = Rect::new(border_row.x + x, border_row.y, width, 1).intersection(border_row);
            if !area.is_empty() {
                targets.push((
                    area,
                    ClickAction::Key(KeyCode::Char(code), KeyModifiers::NONE),
                ));
            }
        }
    }
    drop(targets);

    let hint_style = Style::default().fg(theme.pending);
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.neutral))
        .title(Span::styled(
            " select mode ",
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        ))
        .title_bottom(settings);
    if more_above {
        block = block.title(Line::styled(" ▲ more ", hint_style).right_aligned());
    }
    if more_below {
        block = block.title_bottom(Line::styled(" ▼ more ", hint_style).right_aligned());
    }
    let list = Paragraph::new(menu.lines)
        .scroll((scroll as u16, 0))
        .block(block);
    f.render_widget(list, inner);

    const HINTS: &[super::Hint] = &[
        ("↑/↓ category", None),
        ("←/→ option", None),
        ("Enter start", key(KeyCode::Enter)),
        ("s stats", key(KeyCode::Char('s'))),
        ("? help", key(KeyCode::Char('?'))),
        ("q quit", key(KeyCode::Char('q'))),
    ];
    HINTS
}

/// The `p` / `n` / `b` word settings drawn on the menu's bottom border, and
/// each one's (x offset, width, key) for clicking. The state is spelled out
/// ("on" / "off"), not shown by color alone.
fn word_settings(app: &App) -> (Line<'static>, Vec<(u16, u16, char)>) {
    let theme = &app.theme;
    let settings = [
        ('p', "punctuation", app.word_decor.punctuation),
        ('n', "numbers", app.word_decor.numbers),
        ('b', "10k words", app.word_pool == WordPool::Extended),
    ];
    let mut spans = vec![Span::raw(" ")];
    let mut chips = Vec::new();
    let mut x = 1u16;
    for (code, name, on) in settings {
        let (name_style, state_style) = if on {
            (
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(theme.correct)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (
                Style::default().fg(theme.pending),
                Style::default().fg(theme.pending),
            )
        };
        let chip = [
            Span::styled(
                format!("{code} "),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{name} "), name_style),
            Span::styled(if on { "on " } else { "off" }, state_style),
        ];
        let width: u16 = chip.iter().map(|span| span.width() as u16).sum();
        chips.push((x, width, code));
        spans.extend(chip);
        spans.push(Span::raw("  "));
        x += width + 2;
    }
    (Line::from(spans), chips)
}

/// Lay out the mode list for a box `width` cells wide (inside the border).
/// A blank line separates categories (see below), except the first
/// `skip_gaps` of them — dropped when the screen is too short for all.
fn menu_lines(app: &App, width: usize, skip_gaps: usize) -> MenuLines {
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
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut chips = Vec::new();
    let mut selected_line = 0;
    let mut previous_row: Option<std::ops::Range<usize>> = None;
    let mut gaps = 0;
    let mut skipped = 0;
    // A row gets a heading line when it has several options, or when its one
    // option is a file (a single custom file or snippet — even one named
    // `custom.txt` must not look like the bare "nothing yet" placeholder).
    let has_heading =
        |row: &std::ops::Range<usize>| row.len() > 1 || app.menu[row.start].custom_path.is_some();
    let mut start = 0;
    while start < app.menu.len() {
        let row = menu_row(&app.menu, start);
        let is_selected_row = row == selected_row;
        let group = app.menu[start].group;
        // Breathing room between groups. Consecutive single-option rows of the
        // same kind (quote/zen start a game, stats/quit don't) stay together.
        if let Some(previous) = &previous_row {
            let starts_game = |index: usize| {
                app.menu[index].action != MenuAction::ShowStats
                    && app.menu[index].action != MenuAction::Quit
            };
            let same_kind = starts_game(previous.start) == starts_game(start);
            if has_heading(previous) || has_heading(&row) || !same_kind {
                if skipped < skip_gaps {
                    skipped += 1;
                } else {
                    lines.push(Line::raw(""));
                    gaps += 1;
                }
            }
        }

        let marker = Span::styled(
            if is_selected_row { " ➤ " } else { "   " },
            Style::default().fg(theme.accent),
        );
        let headed = has_heading(&row);
        if headed {
            // Category heading on its own line, options below.
            let heading_style = if is_selected_row {
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.pending)
            };
            lines.push(Line::from(vec![
                marker.clone(),
                Span::styled(format!(" {}", group), heading_style),
            ]));
        }

        const INDENT: &str = "   ";
        let mut spans: Vec<Span<'static>> = vec![if headed { Span::raw(INDENT) } else { marker }];
        let mut used = spans[0].width();
        for index in row.clone() {
            let style = if index == app.menu_index {
                selected_style
            } else {
                option_style
            };
            // Pad to 4 so the time and words options line up in columns.
            let span = Span::styled(format!(" {:<4} ", app.menu[index].label), style);
            // Wrap onto an indented continuation line rather than letting a
            // long row (many snippets, a narrow terminal) run off the border.
            if used > INDENT.len() && used + span.width() > width {
                lines.push(Line::from(std::mem::replace(
                    &mut spans,
                    vec![Span::raw(INDENT)],
                )));
                used = INDENT.len();
            }
            let line = lines.len();
            if index == app.menu_index {
                selected_line = line;
            }
            chips.push((line, used as u16, span.width() as u16, index));
            used += span.width() + 1;
            spans.push(span);
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
        start = row.end;
        previous_row = Some(row);
    }
    // A blank line under the last option, so the word settings on the
    // bottom border don't sit right against it. Always kept: when space is
    // short, the gaps between categories go first.
    lines.push(Line::raw(""));
    MenuLines {
        lines,
        chips,
        selected_line,
        gaps,
    }
}
