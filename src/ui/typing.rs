//! The typing screen — header (live stats), progress bar, words area, footer.
//!
//! The cursor is **steady** (no blink) to avoid layout shifts. Both cursor
//! positions render the same way:
//! - on a character → underline in `theme.accent`
//! - past the last character → an underlined trailing space in `theme.accent`
//!
//! Keeping the cursor a single visual style (rather than a "block fill" on
//! characters + "underline" on spaces) avoids the visual jolt of switching
//! styles as the cursor crosses word boundaries.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Gauge, Paragraph, Wrap},
};

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
    if app.is_paused() {
        render_pause_overlay(f, app, area);
    }
}

/// Small centered modal shown while the session is paused (v0.5.0). Drawn on
/// top of the frozen typing screen so the user keeps their visual context.
fn render_pause_overlay(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let width = 40.min(area.width);
    let height = 5.min(area.height);
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    f.render_widget(Clear, popup);
    let lines = vec![
        Line::from(Span::styled(
            "  ⏸ paused",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            "  ctrl+p resume  ·  esc end session",
            Style::default().fg(theme.neutral),
        )),
    ];
    let modal = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent))
            .title(" paused "),
    );
    f.render_widget(modal, popup);
}

/// Renders the live WPM / accuracy / time / mode strip at the top of the screen.
/// In zen mode this is intentionally minimal (no numbers — just a label).
fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let is_zen_mode = matches!(app.mode, Mode::Zen);
    let theme = &app.theme;

    let timer_text = if app.is_paused() {
        "  ⏸".to_string()
    } else if let Some(remaining) = app.time_remaining() {
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
                format!("[{}]", app.mode.label()),
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
    // Gauge label sits in the middle of the bar, often straddling the
    // boundary between the filled (bg = accent) and unfilled (bg = theme bg)
    // portions. `secondary + BOLD` gives high contrast on both halves for
    // every built-in theme without needing a separate "on-accent" slot.
    let label_style = Style::default()
        .fg(app.theme.secondary)
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
/// Word-wraps manually (rather than relying on `Paragraph::wrap`) so we keep
/// full control of where line breaks happen — important because each character
/// has its own style.
///
/// The cursor never blinks: it is rendered as an underlined character (or
/// underlined trailing space when past the end of the current word) using
/// `theme.accent`. Keeping the cursor steady avoids the horizontal "jitter"
/// that a phantom blinking character would cause.
fn render_words(f: &mut Frame, app: &App, area: Rect) {
    let mut wrapped_lines: Vec<Line> = vec![];
    let mut current_line: Vec<Span> = vec![];
    // -4 to account for the surrounding border (1 char each side + padding).
    let max_line_width = area.width.saturating_sub(4) as usize;
    let mut current_line_width = 0usize;
    let is_zen_mode = matches!(app.mode, Mode::Zen);
    let last_word_index = app.words.len().saturating_sub(1);
    let cursor_style = Style::default()
        .fg(app.theme.accent)
        .add_modifier(Modifier::UNDERLINED);

    for (word_index, word) in app.words.iter().enumerate() {
        let char_states = get_char_states(&word.text, &word.typed);
        let typed_char_count = word.typed.chars().count();
        let is_active_word = word_index == app.current_word;
        // True when the user has typed at least as many characters as the word's
        // states — i.e. the cursor sits past the final character and any extras.
        let cursor_past_word_end = is_active_word && typed_char_count >= char_states.len();

        // 1. Render every character (target + extras) with its state-driven style.
        //    If the cursor is on this char, overlay it with the cursor style.
        let mut word_spans: Vec<Span> = Vec::with_capacity(char_states.len() + 1);
        for (char_index, (ch, state)) in char_states.iter().enumerate() {
            let cursor_on_this_char = is_active_word && char_index == typed_char_count;
            let base_style = style_for_char(*state, is_zen_mode, &app.theme);
            let style = if cursor_on_this_char {
                cursor_style
            } else {
                base_style
            };
            word_spans.push(Span::styled(ch.to_string(), style));
        }

        // 2. Decide whether (and how) to render the trailing space.
        //    A trailing space goes between every pair of words. It also doubles
        //    as the cursor "rest position" when the user has finished a word
        //    and is about to press space.
        let has_trailing_space = word_index < last_word_index;
        let space_style = if cursor_past_word_end {
            cursor_style
        } else {
            Style::default()
        };

        let trailing_chars: usize = if has_trailing_space {
            word_spans.push(Span::styled(" ", space_style));
            1
        } else if cursor_past_word_end {
            // Last word edge case — still reserve one space so the cursor has
            // something to underline, but only when the cursor is actually here.
            word_spans.push(Span::styled(" ", space_style));
            1
        } else {
            0
        };

        // 3. Word-wrap: if this word + space won't fit on the current line,
        //    flush and start a new one.
        let visible_word_width = char_states.len();
        let total_word_width = visible_word_width + trailing_chars;

        if current_line_width + total_word_width > max_line_width && !current_line.is_empty() {
            wrapped_lines.push(Line::from(std::mem::take(&mut current_line)));
            current_line_width = 0;
        }
        current_line.extend(word_spans);
        current_line_width += total_word_width;
    }

    if !current_line.is_empty() {
        wrapped_lines.push(Line::from(current_line));
    }

    let words_paragraph = Paragraph::new(wrapped_lines)
        .wrap(Wrap { trim: false })
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
/// was typed correctly, incorrectly, not yet typed, or typed as an extra.
///
/// Zen mode intentionally desaturates everything to the theme's muted shades
/// (with correct chars in `neutral`) so the screen stays calm and the user
/// isn't distracted by score-shaped feedback.
fn style_for_char(state: CharState, is_zen_mode: bool, theme: &ThemePalette) -> Style {
    if is_zen_mode {
        return match state {
            CharState::Correct => Style::default().fg(theme.neutral),
            CharState::Incorrect | CharState::Extra => Style::default()
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
        CharState::Extra => Style::default()
            .fg(theme.extra)
            .add_modifier(Modifier::UNDERLINED),
    }
}

/// Tiny hint strip at the bottom of the screen.
fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let footer = Paragraph::new("  ctrl+p pause  ·  ctrl+r restart  ·  esc menu  ·  ctrl+c quit")
        .style(Style::default().fg(app.theme.pending));
    f.render_widget(footer, area);
}
