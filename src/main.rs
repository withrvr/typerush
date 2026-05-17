mod app;
mod game;
mod storage;
mod ui;
mod words;

use std::{
    io::{stdout, Stdout},
    panic,
    time::{Duration, Instant},
};

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::{App, MenuAction, Mode, Screen};
use crate::storage::SessionRecord;

#[derive(Parser, Debug)]
#[command(
    name = "typerush",
    version,
    about = "In your terminal — a fast WPM typing trainer",
    long_about = None,
)]
struct Cli {
    /// Custom text file to use as the word source
    #[arg(short, long)]
    file: Option<String>,

    /// Start directly in time mode for N seconds (15/30/60/120)
    #[arg(long)]
    time: Option<u64>,

    /// Start directly in words mode for N words
    #[arg(long)]
    words: Option<usize>,

    /// Skip the menu and start a quote session
    #[arg(long)]
    quote: bool,

    /// Skip the menu and start a code session: rust|python|js
    #[arg(long)]
    code: Option<String>,

    /// Skip the menu and start zen mode
    #[arg(long)]
    zen: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    install_panic_hook();
    let mut terminal = setup_terminal()?;
    let res = run_app(&mut terminal, cli);
    restore_terminal(&mut terminal)?;
    res
}

type Tui = Terminal<CrosstermBackend<Stdout>>;

fn setup_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(out);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Tui) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

fn install_panic_hook() {
    let original = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original(info);
    }));
}

fn run_app(terminal: &mut Tui, cli: Cli) -> Result<()> {
    let mut app = App::new(cli.file.clone());

    // Auto-start from CLI args
    if let Some(secs) = cli.time {
        app.start_game(Mode::Time(secs))?;
    } else if let Some(n) = cli.words {
        app.start_game(Mode::Words(n))?;
    } else if cli.quote {
        app.start_game(Mode::Quote)?;
    } else if let Some(lang) = cli.code.as_deref() {
        let lang = match lang.to_lowercase().as_str() {
            "rust" | "rs" => words::CodeLang::Rust,
            "python" | "py" => words::CodeLang::Python,
            "js" | "javascript" => words::CodeLang::JavaScript,
            _ => return Err(anyhow::anyhow!("unknown code lang: {lang}")),
        };
        app.start_game(Mode::Code(lang))?;
    } else if cli.zen {
        app.start_game(Mode::Zen)?;
    } else if cli.file.is_some() {
        app.start_game(Mode::Custom)?;
    }

    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();
    let mut last_screen = app.screen;
    let mut results_saved = false;

    loop {
        terminal.draw(|f| ui::render(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
                    continue;
                }
                if app.error_message.is_some() {
                    app.error_message = None;
                    continue;
                }
                handle_key(&mut app, key.code, key.modifiers);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.tick();
            last_tick = Instant::now();
        }

        // Save session exactly once when we land on Results.
        if app.screen == Screen::Results && last_screen != Screen::Results && !results_saved {
            save_current_session(&app);
            results_saved = true;
        }
        if app.screen != Screen::Results {
            results_saved = false;
        }
        last_screen = app.screen;

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    // Global: Ctrl+C
    if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }
    // Toggle help overlay
    if code == KeyCode::Char('?') && app.screen != Screen::Typing {
        toggle_help(app);
        return;
    }
    if app.screen == Screen::Help {
        if matches!(code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')) {
            app.screen = app.previous_screen;
        }
        return;
    }

    match app.screen {
        Screen::Menu => handle_menu_key(app, code, mods),
        Screen::Typing => handle_typing_key(app, code, mods),
        Screen::Results => handle_results_key(app, code, mods),
        Screen::Stats => handle_stats_key(app, code, mods),
        Screen::Help => {}
    }
}

fn toggle_help(app: &mut App) {
    if app.screen == Screen::Help {
        app.screen = app.previous_screen;
    } else {
        app.previous_screen = app.screen;
        app.screen = Screen::Help;
    }
}

fn handle_menu_key(app: &mut App, code: KeyCode, _mods: KeyModifiers) {
    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.menu_index == 0 {
                app.menu_index = app.menu.len() - 1;
            } else {
                app.menu_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.menu_index = (app.menu_index + 1) % app.menu.len();
        }
        KeyCode::Enter => {
            let item = &app.menu[app.menu_index];
            let action = item.action;
            let mode = item.mode;
            match action {
                MenuAction::Start => {
                    if let Some(m) = mode {
                        if let Err(e) = app.start_game(m) {
                            app.error_message = Some(e.to_string());
                        }
                    }
                }
                MenuAction::ShowStats => app.screen = Screen::Stats,
                MenuAction::Quit => app.should_quit = true,
            }
        }
        KeyCode::Tab => app.screen = Screen::Stats,
        KeyCode::Char('q') => app.should_quit = true,
        _ => {}
    }
}

fn handle_typing_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match code {
        KeyCode::Esc => {
            app.finish_game();
        }
        KeyCode::Char('r') if mods.contains(KeyModifiers::CONTROL) => {
            if let Err(e) = app.restart() {
                app.error_message = Some(e.to_string());
            }
        }
        KeyCode::Backspace => {
            app.handle_backspace(mods.contains(KeyModifiers::CONTROL) || mods.contains(KeyModifiers::ALT));
        }
        KeyCode::Char(c) => {
            // skip control chord chars (e.g. Ctrl+A) so they don't get typed
            if mods.contains(KeyModifiers::CONTROL) {
                return;
            }
            app.handle_char(c);
        }
        _ => {}
    }
}

fn handle_results_key(app: &mut App, code: KeyCode, _mods: KeyModifiers) {
    match code {
        KeyCode::Enter | KeyCode::Char('r') => {
            if let Err(e) = app.restart() {
                app.error_message = Some(e.to_string());
            }
        }
        KeyCode::Char('m') | KeyCode::Esc => app.screen = Screen::Menu,
        KeyCode::Tab | KeyCode::Char('s') => app.screen = Screen::Stats,
        KeyCode::Char('q') => app.should_quit = true,
        _ => {}
    }
}

fn handle_stats_key(app: &mut App, code: KeyCode, _mods: KeyModifiers) {
    match code {
        KeyCode::Char('m') | KeyCode::Esc | KeyCode::Tab => app.screen = Screen::Menu,
        KeyCode::Char('q') => app.should_quit = true,
        _ => {}
    }
}

fn save_current_session(app: &App) {
    // Skip zen mode — it's pressure-free, no tracking.
    if matches!(app.mode, Mode::Zen) {
        return;
    }
    let duration = app.elapsed().as_secs_f64();
    if duration < 1.0 || app.total_typed_chars == 0 {
        return;
    }
    let record = SessionRecord {
        wpm: app.wpm(),
        accuracy: app.accuracy(),
        mode: app.mode.label(),
        word_count: app.current_word,
        correct_chars: app.correct_chars,
        total_chars: app.total_typed_chars,
        duration_secs: duration,
        timestamp: chrono::Local::now(),
    };
    let _ = storage::save_session(&record);
}
