//! The typing screen — header (live stats), progress bar, words area, footer.
//!
//! The cursor is **steady** (no blink) to avoid layout shifts. Both cursor
//! positions render the same way:
//! - on a character → underline in `theme.accent`
//! - on the space after a word → an underlined space in `theme.accent`
//!
//! Keeping the cursor a single visual style (rather than a "block fill" on
//! characters + "underline" on spaces) avoids the visual jolt of switching
//! styles as the cursor crosses word boundaries.

use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, Paragraph},
};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    app::{App, Mode},
    game::{get_char_states, CharState},
    theme::ThemePalette,
};

/// Top-level entry point for the typing screen. Splits the area into four
/// horizontal bands: header, progress bar, words, footer.
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let layout = Layout::vertical([
        Constraint::Length(3), // live stats header
        Constraint::Length(2), // progress gauge
        Constraint::Min(6),    // words to type
        Constraint::Length(3), // keybinding footer
    ])
    .split(area);

    render_header(f, app, layout[0]);
    render_progress(f, app, layout[1]);
    render_words(f, app, layout[2]);
    render_footer(f, app, layout[3]);
}

/// Renders the live WPM / accuracy / time / mode strip at the top of the screen.
/// In zen mode this is intentionally minimal (no numbers — just a label).
fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let is_zen_mode = matches!(app.mode, Mode::Zen);
    let theme = &app.theme;

    let timer_text = if let Some(remaining) = app.time_remaining() {
        format!("{:>3}s", remaining.as_secs())
    } else {
        format!("{:>5.1}s", app.elapsed().as_secs_f64())
    };

    let header = if is_zen_mode {
        Line::from(vec![
            Span::styled(" zen ", Style::default().fg(theme.pending)),
            Span::styled(" · esc to finish", Style::default().fg(theme.pending)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" wpm ", Style::default().fg(theme.pending)),
            Span::styled(
                format!("{:>3.0}", app.wpm()),
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("acc ", Style::default().fg(theme.pending)),
            Span::styled(
                format!("{:>5.1}%", app.accuracy()),
                Style::default().fg(theme.correct),
            ),
            Span::raw("   "),
            Span::styled("time ", Style::default().fg(theme.pending)),
            Span::styled(timer_text, Style::default().fg(theme.accent)),
            Span::raw("   "),
            Span::styled(
                // A custom label carries the file name: keep it to a size
                // that leaves the stats on the line readable.
                format!("[{}]", crate::text::shorten(&app.session_label(), 32)),
                Style::default().fg(theme.mode_tag),
            ),
        ])
    };

    let header_paragraph = Paragraph::new(header).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(theme.pending)),
    );
    f.render_widget(header_paragraph, area);
}

/// Progress bar — words-typed / total for word modes, elapsed / total for time modes.
/// Quote, code, zen and custom modes don't show a bar.
fn render_progress(f: &mut Frame, app: &App, area: Rect) {
    let gauge_color = app.theme.accent;
    // The label sits in the middle of the bar, often straddling the filled
    // (accent) and empty halves. Drawing it on the theme background keeps it
    // readable on both, whatever the accent is.
    let label_style = Style::default()
        .fg(app.theme.secondary)
        .bg(app.theme.background)
        .add_modifier(Modifier::BOLD);
    if let Some((done, total)) = app.progress() {
        let ratio = if total == 0 {
            0.0
        } else {
            done as f64 / total as f64
        };
        let gauge = Gauge::default()
            .block(Block::default())
            .gauge_style(Style::default().fg(gauge_color))
            .ratio(ratio.min(1.0))
            .label(Span::styled(format!("{} / {}", done, total), label_style));
        f.render_widget(gauge, area);
    } else if let Mode::Time(total) = app.mode {
        let elapsed = app.elapsed().as_secs_f64();
        let ratio = (elapsed / total as f64).min(1.0);
        let gauge = Gauge::default()
            .block(Block::default())
            .gauge_style(Style::default().fg(gauge_color))
            .ratio(ratio)
            .label(Span::styled(
                format!("{:.0}s / {}s", elapsed, total),
                label_style,
            ));
        f.render_widget(gauge, area);
    }
}

/// Renders the words to type, colored character by character.
///
/// Lays the text out by hand (no `Paragraph::wrap`) so the line of every
/// character is known: widths are terminal cells (a CJK character is two), a
/// word that doesn't fit moves to the next line whole, and a word wider than
/// a whole line is broken across lines. Each laid-out line is exactly one row
/// on screen, so the scroll below can't be thrown off by the widget wrapping
/// behind its back.
///
/// When the text is taller than the box it scrolls: the cursor's line stays
/// second from the top, so the line just typed is still visible above it.
/// Only the visible lines are styled — a long time run has hundreds of words,
/// and every keystroke redraws the screen.
///
/// The cursor never blinks: it is rendered as an underlined character (or
/// underlined trailing space when it sits on the space after the word) using
/// `theme.accent`. Keeping the cursor steady avoids the horizontal "jitter"
/// that a phantom blinking character would cause.
fn render_words(f: &mut Frame, app: &App, area: Rect) {
    // -4 to account for the surrounding border (1 char each side + padding).
    let max_width = area.width.saturating_sub(4).max(2) as usize;
    let visible_lines = area.height.saturating_sub(2) as usize;
    let last_word_index = app.words.len().saturating_sub(1);
    let has_space = |word_index: usize| word_index < last_word_index;
    // Next cell for a character `width` cells wide at (line, column).
    let place = |(line, column): (usize, usize), width: usize| {
        if column > 0 && column + width > max_width {
            (line + 1, 0)
        } else {
            (line, column)
        }
    };

    // 1. Lay out: where each word starts, and the cursor's line.
    //    ponytail: measures every word each frame (a cell count per
    //    character, ~0.2 ms a frame at 10,000 words); cache per word list if a run
    //    ever gets long enough to show up in a profile.
    let mut word_starts = Vec::with_capacity(app.words.len());
    let mut cursor_line = 0;
    let mut at = (0, 0);
    for (word_index, word) in app.words.iter().enumerate() {
        let width = word.text.width() + usize::from(has_space(word_index));
        at = place(at, width.min(max_width));
        word_starts.push(at);
        let is_active = word_index == app.current_word;
        if at.1 + width <= max_width && !is_active {
            at.1 += width;
            continue;
        }
        // Wider than what's left of the line, or the cursor is in it: walk
        // it character by character.
        let typed = word.typed.chars().count();
        let space = has_space(word_index).then_some(' ');
        for (char_index, ch) in word.text.chars().chain(space).enumerate() {
            let cells = ch.width().unwrap_or(0);
            at = place(at, cells);
            if is_active && char_index == typed {
                cursor_line = at.0;
            }
            at.1 += cells;
        }
    }
    if app.current_word >= app.words.len() {
        cursor_line = at.0;
    }
    let total_lines = if app.words.is_empty() { 0 } else { at.0 + 1 };

    // 2. Scroll so the cursor's line is second from the top, once the text
    //    no longer fits.
    let first_line = if total_lines > visible_lines {
        cursor_line
            .saturating_sub(1)
            .min(total_lines - visible_lines)
    } else {
        0
    };
    let end_line = (first_line + visible_lines).min(total_lines);

    // 3. Style only the characters on visible lines. Start from the last
    //    word that begins before the first visible line: a long word can
    //    run onto it.
    let is_zen_mode = matches!(app.mode, Mode::Zen);
    let cursor_style = Style::default()
        .fg(app.theme.accent)
        .add_modifier(Modifier::UNDERLINED);
    let mut lines: Vec<Vec<Span>> = vec![Vec::new(); end_line - first_line];
    let first_word = word_starts
        .partition_point(|&(line, _)| line < first_line)
        .saturating_sub(1);
    for (word_index, word) in app.words.iter().enumerate().skip(first_word) {
        let mut at = word_starts[word_index];
        if at.0 >= end_line {
            break;
        }
        let char_states = get_char_states(&word.text, &word.typed);
        let typed_char_count = word.typed.chars().count();
        let is_active_word = word_index == app.current_word;

        // Every character with its state-driven style; the cursor overlays
        // the character it sits on. The space between words is a character
        // like any other: the cursor lands on it once the word is typed.
        let space = has_space(word_index).then(|| {
            let style = if word.space_missed {
                // A colored blank is invisible — underline so the wrong space shows.
                style_for_char(CharState::Incorrect, is_zen_mode, &app.theme)
                    .add_modifier(Modifier::UNDERLINED)
            } else if is_active_word && typed_char_count >= char_states.len() {
                cursor_style
            } else {
                Style::default()
            };
            (' ', style)
        });
        let styled = char_states
            .iter()
            .enumerate()
            .map(|(char_index, (ch, state))| {
                let style = if is_active_word && char_index == typed_char_count {
                    cursor_style
                } else {
                    style_for_char(*state, is_zen_mode, &app.theme)
                };
                (*ch, style)
            });
        for (ch, style) in styled.chain(space) {
            let cells = ch.width().unwrap_or(0);
            at = place(at, cells);
            if (first_line..end_line).contains(&at.0) {
                lines[at.0 - first_line].push(Span::styled(ch.to_string(), style));
            }
            at.1 += cells;
        }
    }

    let words_paragraph = Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.pending))
                .title(Span::styled(
                    " typerush ",
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                )),
        );
    f.render_widget(words_paragraph, area);
}

/// Picks a foreground color/modifier for one character based on whether it
/// was typed correctly, incorrectly, or not yet typed.
///
/// Zen mode intentionally desaturates everything to the theme's muted shades
/// (with correct chars in `neutral`) so the screen stays calm and the user
/// isn't distracted by score-shaped feedback.
fn style_for_char(state: CharState, is_zen_mode: bool, theme: &ThemePalette) -> Style {
    if is_zen_mode {
        return match state {
            CharState::Correct => Style::default().fg(theme.neutral),
            CharState::Incorrect => Style::default()
                .fg(theme.pending)
                .add_modifier(Modifier::UNDERLINED),
            CharState::Pending => Style::default().fg(theme.pending),
        };
    }
    match state {
        CharState::Correct => Style::default().fg(theme.correct),
        CharState::Incorrect => Style::default()
            .fg(theme.incorrect)
            .add_modifier(Modifier::BOLD),
        CharState::Pending => Style::default().fg(theme.pending),
    }
}

/// Tiny hint strip at the bottom of the screen. No help hint: `?` is a
/// character you may need to type here.
fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    super::render_footer(
        f,
        app,
        area,
        &[
            ("ctrl+r / F5 restart", super::key(KeyCode::F(5))),
            ("esc finish", super::key(KeyCode::Esc)),
            (
                "ctrl+c quit",
                Some((KeyCode::Char('c'), KeyModifiers::CONTROL)),
            ),
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Word;
    use crate::theme::builtin;
    use ratatui::{backend::TestBackend, Terminal};

    /// The cell the cursor is drawn on (accent + underline), if on screen.
    fn cursor_cell(app: &App, width: u16, height: u16) -> Option<(u16, u16)> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| render(f, app)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .find(|&(x, y)| {
                let cell = &buffer[(x, y)];
                cell.fg == app.theme.accent && cell.modifier.contains(Modifier::UNDERLINED)
            })
    }

    /// Far into a long run the words area scrolls so the cursor's line stays
    /// on screen (it used to run off the bottom of the box).
    #[test]
    fn cursor_stays_visible_deep_into_a_long_run() {
        let mut app = App::new(None, builtin::DARK, Mode::Time(15));
        app.start_game(Mode::Words(400)).unwrap();
        app.words = (0..400).map(|_| Word::new("word".into())).collect();
        for word in &mut app.words[..350] {
            word.typed = word.text.clone();
        }
        app.current_word = 350;
        let (_, y) = cursor_cell(&app, 80, 24).expect("cursor drawn");
        assert!((6..23).contains(&y), "cursor row {y} outside the words box");
    }

    /// Wide (CJK) characters take two cells and a word longer than the box
    /// is broken across lines; the layout counts both, so the cursor still
    /// stays inside the box and no line spills past its border.
    #[test]
    fn cursor_stays_visible_with_wide_and_overlong_words() {
        let mut app = App::new(None, builtin::DARK, Mode::Time(15));
        app.start_game(Mode::Words(400)).unwrap();
        app.words = (0..400)
            .map(|i| {
                let text = if i % 50 == 7 {
                    "x".repeat(200)
                } else {
                    "漢字漢字".into()
                };
                Word::new(text)
            })
            .collect();
        for word in &mut app.words[..260] {
            word.typed = word.text.clone();
        }
        app.current_word = 260;
        let (_, y) = cursor_cell(&app, 80, 24).expect("cursor drawn");
        assert!((6..23).contains(&y), "cursor row {y} outside the words box");

        // Cursor in the middle of an over-long word, on its second row.
        app.current_word = 257;
        app.words[257].typed = "x".repeat(100);
        let (_, y) = cursor_cell(&app, 80, 24).expect("cursor drawn in a long word");
        assert!((6..23).contains(&y), "cursor row {y} outside the words box");
    }
}
