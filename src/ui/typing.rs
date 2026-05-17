use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
};

use crate::{
    app::{App, Mode},
    game::{get_char_states, CharState},
};

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(6),
        Constraint::Length(3),
    ])
    .split(area);

    render_header(f, app, layout[0]);
    render_progress(f, app, layout[1]);
    render_words(f, app, layout[2]);
    render_footer(f, layout[3]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let zen = matches!(app.mode, Mode::Zen);

    let timer_text = if let Some(remaining) = app.time_remaining() {
        format!("{:>3}s", remaining.as_secs())
    } else {
        format!("{:>5.1}s", app.elapsed().as_secs_f64())
    };

    let header = if zen {
        Line::from(vec![
            Span::styled(" zen ", Style::default().fg(Color::DarkGray)),
            Span::styled(" · esc to finish", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" wpm ",  Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:>3.0}", app.wpm()), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("   "),
            Span::styled("acc ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:>5.1}%", app.accuracy()), Style::default().fg(Color::Green)),
            Span::raw("   "),
            Span::styled("time ", Style::default().fg(Color::DarkGray)),
            Span::styled(timer_text, Style::default().fg(Color::Cyan)),
            Span::raw("   "),
            Span::styled(format!("[{}]", app.mode.label()), Style::default().fg(Color::Magenta)),
        ])
    };

    let p = Paragraph::new(header)
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(p, area);
}

fn render_progress(f: &mut Frame, app: &App, area: Rect) {
    if let Some((done, total)) = app.progress() {
        let ratio = if total == 0 { 0.0 } else { done as f64 / total as f64 };
        let gauge = Gauge::default()
            .block(Block::default())
            .gauge_style(Style::default().fg(Color::Cyan))
            .ratio(ratio.min(1.0))
            .label(format!("{} / {}", done, total));
        f.render_widget(gauge, area);
    } else if let Mode::Time(total) = app.mode {
        let elapsed = app.elapsed().as_secs_f64();
        let ratio = (elapsed / total as f64).min(1.0);
        let gauge = Gauge::default()
            .block(Block::default())
            .gauge_style(Style::default().fg(Color::Cyan))
            .ratio(ratio)
            .label(format!("{:.0}s / {}s", elapsed, total));
        f.render_widget(gauge, area);
    }
}

fn render_words(f: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = vec![];
    let mut current_line: Vec<Span> = vec![];
    let max_width = area.width.saturating_sub(4) as usize;
    let mut current_width = 0usize;
    let cursor_visible = app.tick_count % 10 < 5;
    let zen = matches!(app.mode, Mode::Zen);

    for (wi, word) in app.words.iter().enumerate() {
        let states = get_char_states(&word.text, &word.typed);
        let typed_count = word.typed.chars().count();

        let mut word_spans: Vec<Span> = vec![];
        for (ci, (ch, state)) in states.iter().enumerate() {
            let is_cursor = wi == app.current_word && ci == typed_count && cursor_visible;
            let style = if zen {
                if state == &CharState::Correct {
                    Style::default().fg(Color::Gray)
                } else if state == &CharState::Incorrect || state == &CharState::Extra {
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED)
                } else {
                    Style::default().fg(Color::DarkGray)
                }
            } else {
                match state {
                    CharState::Correct => Style::default().fg(Color::Green),
                    CharState::Incorrect => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    CharState::Pending => Style::default().fg(Color::DarkGray),
                    CharState::Extra => Style::default().fg(Color::Red).add_modifier(Modifier::UNDERLINED),
                }
            };
            let style = if is_cursor {
                style.add_modifier(Modifier::REVERSED)
            } else {
                style
            };
            word_spans.push(Span::styled(ch.to_string(), style));
        }

        // Cursor at end of typed input when typed >= chars in target
        if wi == app.current_word && typed_count >= word.text.chars().count() && cursor_visible {
            // already handled via Extra cursor coloring if any extras
            if word.typed.chars().count() == word.text.chars().count() {
                // append a phantom cursor block
                word_spans.push(Span::styled(
                    "▏",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ));
            }
        }

        let word_len: usize = word.text.chars().count().max(word.typed.chars().count());
        let space_len = if wi < app.words.len() - 1 { 1 } else { 0 };
        if current_width + word_len + space_len > max_width && !current_line.is_empty() {
            lines.push(Line::from(std::mem::take(&mut current_line)));
            current_width = 0;
        }
        current_line.extend(word_spans);
        current_width += word_len;
        if space_len > 0 {
            current_line.push(Span::raw(" "));
            current_width += 1;
        }
    }
    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }

    let para = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(" typerush ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        );
    f.render_widget(para, area);
}

fn render_footer(f: &mut Frame, area: Rect) {
    let footer = Paragraph::new(
        "  ctrl+r restart  ·  esc menu  ·  ctrl+c quit  ·  ? help",
    )
    .style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, area);
}
