//! Settings screen — the saved settings (theme, start mode, word settings)
//! and "reset to defaults", in the same grid as every screen: a title row,
//! a blank row, one full-width box with labels two cells in and values in
//! the shared column. Reads only `App::settings`; never the disk.

use ratatui::crossterm::event::KeyCode;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use super::{key, selected_style, LABEL_WIDTH};
use crate::app::{App, ClickAction};
use crate::settings::{mode_name, Row, SettingsView, ROWS};
use crate::text::{printable, screen_path, shorten};
use crate::words::WordPool;

/// Render the Settings screen. Returns the footer hints.
pub fn render(f: &mut Frame, app: &App, area: Rect) -> &'static [super::Hint] {
    let theme = &app.theme;
    let [title_area, box_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(area);
    f.render_widget(
        Paragraph::new(Span::styled(
            "  ◆ settings",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        title_area,
    );

    // Normally loaded on screen entry (main loop); the built-in defaults
    // only if something draws Settings without going through it (tests).
    let fallback;
    let view = match &app.settings {
        Some(view) => view,
        None => {
            fallback = SettingsView::load(None, 0);
            &fallback
        }
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.pending))
        .title(" saved settings ");
    let inner = block.inner(box_area);
    let saved_to = app.config_file.as_ref().map_or_else(
        || " not saved ".to_string(),
        |path| {
            let room = (inner.width as usize).saturating_sub(12);
            format!(" saved to {} ", shorten(&screen_path(path), room))
        },
    );
    let block = block.title_bottom(Span::styled(saved_to, Style::default().fg(theme.pending)));
    f.render_widget(block, box_area);

    // A blank line, the settings, a blank line, reset, then any problem
    // with the file.
    let mut lines = vec![Line::raw("")];
    let mut targets = app.click_targets.borrow_mut();
    for (index, row) in ROWS.iter().enumerate() {
        if *row == Row::Reset {
            lines.push(Line::raw(""));
        }
        let y = inner.y + lines.len() as u16;
        let (line, chips) = row_line(app, view, index, *row);
        for (x, width, code) in chips {
            let cell = Rect::new(inner.x + x, y, width, 1).intersection(inner);
            if !cell.is_empty() {
                targets.push((cell, ClickAction::Setting(index, code)));
            }
        }
        lines.push(line);
    }
    drop(targets);
    if let Some(problem) = &view.problem {
        // The first line of the loader's warning: the file is still used
        // where it can be, and the problem stays until it is fixed.
        let first = problem.lines().next().unwrap_or("");
        let room = (inner.width as usize).saturating_sub(4);
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("  {}", shorten(&printable(first), room)),
            Style::default().fg(theme.error),
        ));
    }
    f.render_widget(Paragraph::new(lines), inner);

    const HINTS: &[super::Hint] = &[
        ("↑/↓ setting", None),
        ("←/→ change", None),
        ("Enter toggle", key(KeyCode::Enter)),
        ("Esc menu", key(KeyCode::Esc)),
        ("? help", key(KeyCode::Char('?'))),
    ];
    HINTS
}

/// One row: the ➤ marker in the two-cell gutter, its label, its value. The
/// clickable parts come back as (x offset in the box, width, key): ‹ and ›
/// act like ← and →, the value like Enter.
fn row_line(
    app: &App,
    view: &SettingsView,
    index: usize,
    row: Row,
) -> (Line<'static>, Vec<(u16, u16, KeyCode)>) {
    let theme = &app.theme;
    let selected = view.selected == index;
    // Text starts two cells in from the border, as in every box; the ➤
    // marker sits in that gutter.
    let marker = |cells: usize| {
        let text = if selected { "➤" } else { " " };
        Span::styled(format!("{text:<cells$}"), Style::default().fg(theme.accent))
    };
    let chip_style = |style: Style| {
        if selected {
            selected_style(theme)
        } else {
            style
        }
    };
    let arrow = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let value_x = (2 + LABEL_WIDTH) as u16;

    if row == Row::Reset {
        let (text, hint) = if view.confirm_reset {
            (
                " Enter again to reset every setting ",
                "  ·  Esc keeps them",
            )
        } else {
            (" reset to defaults ", "")
        };
        let chip = Span::styled(text, chip_style(Style::default().fg(theme.neutral)));
        let width = chip.width() as u16;
        // A one-cell marker: the chip's own leading space puts its text in
        // the label column.
        let line = Line::from(vec![
            marker(1),
            chip,
            Span::styled(hint, Style::default().fg(theme.pending)),
        ]);
        return (line, vec![(1, width, KeyCode::Enter)]);
    }

    let (name, value) = match row {
        Row::Theme => ("theme", view.theme.clone()),
        Row::Start => ("start mode", mode_name(view.start)),
        Row::Punctuation => ("punctuation", on_off(view.decor.punctuation)),
        Row::Numbers => ("numbers", on_off(view.decor.numbers)),
        Row::Pool => ("10k words", on_off(view.pool == WordPool::Extended)),
        Row::LiveWpm => ("live wpm", on_off(view.typing.live_wpm)),
        Row::Blind => ("blind", on_off(view.typing.blind)),
        Row::StopOnError => ("stop on", view.typing.stop_on_error.name().to_string()),
        Row::Reset => unreachable!("drawn above"),
    };
    let label = Span::styled(
        format!("{name:<LABEL_WIDTH$}"),
        if selected {
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.pending)
        },
    );
    // Every value is "‹ value ›", so they line up and each has the same
    // three targets. A switch says "on" / "off" in words, not only in color.
    let value_style = match value.as_str() {
        "on" => Style::default()
            .fg(theme.correct)
            .add_modifier(Modifier::BOLD),
        "off" => Style::default().fg(theme.pending),
        _ => Style::default().fg(theme.neutral),
    };
    let chip = Span::styled(format!(" {value} "), chip_style(value_style));
    let width = chip.width() as u16;
    let chips = vec![
        (value_x, 1, KeyCode::Left),
        (value_x + 1, width, KeyCode::Enter),
        (value_x + 1 + width, 1, KeyCode::Right),
    ];
    let line = Line::from(vec![
        marker(2),
        label,
        Span::styled("‹", arrow),
        chip,
        Span::styled("›", arrow),
    ]);
    (line, chips)
}

fn on_off(on: bool) -> String {
    if on { "on" } else { "off" }.to_string()
}
