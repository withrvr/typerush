//! UI dispatcher.
//!
//! Each screen lives in its own submodule and exposes a `render(frame, app)`
//! function. The top-level `render` here just looks at `app.screen` and calls
//! the right one. The Help overlay and any error modal are drawn on top of
//! whatever screen is underneath.
//!
//! Before any screen renders, we paint **every cell** in the frame buffer
//! with the active theme's background. We use `buffer_mut().set_style` rather
//! than rendering a Block widget so the bg fill is unconditional — no widget
//! render path gets a chance to skip cells. The `dark` theme uses
//! `Color::Reset` here so the user's terminal background shines through
//! exactly as it did in v0.1.
//!
//! Note: some terminals add a few pixels of padding *around* the character
//! grid (e.g. Windows Terminal defaults to 8px). That padding lives outside
//! anything ratatui can paint — if a user reports a thin stripe along the
//! edges in non-default themes, it's the terminal's padding setting, not
//! TypeRush.

pub mod help;
pub mod menu;
pub mod results;
pub mod stats;
pub mod typing;

use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{prelude::*, widgets::Paragraph, Frame};

use crate::app::{App, ClickAction, Screen};

/// Single entry point called once per frame from the main event loop.
pub fn render(frame: &mut Frame, app: &App) {
    // 0. Click targets are rebuilt from scratch by whatever draws this frame.
    app.click_targets.borrow_mut().clear();

    // 1. Paint every cell in the frame with the theme background.
    let area = frame.area();
    frame
        .buffer_mut()
        .set_style(area, Style::default().bg(app.theme.background));

    // 2. Render the active screen above the footer row, then the footer:
    //    every screen's key hints sit on the last row, in the same place.
    let [body, footer] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);
    let hints = match app.screen {
        Screen::Menu => menu::render(frame, app, body),
        Screen::Typing => typing::render(frame, app, body),
        Screen::Results => results::render(frame, app, body),
        Screen::Stats => stats::render(frame, app, body),
        Screen::Help => help::render(frame, app, body),
    };
    render_footer(frame, app, footer, hints);
    // 3. Error overlay sits on top of everything else when present.
    if let Some(message) = &app.error_message {
        help::render_error(frame, &app.theme, message);
    }
}

/// Width of the label column inside boxes ("accuracy", "time typed", …).
/// Labels are padded to it on every screen, so values line up in one column.
pub const LABEL_WIDTH: usize = 12;

/// A muted label padded to [`LABEL_WIDTH`], two cells in from the box border
/// like all text inside a box: `"  accuracy    "`.
pub fn label(app: &App, text: &str) -> Span<'static> {
    Span::styled(
        format!("  {text:<LABEL_WIDTH$}"),
        Style::default().fg(app.theme.pending),
    )
}

/// A footer hint: its text and, when clicking it should do something, the key
/// press it stands for. Navigation hints like "↑/↓ category" carry `None`.
pub type Hint = (&'static str, Option<(KeyCode, KeyModifiers)>);

/// Shorthand for a clickable hint that acts like pressing `code`.
pub const fn key(code: KeyCode) -> Option<(KeyCode, KeyModifiers)> {
    Some((code, KeyModifiers::NONE))
}

/// Draw the one-line key-hint footer ("Enter start  ·  q quit") on the last
/// row and register each actionable hint as a click target, so every footer
/// action works with the keyboard *and* the mouse. Each screen's `render`
/// returns its hints; this is the only place they are drawn.
fn render_footer(frame: &mut Frame, app: &App, area: Rect, hints: &[Hint]) {
    let style = Style::default().fg(app.theme.pending);
    let mut spans = vec![Span::raw("  ")];
    let mut x = area.x + 2;
    let mut targets = app.click_targets.borrow_mut();
    for (i, (text, action)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  ·  "));
            x += 5;
        }
        let span = Span::styled(*text, style);
        let width = span.width() as u16;
        if let Some((code, mods)) = action {
            targets.push((
                Rect::new(x, area.y, width, 1).intersection(area),
                ClickAction::Key(*code, *mods),
            ));
        }
        spans.push(span);
        x += width;
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(style), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Mode;
    use crate::theme::builtin;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// Render the menu screen into a fake terminal and return the cell at
    /// `(x, y)`. Far-corner cells aren't touched by any widget on the menu
    /// screen, so they reflect the theme's background paint directly.
    fn render_and_sample(palette: crate::theme::ThemePalette, x: u16, y: u16) -> (Color, Color) {
        let app = App::new(None, palette, Mode::Time(15));
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let cell = &buffer[(x, y)];
        (cell.fg, cell.bg)
    }

    /// v0.1 compatibility: the dark theme must NOT force any explicit bg.
    /// `Color::Reset` lets the user's terminal background shine through —
    /// regressing this would change the look on every terminal.
    #[test]
    fn dark_theme_leaves_background_as_reset() {
        let (_fg, bg) = render_and_sample(builtin::DARK, 79, 23);
        assert_eq!(bg, Color::Reset);
    }

    /// Non-default themes paint their canonical background on every cell.
    /// Sampling a far-corner cell guarantees we're reading the painted bg
    /// rather than something a widget happened to render there.
    #[test]
    fn monokai_theme_paints_canonical_background() {
        let (_fg, bg) = render_and_sample(builtin::MONOKAI, 79, 23);
        assert_eq!(bg, Color::Rgb(0x27, 0x28, 0x22));
    }

    #[test]
    fn dracula_theme_paints_canonical_background() {
        let (_fg, bg) = render_and_sample(builtin::DRACULA, 79, 23);
        assert_eq!(bg, Color::Rgb(0x28, 0x2A, 0x36));
    }

    #[test]
    fn light_theme_paints_off_white_background() {
        let (_fg, bg) = render_and_sample(builtin::LIGHT, 79, 23);
        assert_eq!(bg, Color::Rgb(0xFA, 0xFA, 0xFA));
    }

    /// Regression (#8): a wrong key on a space must show as a red, underlined
    /// space — a plain colored blank would be invisible.
    #[test]
    fn wrong_space_renders_red_underlined() {
        let palette = builtin::DARK;
        let mut app = App::new(None, palette, Mode::Time(15));
        app.mode = crate::app::Mode::Custom;
        app.words = vec![
            crate::app::Word::new("hi".into()),
            crate::app::Word::new("bye".into()),
        ];
        app.screen = crate::app::Screen::Typing;
        for ch in "hix".chars() {
            app.handle_char(ch);
        }

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, &app)).unwrap();
        let buffer = terminal.backend().buffer();

        let has_red_space = buffer.content().iter().any(|cell| {
            cell.symbol() == " "
                && cell.fg == palette.incorrect
                && cell.modifier.contains(Modifier::UNDERLINED)
        });
        assert!(has_red_space);
    }

    /// A multi-line error (a TOML parse error points at the bad line with a
    /// caret) keeps its line breaks and indent, fits in the box with the
    /// dismiss hint, and an escape character in it doesn't reach the terminal.
    #[test]
    fn error_modal_keeps_lines_and_hides_control_characters() {
        let mut app = App::new(None, builtin::DARK, Mode::Time(15));
        app.error_message = Some(
            "TOML parse error at line 3, column 5\n  |\n3 | foo =\x1b[2J\n  |     ^\nexpected value"
                .to_string(),
        );
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| render(f, &app)).unwrap();
        let rows: Vec<String> = terminal
            .backend()
            .buffer()
            .content()
            .chunks(80)
            .map(|row| row.iter().map(|c| c.symbol()).collect())
            .collect();
        let dump = rows.join("\n");
        let row_of = |needle: &str| {
            rows.iter()
                .position(|r| r.contains(needle))
                .unwrap_or_else(|| panic!("{needle:?} not shown:\n{dump}"))
        };
        let first = row_of("  TOML parse error");
        assert_eq!(row_of("  3 | foo =?[2J"), first + 2, "{dump}");
        assert_eq!(row_of("expected value"), first + 4, "{dump}");
        assert!(row_of("press any key") > first + 4, "{dump}");
        assert!(!dump.contains('\x1b'));
    }

    /// Symbols mode shows a token-count gauge like words mode does.
    #[test]
    fn symbols_mode_shows_token_progress() {
        let mut app = App::new(None, builtin::DARK, Mode::Time(15));
        app.start_game(crate::app::Mode::Symbols(2)).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| render(f, &app)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("0 / 2"), "gauge missing");
        assert!(text.contains("symbols-2"), "mode tag missing");
    }

    /// Mode labels in the history table are made safe (stats.json can be
    /// hand-edited; custom labels carry file names) and long ones shortened
    /// to the column instead of being cut off.
    #[test]
    fn stats_table_mode_labels_are_safe_and_shortened() {
        let mut app = App::new(None, builtin::DARK, Mode::Time(15));
        app.stats_cache = Some(vec![crate::storage::SessionRecord {
            wpm: 80.0,
            accuracy: 97.0,
            mode: "custom-\u{1b}[2Jquarterly-report-final-draft".into(),
            word_count: 10,
            correct_chars: 50,
            total_chars: 52,
            duration_secs: 30.0,
            timestamp: chrono::Local::now(),
            key_hits: Default::default(),
            key_misses: Default::default(),
        }]);
        app.screen = crate::app::Screen::Stats;
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        terminal.draw(|f| render(f, &app)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(!text.contains('\x1b'));
        assert!(text.contains("custom-?…nal-draft "), "{text}");
    }

    /// Draw the Stats screen for `sessions` at 80×24 and return its rows.
    fn stats_rows(app: &App) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| render(f, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .chunks(80)
            .map(|row| row.iter().map(|c| c.symbol()).collect())
            .collect()
    }

    fn record(wpm: f64, mode: &str) -> crate::storage::SessionRecord {
        crate::storage::SessionRecord {
            wpm,
            accuracy: 97.0,
            mode: mode.into(),
            word_count: 10,
            correct_chars: 50,
            total_chars: 52,
            duration_secs: 30.0,
            timestamp: chrono::Local::now(),
            key_hits: Default::default(),
            key_misses: Default::default(),
        }
    }

    /// The session holding a mode's best is starred in the sessions list,
    /// the list scrolls (clamped to a full last page), and the bests
    /// category lists one row per mode.
    #[test]
    fn stats_screen_stars_bests_scrolls_and_lists_bests() {
        let mut app = App::new(None, builtin::DARK, Mode::Time(15));
        let mut history: Vec<_> = (0..12)
            .map(|i| record(40.0 + f64::from(i), "time-30s"))
            .collect();
        history.push(record(55.0, "words-25"));
        app.stats_cache = Some(history);
        app.screen = crate::app::Screen::Stats;
        let sessions = app.stats_cache.as_deref().unwrap();
        app.stats_view = Some(crate::storage::StatsView::new(sessions, None));

        let rows = stats_rows(&app);
        let dump = rows.join("\n");
        assert!(
            rows.iter()
                .any(|r| r.contains("★") && r.contains("words-25")),
            "{dump}"
        );
        assert!(
            rows.iter().any(|r| r.contains("★") && r.contains("51.0")),
            "{dump}"
        );
        assert!(dump.contains("sessions · 1–"), "{dump}");
        assert!(dump.contains("[all]   ‹ ›   01/04   all"), "{dump}");

        // Scrolling far past the end shows the last page, oldest at the bottom.
        app.stats_view.as_ref().unwrap().scroll_by(1000);
        let dump = stats_rows(&app).join("\n");
        assert!(dump.contains("of 13 "), "{dump}");
        assert!(dump.contains("40.0"), "{dump}");
        let first = app.stats_view.as_ref().unwrap().scroll.get();
        assert!(first > 0 && first < 12, "clamped scroll {first}");

        let sessions = app.stats_cache.as_deref().unwrap();
        app.stats_view.as_mut().unwrap().cycle(sessions, true);
        let dump = stats_rows(&app).join("\n");
        assert!(dump.contains("bests per mode"), "{dump}");
        assert!(dump.contains("★ 51.0"), "{dump}");
        assert!(dump.contains("★ 55.0"), "{dump}");
    }

    /// A window too short for any table row (three rows tall: the table's
    /// minimum gives way only there) says how many sessions there are instead
    /// of an impossible range, and keeps the scroll.
    #[test]
    fn stats_table_too_short_for_rows() {
        let mut app = App::new(None, builtin::DARK, Mode::Time(15));
        app.stats_cache = Some((0..13).map(|_| record(50.0, "time-30s")).collect());
        app.screen = crate::app::Screen::Stats;
        let view = crate::storage::StatsView::new(app.stats_cache.as_deref().unwrap(), None);
        view.scroll.set(3);
        app.stats_view = Some(view);
        let mut terminal = Terminal::new(TestBackend::new(80, 3)).unwrap();
        terminal.draw(|f| render(f, &app)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("sessions · 13 "), "{text}");
        assert_eq!(app.stats_view.as_ref().unwrap().scroll.get(), 3);
    }

    /// Every screen's key hints are on the last row, starting in the same
    /// column, so they never jump around when the screen changes.
    #[test]
    fn footer_is_on_the_last_row_of_every_screen() {
        use crate::app::Screen;
        for (height, screen, first_hint) in [
            (24, Screen::Menu, "↑/↓ category"),
            (24, Screen::Typing, "Ctrl+R / F5 restart"),
            (24, Screen::Results, "Enter / F5 restart"),
            (24, Screen::Stats, "←/→ category"),
            (24, Screen::Help, "Esc / F1 close"),
            (40, Screen::Results, "Enter / F5 restart"),
        ] {
            let mut app = App::new(None, builtin::DARK, Mode::Time(15));
            app.start_game(Mode::Words(5)).unwrap();
            app.stats_cache = Some(Vec::new());
            app.screen = screen;
            let mut terminal = Terminal::new(TestBackend::new(80, height)).unwrap();
            terminal.draw(|f| render(f, &app)).unwrap();
            let last: String = (0..80)
                .map(|x| {
                    terminal.backend().buffer()[(x, height - 1)]
                        .symbol()
                        .to_string()
                })
                .collect();
            assert!(
                last.starts_with(&format!("  {first_hint}")),
                "{screen:?}: {last:?}"
            );
        }
    }

    /// One grid on every screen: the title starts at column 2 of the first
    /// row, and inside boxes every value starts in the same column (border,
    /// two cells, the shared label width) — on Results and in the Stats
    /// summary alike.
    #[test]
    fn titles_and_values_line_up_across_screens() {
        use crate::app::Screen;
        let value_column = 1 + 2 + LABEL_WIDTH;
        let rows_of = |app: &App| -> Vec<Vec<String>> {
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal.draw(|f| render(f, app)).unwrap();
            let buffer = terminal.backend().buffer();
            (0..24)
                .map(|y| {
                    (0..80)
                        .map(|x| buffer[(x, y)].symbol().to_string())
                        .collect()
                })
                .collect()
        };
        // Column of the first visible cell after `label` on its row.
        let value_after = |rows: &[Vec<String>], label: &str| -> usize {
            let row = rows
                .iter()
                .find(|row| row.concat().contains(&format!("  {label}  ")))
                .unwrap_or_else(|| panic!("{label:?} not shown"));
            let text: Vec<&str> = row.iter().map(String::as_str).collect();
            let start = (0..text.len())
                .find(|&x| text[x..].concat().starts_with(&format!("  {label}")))
                .unwrap();
            (start + 2 + label.chars().count()..text.len())
                .find(|&x| text[x].trim() != "")
                .unwrap()
        };

        let mut app = App::new(None, builtin::DARK, Mode::Time(15));
        app.start_game(Mode::Words(2)).unwrap();
        app.stats_cache = Some(vec![record(61.5, "words-2")]);
        for screen in [Screen::Menu, Screen::Typing, Screen::Results, Screen::Stats] {
            app.screen = screen;
            let rows = rows_of(&app);
            let first = rows[0].iter().position(|cell| cell.trim() != "").unwrap();
            assert_eq!(first, 2, "{screen:?} title column");
            let labels: &[&str] = match screen {
                Screen::Results => &["wpm", "accuracy", "time", "chars", "mode", "mode best"],
                Screen::Stats => &[
                    "best wpm",
                    "sessions",
                    "streak",
                    "7-day avg",
                    "30-day avg",
                    "time typed",
                ],
                _ => &[],
            };
            for label in labels {
                assert_eq!(
                    value_after(&rows, label),
                    value_column,
                    "{screen:?} {label:?}"
                );
            }
            if screen == Screen::Menu {
                // The menu box is full width like the others, and its text
                // starts two cells in from the border too.
                let words = rows
                    .iter()
                    .find(|row| row.concat().contains("words"))
                    .unwrap();
                assert_eq!(words[0], "│", "menu box starts at column 0");
                let column = (0..80)
                    .find(|&x| words[x..].concat().starts_with("words"))
                    .unwrap();
                assert_eq!(column, 3, "menu text column");
            }
        }
    }

    /// The Stats title keeps [all], ‹ › and the zero-padded counter in the
    /// same columns whatever the category's name is; only the name, last on
    /// the row, changes.
    #[test]
    fn stats_title_buttons_never_move() {
        let mut app = App::new(None, builtin::DARK, Mode::Time(15));
        app.stats_cache = Some(vec![
            record(50.0, "time-30s"),
            record(60.0, "custom-a-very-long-file-name"),
        ]);
        app.screen = crate::app::Screen::Stats;
        let sessions = app.stats_cache.as_deref().unwrap();
        app.stats_view = Some(crate::storage::StatsView::new(sessions, None));
        let mut columns = Vec::new();
        for _ in 0..app.stats_view.as_ref().unwrap().category_count() {
            let rows = stats_rows(&app);
            let title = &rows[0];
            let at = |text: &str| {
                title
                    .find(text)
                    .unwrap_or_else(|| panic!("{text:?} in {title:?}"))
            };
            columns.push((at("[all]"), at("‹ ›"), at("/04")));
            assert!(
                title.contains(&format!("{:02}/04", columns.len())),
                "{title}"
            );
            let sessions = app.stats_cache.as_deref().unwrap();
            app.stats_view.as_mut().unwrap().cycle(sessions, true);
        }
        assert!(
            columns.windows(2).all(|pair| pair[0] == pair[1]),
            "{columns:?}"
        );
    }
}
