//! TypeRush — terminal typing trainer.
//!
//! Entry point. Responsibilities:
//!   1. Parse CLI args via `clap`.
//!   2. Put the terminal into raw mode + alternate screen.
//!   3. Run the main event loop:
//!        - draw one frame
//!        - read key events (with a 100ms timeout)
//!        - call `app.tick()` every 100ms
//!        - save a session record the moment we land on the Results screen
//!   4. Restore the terminal on exit (and on panic, via a hook).

mod app;
mod config;
mod game;
mod state;
mod storage;
mod theme;
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
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::{App, MenuAction, Mode, Screen};
use crate::config::load::CliOverrides;
use crate::storage::SessionRecord;
use crate::theme::builtin;
use crate::words::WordPool;

/// Command-line interface. Run with no args to open the interactive menu; pass
/// any of the mode flags to skip the menu and jump straight into a session.
#[derive(Parser, Debug)]
#[command(
    name = "typerush",
    version,
    about = "In your terminal — a fast WPM typing trainer",
    long_about = None,
)]
struct Cli {
    /// Custom text file to use as the word source.
    #[arg(short, long)]
    file: Option<String>,

    /// Start directly in time mode for N seconds (15/30/60/120).
    #[arg(long)]
    time: Option<u64>,

    /// Start directly in words mode for N words.
    #[arg(long)]
    words: Option<usize>,

    /// Skip the menu and start a quote session.
    #[arg(long)]
    quote: bool,

    /// Skip the menu and start a code session: rust|python|js|go|java|sql|shell.
    #[arg(long)]
    code: Option<String>,

    /// Skip the menu and start zen mode.
    #[arg(long)]
    zen: bool,

    /// Skip the menu and start a symbols-drill session of N tokens.
    #[arg(long)]
    symbols: Option<usize>,

    /// Use the larger 10,000-word pool instead of the default ~1,000.
    #[arg(long)]
    big: bool,

    /// Sprinkle punctuation marks onto random words during Time / Words modes.
    #[arg(long)]
    punctuation: bool,

    /// Mix random number literals (1–4 digits) into Time / Words modes.
    #[arg(long)]
    numbers: bool,

    /// One-shot theme override: dark | light | monokai | dracula.
    /// Takes precedence over the `theme` setting in `~/.typerush/config.toml`.
    #[arg(long)]
    theme: Option<String>,

    /// Print available built-in theme names and exit.
    #[arg(long)]
    list_themes: bool,

    /// Print every snippet name discovered under `~/.typerush/snippets/` and exit.
    #[arg(long)]
    list_snippets: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.list_themes {
        print_themes_and_exit();
    }
    if cli.list_snippets {
        print_snippets_and_exit();
    }
    install_panic_hook();
    let mut terminal = setup_terminal()?;
    let run_result = run_app(&mut terminal, cli);
    restore_terminal(&mut terminal)?;
    run_result
}

/// Print every built-in theme name on its own line and exit with status 0.
/// Intended for shell completion / discoverability.
fn print_themes_and_exit() -> ! {
    for theme_name in builtin::names() {
        println!("{}", theme_name);
    }
    std::process::exit(0);
}

/// Print every snippet discovered under `~/.typerush/snippets/` and exit.
/// Each line is `<name>\t<path>` so shell scripts can split on tab.
fn print_snippets_and_exit() -> ! {
    let discovered = words::snippets::discover_snippets();
    if discovered.is_empty() {
        eprintln!(
            "no snippets found. Drop .txt files into {} to populate this list.",
            words::snippets::snippets_dir().display()
        );
    }
    for snippet in &discovered {
        println!("{}\t{}", snippet.name, snippet.path.display());
    }
    std::process::exit(0);
}

/// Type alias to keep function signatures readable.
type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Switch the terminal into raw mode + alternate screen so we can take over
/// the display without scrambling the user's scrollback history.
fn setup_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(out);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Reverse of `setup_terminal` — restore cooked mode and exit the alt screen.
fn restore_terminal(terminal: &mut Tui) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// If we panic mid-render the user's terminal will be left in a broken state
/// (raw mode, no cursor, alt screen). This hook resets those settings before
/// the default panic handler prints its message.
fn install_panic_hook() {
    let original = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original(info);
    }));
}

/// The main event/render loop.
///
/// `tick_rate` is 100ms — small enough that the WPM display feels live, large
/// enough that we're not busy-looping. `event::poll` blocks for the remainder
/// of the tick interval so keystrokes are still handled instantly.
fn run_app(terminal: &mut Tui, cli: Cli) -> Result<()> {
    let overrides = CliOverrides {
        theme: cli.theme.as_deref(),
        word_pool: if cli.big {
            Some(WordPool::Extended)
        } else {
            None
        },
        punctuation: if cli.punctuation { Some(true) } else { None },
        numbers: if cli.numbers { Some(true) } else { None },
    };
    let (resolved, warnings) = config::load::load_or_default_with(overrides);
    let mut app = App::new(
        cli.file.clone(),
        resolved.palette,
        resolved.default_mode,
        resolved.word_pool,
        resolved.word_decor,
    );
    // Surface the first config warning (if any) via the existing error modal.
    // We only show one — chaining them would force the user to dismiss N
    // popups before reaching the menu.
    if let Some(first_warning) = warnings.into_iter().next() {
        app.error_message = Some(first_warning);
    }
    apply_cli_autostart(&mut app, &cli)?;

    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();
    let mut last_screen = app.screen;
    let mut session_saved_for_this_results_screen = false;

    loop {
        terminal.draw(|frame| ui::render(frame, &app))?;

        // Block until either a key event arrives or the tick interval elapses.
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // Ignore key-release events on platforms that emit them.
                if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
                    continue;
                }
                // If a modal error is showing, any key dismisses it.
                if app.error_message.is_some() {
                    app.error_message = None;
                    continue;
                }
                handle_key(&mut app, key.code, key.modifiers);
            }
        }

        // Run app.tick() at a regular cadence regardless of how often we
        // wake up from event::poll.
        if last_tick.elapsed() >= tick_rate {
            app.tick();
            last_tick = Instant::now();
        }

        // Save the session exactly once on the transition INTO the Results screen.
        if app.screen == Screen::Results
            && last_screen != Screen::Results
            && !session_saved_for_this_results_screen
        {
            save_current_session(&mut app);
            session_saved_for_this_results_screen = true;
        }
        if app.screen != Screen::Results {
            session_saved_for_this_results_screen = false;
        }

        // Populate the session cache once when entering the Stats or Results
        // screen so the render path never reads stats.json on every frame.
        // (On Results entry this runs *after* the save above, so the cache
        // includes the session that was just recorded.)
        if (app.screen == Screen::Stats || app.screen == Screen::Results)
            && last_screen != app.screen
        {
            app.stats_cache = storage::load_sessions().ok();
        }

        last_screen = app.screen;

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

/// If the user passed a mode flag (`--time`, `--words`, `--quote`, `--code`,
/// `--zen`, `--symbols`, `--file`) skip the menu and start that mode immediately.
fn apply_cli_autostart(app: &mut App, cli: &Cli) -> Result<()> {
    if let Some(seconds) = cli.time {
        app.start_game(Mode::Time(seconds))?;
    } else if let Some(word_count) = cli.words {
        app.start_game(Mode::Words(word_count))?;
    } else if cli.quote {
        app.start_game(Mode::Quote)?;
    } else if let Some(lang_string) = cli.code.as_deref() {
        let lang = match lang_string.to_lowercase().as_str() {
            "rust" | "rs" => words::CodeLang::Rust,
            "python" | "py" => words::CodeLang::Python,
            "js" | "javascript" => words::CodeLang::JavaScript,
            "go" | "golang" => words::CodeLang::Go,
            "java" => words::CodeLang::Java,
            "sql" => words::CodeLang::Sql,
            "shell" | "sh" | "bash" => words::CodeLang::Shell,
            other => return Err(anyhow::anyhow!("unknown code lang: {other}")),
        };
        app.start_game(Mode::Code(lang))?;
    } else if cli.zen {
        app.start_game(Mode::Zen)?;
    } else if let Some(count) = cli.symbols {
        app.start_game(Mode::Symbols(count))?;
    } else if let Some(path) = cli.file.as_deref() {
        // Remember the explicit `--file` path so the menu's Custom row stays
        // useful on the next launch.
        app.remember_custom_file(path);
        app.start_game(Mode::Custom)?;
    }
    Ok(())
}

/// Top-level keymap dispatcher: handles global shortcuts first (Ctrl+C,
/// help overlay), then delegates to a per-screen handler.
fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    // Global: Ctrl+C always quits.
    if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }
    // '?' toggles the help overlay everywhere except during typing (where
    // '?' is a valid character to type).
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

/// Show or hide the help overlay. Remembers the previous screen so we can
/// restore it when the overlay closes.
fn toggle_help(app: &mut App) {
    if app.screen == Screen::Help {
        app.screen = app.previous_screen;
    } else {
        app.previous_screen = app.screen;
        app.screen = Screen::Help;
    }
}

/// Keymap for the main menu: arrow keys / j-k to navigate, Enter to act.
fn handle_menu_key(app: &mut App, code: KeyCode, _mods: KeyModifiers) {
    match code {
        KeyCode::Up | KeyCode::Char('k') => move_menu_cursor(app, -1),
        KeyCode::Down | KeyCode::Char('j') => move_menu_cursor(app, 1),
        KeyCode::Enter => {
            let item = &app.menu[app.menu_index];
            let action = item.action;
            let mode = item.mode;
            let custom_path = item.custom_path.clone();
            match action {
                MenuAction::Start => {
                    if let Some(m) = mode {
                        if let Err(e) = app.start_game(m) {
                            app.error_message = Some(e.to_string());
                        }
                    }
                }
                MenuAction::StartCustom => {
                    // Per-snippet rows carry their own path; the bare "Custom"
                    // row uses whatever `app.custom_file` holds (set by
                    // `--file` or by a previous session via state.json).
                    if let Some(path) = custom_path {
                        app.remember_custom_file(&path);
                    }
                    if let Err(e) = app.start_game(Mode::Custom) {
                        app.error_message = Some(e.to_string());
                    }
                }
                MenuAction::ShowStats => app.screen = Screen::Stats,
                MenuAction::Quit => app.should_quit = true,
                MenuAction::Separator => {}
            }
        }
        KeyCode::Tab => app.screen = Screen::Stats,
        KeyCode::Char('q') => app.should_quit = true,
        _ => {}
    }
}

/// Move the menu highlight by `step` (positive = down) and skip any
/// separator rows that fall in the way. Wraps around at the ends. Safe even
/// if every row is a separator (caller terminates after one full lap).
fn move_menu_cursor(app: &mut App, step: i32) {
    let len = app.menu.len();
    if len == 0 {
        return;
    }
    let mut idx = app.menu_index as i32;
    for _ in 0..len as i32 {
        idx += step;
        if idx < 0 {
            idx += len as i32;
        }
        if idx >= len as i32 {
            idx -= len as i32;
        }
        if !matches!(app.menu[idx as usize].action, MenuAction::Separator) {
            app.menu_index = idx as usize;
            return;
        }
    }
}

/// Keymap for the typing screen. Note that '?' is **not** a help shortcut
/// here — the user may legitimately need to type it.
pub(crate) fn handle_typing_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
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
            // Ctrl+Backspace (or Alt+Backspace on some terms) = delete whole word.
            let delete_whole_word =
                mods.contains(KeyModifiers::CONTROL) || mods.contains(KeyModifiers::ALT);
            app.handle_backspace(delete_whole_word);
        }
        // Many terminals (notably Windows Terminal, iTerm2, most Linux
        // emulators) send Ctrl+Backspace as a literal `^H` byte — crossterm
        // surfaces this as `Char('h') + CTRL`, NOT as `Backspace + CTRL`,
        // so the dedicated Backspace branch above never fires. Ctrl+W is
        // the Unix convention for "kill word" and is included for the same
        // reason — common muscle memory shouldn't fall on the floor.
        KeyCode::Char('h') | KeyCode::Char('w') if mods.contains(KeyModifiers::CONTROL) => {
            app.handle_backspace(true);
        }
        KeyCode::Char(typed_char) => {
            // Ignore other control chords like Ctrl+A — we never want those
            // to be counted as typed characters.
            if mods.contains(KeyModifiers::CONTROL) {
                return;
            }
            app.handle_char(typed_char);
        }
        _ => {}
    }
}

/// Keymap for the post-session results screen.
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

/// Keymap for the historical stats screen.
fn handle_stats_key(app: &mut App, code: KeyCode, _mods: KeyModifiers) {
    match code {
        KeyCode::Char('m') | KeyCode::Esc | KeyCode::Tab => app.screen = Screen::Menu,
        KeyCode::Char('q') => app.should_quit = true,
        _ => {}
    }
}

/// Persist the just-finished session to `~/.typerush/stats.json`.
///
/// Sessions are skipped when:
///   - the mode is Zen (no-stats philosophy)
///   - the session is shorter than 1 second
///   - the user hasn't typed a single character
///
/// File I/O errors are intentionally swallowed — losing a stat row is never a
/// good reason to crash on the user.
fn save_current_session(app: &mut App) {
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
        key_hits: app
            .key_hits
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect(),
        key_misses: app
            .key_misses
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect(),
    };
    // Persist to stats.json (save_session also updates aggregate.json on disk).
    let _ = storage::save_session(&record);
    // Keep the in-memory aggregate current so the Stats panel stays accurate
    // without an extra disk read.
    storage::apply_session_to_aggregate(&mut app.aggregate, &record);
    // Invalidate the session cache so the next Stats screen visit reloads fresh data.
    app.stats_cache = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Screen, Word};
    use crate::config::load::DefaultMode;
    use crate::theme::ThemePalette;

    fn make_typing_app() -> App {
        let mut app = App::new(
            None,
            ThemePalette::default(),
            DefaultMode::Time(15),
            Default::default(),
            Default::default(),
        );
        app.screen = Screen::Typing;
        app.mode = Mode::Words(2);
        app.words = vec![Word::new("hello".into()), Word::new("world".into())];
        app
    }

    /// Regression: most terminals send Ctrl+Backspace as a literal ^H byte,
    /// which crossterm reports as Char('h') + CTRL. Before this fix the
    /// keymap dropped that into the "ignore control chords" branch and the
    /// user saw nothing happen.
    #[test]
    fn ctrl_h_deletes_current_word() {
        let mut app = make_typing_app();
        for ch in "hel".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.words[0].typed, "hel");

        handle_typing_key(&mut app, KeyCode::Char('h'), KeyModifiers::CONTROL);
        assert_eq!(app.words[0].typed, "");
    }

    /// Ctrl+W is the Unix convention for "kill word" — common muscle memory.
    #[test]
    fn ctrl_w_deletes_current_word() {
        let mut app = make_typing_app();
        for ch in "hel".chars() {
            app.handle_char(ch);
        }
        handle_typing_key(&mut app, KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(app.words[0].typed, "");
    }

    /// The original Backspace+CTRL path still works on terminals that DO
    /// surface Ctrl+Backspace as a real Backspace keycode.
    #[test]
    fn ctrl_backspace_keycode_still_deletes_word() {
        let mut app = make_typing_app();
        for ch in "hel".chars() {
            app.handle_char(ch);
        }
        handle_typing_key(&mut app, KeyCode::Backspace, KeyModifiers::CONTROL);
        assert_eq!(app.words[0].typed, "");
    }

    /// Plain Backspace must still delete a single character — not a whole
    /// word. Guards against accidentally widening the new Ctrl+H branch.
    #[test]
    fn plain_backspace_deletes_one_char() {
        let mut app = make_typing_app();
        for ch in "hel".chars() {
            app.handle_char(ch);
        }
        handle_typing_key(&mut app, KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(app.words[0].typed, "he");
    }

    /// Other Ctrl-chords (Ctrl+A, Ctrl+Z, …) must still be ignored — not
    /// counted as typed characters.
    #[test]
    fn other_ctrl_chords_are_ignored() {
        let mut app = make_typing_app();
        for ch in "hel".chars() {
            app.handle_char(ch);
        }
        handle_typing_key(&mut app, KeyCode::Char('a'), KeyModifiers::CONTROL);
        handle_typing_key(&mut app, KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert_eq!(app.words[0].typed, "hel");
    }

    // ── v0.4.0: menu navigation with section separators ─────────────────────

    fn make_menu_app() -> App {
        let mut app = App::new(
            None,
            ThemePalette::default(),
            DefaultMode::Time(15),
            Default::default(),
            Default::default(),
        );
        app.screen = Screen::Menu;
        app
    }

    /// Arrow-down navigation must skip past separator rows so the user never
    /// "lands" on a decorative section header.
    #[test]
    fn arrow_down_skips_separators() {
        let mut app = make_menu_app();
        // Find the first non-separator row.
        let first_non_sep = app
            .menu
            .iter()
            .position(|m| !matches!(m.action, MenuAction::Separator))
            .unwrap();
        app.menu_index = first_non_sep;
        // Move down once and ensure the new row isn't a separator.
        handle_menu_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
        assert!(
            !matches!(app.menu[app.menu_index].action, MenuAction::Separator),
            "landed on a separator at index {}: {:?}",
            app.menu_index,
            app.menu[app.menu_index].label
        );
    }

    /// Arrow-up wraps around past the trailing separators back to a real row.
    #[test]
    fn arrow_up_skips_separators_and_wraps() {
        let mut app = make_menu_app();
        // Start at index 0 — likely a separator. Pressing Up should wrap to
        // the very last selectable row.
        app.menu_index = 0;
        handle_menu_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
        assert!(!matches!(
            app.menu[app.menu_index].action,
            MenuAction::Separator
        ));
    }

    /// Pressing Enter on a separator is a no-op — never starts a game.
    #[test]
    fn enter_on_separator_does_nothing() {
        let mut app = make_menu_app();
        let sep_idx = app
            .menu
            .iter()
            .position(|m| matches!(m.action, MenuAction::Separator))
            .expect("menu must contain a separator");
        app.menu_index = sep_idx;
        handle_menu_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        // Still on the menu, no error fired.
        assert_eq!(app.screen, Screen::Menu);
        assert!(app.error_message.is_none());
    }

    /// Hitting Enter on the Custom row with no remembered file surfaces a
    /// friendly error instead of crashing.
    #[test]
    fn custom_row_without_file_shows_error() {
        let mut app = make_menu_app();
        let idx = app
            .menu
            .iter()
            .position(|m| matches!(m.action, MenuAction::StartCustom) && m.custom_path.is_none())
            .expect("custom row missing");
        app.menu_index = idx;
        // Ensure no file is remembered.
        app.custom_file = None;
        handle_menu_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(app.error_message.is_some());
        // Still on the menu — didn't transition to Typing.
        assert_eq!(app.screen, Screen::Menu);
    }
}
