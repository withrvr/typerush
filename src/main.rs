//! TypeRush — terminal typing trainer.
//!
//! Entry point. Responsibilities:
//!   1. Parse CLI args via `clap`.
//!   2. Put the terminal into raw mode + alternate screen.
//!   3. Run the main event loop:
//!        - draw one frame
//!        - read key and mouse events (with a 100ms timeout)
//!        - call `app.tick()` every 100ms
//!        - save a session record the moment we land on the Results screen
//!   4. Restore the terminal on exit (and on panic, via a hook).

mod app;
mod config;
mod game;
mod state;
mod storage;
mod text;
mod theme;
mod ui;
mod words;

use std::{
    io::{stdout, Stdout},
    panic,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::Result;
use clap::Parser;
use ratatui::crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::{App, ClickAction, MenuAction, Mode, Screen};
use crate::config::load::{check_count, code_lang, CliOverrides, MAX_COUNT, MAX_SECONDS};
use crate::storage::SessionRecord;
use crate::theme::builtin;
use crate::words::{CodeLang, WordPool};

/// Value parser for run lengths: a whole number from 1 to `MAX` (see
/// `config::load::check_count`, which the config uses too).
fn count<T, const MAX: u16>(value: &str) -> Result<T, String>
where
    T: std::str::FromStr + PartialOrd + From<u16>,
{
    let count = value.parse::<T>().map_err(|_| "not a whole number")?;
    check_count(count, MAX)
}

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
    file: Option<PathBuf>,

    /// Start directly in time mode for N seconds (15/30/60/120).
    #[arg(long, value_parser = count::<u64, MAX_SECONDS>)]
    time: Option<u64>,

    /// Start directly in words mode for N words.
    #[arg(long, value_parser = count::<usize, MAX_COUNT>)]
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

    /// Skip the menu and start a programming-symbols drill of N tokens (25/50).
    #[arg(long, value_parser = count::<usize, MAX_COUNT>)]
    symbols: Option<usize>,

    /// Use the 10,000-word English pool (time and words modes).
    #[arg(long, overrides_with = "no_big")]
    big: bool,

    /// Use the common-word pool even if the config picks the 10k one.
    #[arg(long, overrides_with = "big")]
    no_big: bool,

    /// Add punctuation to random words (time and words modes).
    #[arg(long, overrides_with = "no_punctuation")]
    punctuation: bool,

    /// No punctuation, even if the config turns it on.
    #[arg(long, overrides_with = "punctuation")]
    no_punctuation: bool,

    /// Mix numbers in with random words (time and words modes).
    #[arg(long, overrides_with = "no_numbers")]
    numbers: bool,

    /// No numbers, even if the config turns them on.
    #[arg(long, overrides_with = "numbers")]
    no_numbers: bool,

    /// One-shot theme override: dark | light | monokai | dracula.
    /// Takes precedence over the `theme` setting in `~/.typerush/config.toml`.
    #[arg(long)]
    theme: Option<String>,

    /// Print available built-in theme names and exit.
    #[arg(long)]
    list_themes: bool,

    /// Print the snippets found in ~/.typerush/snippets/ and exit.
    #[arg(long)]
    list_snippets: bool,
}

impl Cli {
    /// The settings the command line overrides. A switch pair left alone
    /// (neither `--punctuation` nor `--no-punctuation`) keeps the config's
    /// value; when both are given, the last one wins.
    fn overrides(&self) -> CliOverrides<'_> {
        let switch = |on: bool, off: bool| match (on, off) {
            (true, _) => Some(true),
            (_, true) => Some(false),
            _ => None,
        };
        CliOverrides {
            theme: self.theme.as_deref(),
            word_pool: switch(self.big, self.no_big).map(|big| {
                if big {
                    WordPool::Extended
                } else {
                    WordPool::Common
                }
            }),
            punctuation: switch(self.punctuation, self.no_punctuation),
            numbers: switch(self.numbers, self.no_numbers),
        }
    }
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
    let restored = restore_terminal(&mut terminal);
    if let Err(error) = run_result {
        // The report Rust prints for an `Err` from `main`. File paths in it
        // are already made safe where the error was built; each line goes
        // through `printable` too, as a backstop.
        let report = format!("{error:?}");
        let lines: Vec<_> = report.lines().map(text::printable).collect();
        eprintln!("Error: {}", lines.join("\n"));
        // The run's error is the one that matters; still mention a failed
        // terminal restore rather than dropping it.
        if let Err(restore_error) = restored {
            eprintln!("(also: couldn't restore the terminal: {restore_error:#})");
        }
        std::process::exit(1);
    }
    restored
}

/// Print every built-in theme name on its own line and exit with status 0.
/// Intended for shell completion / discoverability.
fn print_themes_and_exit() -> ! {
    for theme_name in builtin::names() {
        println!("{}", theme_name);
    }
    std::process::exit(0);
}

/// Print each snippet in `~/.typerush/snippets/` as `name<TAB>path` and exit
/// with status 0. With none, say where to put them (on stderr, so scripts
/// reading stdout see an empty list).
fn print_snippets_and_exit() -> ! {
    let snippets = words::snippets::discover_snippets();
    if snippets.is_empty() {
        eprintln!(
            "no snippets yet: drop .txt files in {}",
            text::printable(&words::snippets::snippets_dir().display().to_string())
        );
    }
    // The path column tells same-named files apart (notes.txt / notes.TXT).
    for snippet in snippets {
        println!(
            "{}\t{}",
            text::printable(&snippet.name),
            // Kept a real path (only control and bidi characters replaced) so
            // scripts can open it.
            text::path_text(&snippet.path.display().to_string())
        );
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
///
/// Every step is attempted even if an earlier one fails, so the user gets
/// their normal screen back (and sees any error printed after this); the
/// first failure is returned.
fn restore_terminal(terminal: &mut Tui) -> Result<()> {
    let raw = disable_raw_mode();
    let screen = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let cursor = terminal.show_cursor();
    raw?;
    screen?;
    cursor?;
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
    let (resolved, warnings) = config::load::load_or_default_with(cli.overrides());
    let mut app = App::new(cli.file.clone(), resolved.palette, resolved.default_mode);
    app.set_word_source(resolved.word_pool, resolved.word_decor);
    let state_file = state::state_path();
    app.load_custom_sources(
        words::snippets::discover_snippets(),
        state::load_from_path(&state_file),
        Some(state_file),
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
    let mut session_saved = false;
    let stats_file = storage::stats_path();

    loop {
        terminal.draw(|frame| ui::render(frame, &app))?;

        // Block until either a key event arrives or the tick interval elapses.
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
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
                Event::Mouse(mouse) => handle_mouse(&mut app, mouse),
                _ => {}
            }
        }

        // Run app.tick() at a regular cadence regardless of how often we
        // wake up from event::poll.
        if last_tick.elapsed() >= tick_rate {
            app.tick();
            last_tick = Instant::now();
        }

        after_input(&mut app, last_screen, &mut session_saved, &stats_file);
        last_screen = app.screen;

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

/// If the user passed a mode flag (`--time`, `--words`, `--quote`, `--code`,
/// `--zen`, `--symbols`, `--file`) skip the menu and start that mode.
fn apply_cli_autostart(app: &mut App, cli: &Cli) -> Result<()> {
    if let Some(seconds) = cli.time {
        app.start_game(Mode::Time(seconds))?;
    } else if let Some(word_count) = cli.words {
        app.start_game(Mode::Words(word_count))?;
    } else if cli.quote {
        app.start_game(Mode::Quote)?;
    } else if let Some(lang_string) = cli.code.as_deref() {
        app.start_game(Mode::Code(code_lang_from_cli(lang_string)?))?;
    } else if cli.zen {
        app.start_game(Mode::Zen)?;
    } else if let Some(count) = cli.symbols {
        app.start_game(Mode::Symbols(count))?;
    } else if cli.file.is_some() {
        // `App::new` already holds the file; this starts and remembers it.
        app.start_custom(None)?;
    }
    Ok(())
}

/// `--code <name>`: the same names and aliases as `code_lang` in the config
/// (one table, `config::load::code_lang`), but an unknown name is an
/// error here rather than a fall-back-with-warning.
fn code_lang_from_cli(name: &str) -> Result<CodeLang> {
    code_lang(name).ok_or_else(|| {
        anyhow::anyhow!("unknown code lang: {name} (try rust, python, js, go, java, sql, shell)")
    })
}

/// Top-level keymap dispatcher: handles global shortcuts first (Ctrl+C,
/// help overlay), then delegates to a per-screen handler.
fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    // Global: Ctrl+C always quits.
    if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }
    // '?' (or F1) toggles the help overlay, except while typing ('?' is a
    // character to type, and help would cover the run). On Results only F1
    // does: a fast typist's last keys land there (see `handle_results_key`).
    let help_key = match code {
        KeyCode::F(1) => app.screen != Screen::Typing,
        KeyCode::Char('?') => !matches!(app.screen, Screen::Typing | Screen::Results),
        _ => false,
    };
    if help_key {
        toggle_help(app);
        return;
    }
    if app.screen == Screen::Help {
        if matches!(
            code,
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::F(1)
        ) {
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

/// Mouse support. Every click maps onto an existing keyboard action (see
/// `ClickAction`), so mouse and keyboard reach exactly the same things.
///
/// Actions fire on button *release*, and only when the release lands on the
/// same target the press started on — pressing on the wrong item and sliding
/// off cancels it (WCAG 2.5.2 pointer cancellation). Pressing on a menu option
/// only highlights it.
fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    let released = mouse.kind == MouseEventKind::Up(MouseButton::Left);
    // Modal error / help overlay: a click anywhere closes it, like any key.
    if app.error_message.is_some() {
        if released {
            app.error_message = None;
        }
        return;
    }
    if app.screen == Screen::Help {
        if released {
            toggle_help(app);
        }
        return;
    }

    let position = ratatui::layout::Position::new(mouse.column, mouse.row);
    let target = app
        .click_targets
        .borrow()
        .iter()
        .find(|(area, _)| area.contains(position))
        .map(|(_, action)| *action);

    match mouse.kind {
        MouseEventKind::ScrollUp if app.screen == Screen::Menu => app.menu_move_row(false),
        MouseEventKind::ScrollDown if app.screen == Screen::Menu => app.menu_move_row(true),
        MouseEventKind::ScrollUp if app.screen == Screen::Stats => {
            handle_key(app, KeyCode::Up, KeyModifiers::NONE)
        }
        MouseEventKind::ScrollDown if app.screen == Screen::Stats => {
            handle_key(app, KeyCode::Down, KeyModifiers::NONE)
        }
        MouseEventKind::Down(MouseButton::Left) => {
            app.pressed_target = target;
            if let Some(ClickAction::Menu(index)) = target {
                app.menu_index = index;
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            // Only a press and release on the same target is a click: dragging
            // from one option to another, or a stray release, does nothing.
            let pressed = app.pressed_target.take();
            if target.is_none() || pressed != target {
                return;
            }
            match target {
                Some(ClickAction::Menu(index)) => {
                    app.menu_index = index;
                    handle_key(app, KeyCode::Enter, KeyModifiers::NONE);
                }
                Some(ClickAction::Key(code, mods)) => handle_key(app, code, mods),
                None => {}
            }
        }
        _ => {}
    }
}

/// Keymap for the main menu: ↑/↓ (j/k) pick a category row, ←/→ (h/l) pick
/// an option within it, Enter (or Space) to act. `p` / `n` / `b` toggle the
/// word settings (punctuation, numbers, 10k pool) for time and words runs,
/// for this run of TypeRush; the config and CLI set where they start.
fn handle_menu_key(app: &mut App, code: KeyCode, _mods: KeyModifiers) {
    match code {
        KeyCode::Up | KeyCode::Char('k') => app.menu_move_row(false),
        KeyCode::Down | KeyCode::Char('j') => app.menu_move_row(true),
        KeyCode::Left | KeyCode::Char('h') => app.menu_move_column(false),
        KeyCode::Right | KeyCode::Char('l') => app.menu_move_column(true),
        KeyCode::Enter | KeyCode::Char(' ') => {
            let item = &app.menu[app.menu_index];
            let action = item.action;
            let mode = item.mode;
            let custom_path = item.custom_path.clone();
            match action {
                MenuAction::Start => {
                    if let Some(m) = mode {
                        if let Err(e) = app.start_game(m) {
                            app.show_error(&e);
                        }
                    }
                }
                MenuAction::StartCustom => {
                    // The placeholder option has no path: `start_custom`
                    // then explains how to add a custom file.
                    if let Err(e) = app.start_custom(custom_path) {
                        app.show_error(&e);
                    }
                }
                MenuAction::ShowStats => app.screen = Screen::Stats,
                MenuAction::Quit => app.should_quit = true,
            }
        }
        KeyCode::Char('p') => app.word_decor.punctuation ^= true,
        KeyCode::Char('n') => app.word_decor.numbers ^= true,
        KeyCode::Char('b') => {
            app.word_pool = match app.word_pool {
                WordPool::Common => WordPool::Extended,
                WordPool::Extended => WordPool::Common,
            }
        }
        KeyCode::Tab | KeyCode::Char('s') => app.screen = Screen::Stats,
        KeyCode::Char('q') => app.should_quit = true,
        _ => {}
    }
}

/// Keymap for the typing screen. Note that '?' is **not** a help shortcut
/// here — the user may legitimately need to type it.
pub(crate) fn handle_typing_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match code {
        KeyCode::Esc => {
            app.finish_game();
        }
        KeyCode::F(5) => {
            if let Err(e) = app.restart() {
                app.show_error(&e);
            }
        }
        KeyCode::Char('r') if mods.contains(KeyModifiers::CONTROL) => {
            if let Err(e) = app.restart() {
                app.show_error(&e);
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
            // to be counted as typed characters. AltGr is not one of them.
            if mods.contains(KeyModifiers::CONTROL) && !is_altgr(typed_char, mods) {
                return;
            }
            app.handle_char(typed_char);
        }
        _ => {}
    }
}

/// Whether a Ctrl-modified character is really AltGr. Windows reports AltGr
/// as Ctrl+Alt, and on German, French, Spanish, Polish… layouts AltGr is how
/// `{ } [ ] | @ ~ \` (and letters like `ą`) are typed — the symbols drill and
/// code modes are built on them. A Ctrl+Alt chord that gives an ASCII letter
/// or digit is a real chord (Linux reports Ctrl+Alt+A as `a`), not AltGr.
fn is_altgr(typed_char: char, mods: KeyModifiers) -> bool {
    mods.contains(KeyModifiers::CONTROL | KeyModifiers::ALT) && !typed_char.is_ascii_alphanumeric()
}

/// Keymap for the post-session results screen.
///
/// A fast typist is still typing when the run ends, so the next few keys
/// land here. Only keys nobody presses while typing act — Enter or F5
/// (or Ctrl+R, as while typing) restart, Esc goes to the menu, Tab to
/// stats, F1 opens help, Ctrl+C quits —
/// so those stray letters, spaces and `?` can't skip the results.
fn handle_results_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    let ctrl_r = code == KeyCode::Char('r') && mods.contains(KeyModifiers::CONTROL);
    match code {
        _ if ctrl_r => {
            if let Err(e) = app.restart() {
                app.show_error(&e);
            }
        }
        KeyCode::Enter | KeyCode::F(5) => {
            if let Err(e) = app.restart() {
                app.show_error(&e);
            }
        }
        KeyCode::Esc => app.screen = Screen::Menu,
        KeyCode::Tab => app.screen = Screen::Stats,
        _ => {}
    }
}

/// Keymap for the historical stats screen: ←/→ (h/l) pick a category, `a`
/// shows all sessions, ↑/↓ (j/k), PgUp/PgDn and Home/End scroll the list.
fn handle_stats_key(app: &mut App, code: KeyCode, _mods: KeyModifiers) {
    let history = app.stats_cache.as_deref().unwrap_or(&[]);
    if let Some(view) = app.stats_view.as_mut() {
        match code {
            KeyCode::Left | KeyCode::Char('h') => return view.cycle(history, false),
            KeyCode::Right | KeyCode::Char('l') => return view.cycle(history, true),
            KeyCode::Char('a') => return view.select(history, 0),
            KeyCode::Up | KeyCode::Char('k') => return view.scroll_by(-1),
            KeyCode::Down | KeyCode::Char('j') => return view.scroll_by(1),
            KeyCode::PageUp => return view.scroll_by(-10),
            KeyCode::PageDown => return view.scroll_by(10),
            KeyCode::Home => return view.scroll.set(0),
            KeyCode::End => return view.scroll_by(isize::MAX),
            _ => {}
        }
    }
    match code {
        KeyCode::Char('m') | KeyCode::Esc | KeyCode::Tab => app.screen = Screen::Menu,
        KeyCode::Char('q') => app.should_quit = true,
        _ => {}
    }
}

/// Bookkeeping after each round of input, driven by which screen we are on.
/// `stats_file` is the history file (a temp file in tests).
fn after_input(app: &mut App, last_screen: Screen, session_saved: &mut bool, stats_file: &Path) {
    // Save each finished game exactly once. The flag is cleared only when a
    // new game starts, so leaving Results for the Help overlay (or an error
    // modal) and coming back can't save the same session again.
    if app.screen == Screen::Results && !*session_saved {
        save_current_session(app, stats_file);
        *session_saved = true;
    }
    if app.screen == Screen::Typing {
        *session_saved = false;
    }

    // History is read from disk once; every save hands back the updated list,
    // so the cache stays current without re-reading the file. The render path
    // never touches disk.
    if matches!(app.screen, Screen::Stats | Screen::Results) && app.stats_cache.is_none() {
        app.stats_cache = Some(storage::load_sessions_from_path(stats_file).unwrap_or_default());
    }
    // Results comparisons and the Stats summary (streak, averages, key
    // heatmap) are computed once per visit instead of on every frame.
    if app.screen == Screen::Results && last_screen != Screen::Results {
        let sessions = app.stats_cache.as_deref().unwrap_or(&[]);
        app.results_comparison = Some(storage::ResultsComparison::new(
            sessions,
            app.session_just_saved,
            &app.session_label(),
        ));
    }
    // Coming back from the help overlay keeps the category and scroll; any
    // other way in rebuilds the view (a game may have been saved since).
    // Straight after a run it opens on that run's mode; otherwise it stays
    // on the category that was showing.
    if app.screen == Screen::Stats
        && (app.stats_view.is_none() || !matches!(last_screen, Screen::Stats | Screen::Help))
    {
        let category = if last_screen == Screen::Results && app.session_just_saved {
            Some(app.session_label())
        } else {
            app.stats_view
                .as_ref()
                .map(|view| view.category_name().to_string())
        };
        let sessions = app.stats_cache.as_deref().unwrap_or(&[]);
        app.stats_view = Some(storage::StatsView::new(sessions, category.as_deref()));
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
fn save_current_session(app: &mut App, stats_file: &Path) {
    // Assume not saved until the disk write succeeds; the Results screen
    // reads this to know whether the last history entry is this session.
    app.session_just_saved = false;
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
        mode: app.session_label(),
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
    match storage::save_session_to_path(stats_file, &record) {
        Ok(history) => {
            app.stats_cache = Some(history);
            app.session_just_saved = true;
        }
        // Disk state unknown: drop the cache so the next visit re-reads it.
        Err(_) => app.stats_cache = None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Screen, Word};
    use crate::theme::ThemePalette;

    fn make_typing_app() -> App {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
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

    /// Windows reports AltGr as Ctrl+Alt: on a German layout `{` is
    /// AltGr+7 and arrives as `Char('{')` with CONTROL | ALT. It must type.
    #[test]
    fn altgr_characters_are_typed() {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        app.screen = Screen::Typing;
        app.mode = Mode::Symbols(1);
        app.words = vec![Word::new("{@ą}".into())];
        let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
        for ch in ['{', '@', 'ą'] {
            handle_typing_key(&mut app, KeyCode::Char(ch), altgr);
        }
        // Shift+AltGr works too.
        handle_typing_key(&mut app, KeyCode::Char('}'), altgr | KeyModifiers::SHIFT);
        assert_eq!(app.correct_chars, 4);
        assert_eq!(app.screen, Screen::Results);
    }

    /// Ctrl+Alt with a letter or digit is a chord, not AltGr: ignored.
    #[test]
    fn ctrl_alt_letter_chords_are_ignored() {
        let mut app = make_typing_app();
        let chord = KeyModifiers::CONTROL | KeyModifiers::ALT;
        handle_typing_key(&mut app, KeyCode::Char('a'), chord);
        handle_typing_key(&mut app, KeyCode::Char('1'), chord);
        assert_eq!(app.total_typed_chars, 0);
    }

    // ── mouse ────────────────────────────────────────────────────────────────

    /// Draw one frame of `app` at 80×24 so its click targets are registered.
    fn draw(app: &App) {
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| ui::render(f, app)).unwrap();
    }

    /// Centre cell of the first click target matching `action`.
    fn target_cell(app: &App, action: ClickAction) -> (u16, u16) {
        let targets = app.click_targets.borrow();
        let (area, _) = targets.iter().find(|(_, a)| *a == action).unwrap();
        (area.x + area.width / 2, area.y)
    }

    fn mouse(kind: MouseEventKind, (column, row): (u16, u16)) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn click(app: &mut App, cell: (u16, u16)) {
        handle_mouse(app, mouse(MouseEventKind::Down(MouseButton::Left), cell));
        handle_mouse(app, mouse(MouseEventKind::Up(MouseButton::Left), cell));
    }

    /// The ‹ › around the Stats category are clickable in both directions;
    /// the two-way footer hints are not (a click there can only go one way).
    #[test]
    fn clicking_stats_arrows_changes_category_both_ways() {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        app.stats_cache = Some(Vec::new());
        app.screen = Screen::Stats;
        after_input(&mut app, Screen::Menu, &mut false, Path::new("unused"));
        draw(&app);
        let next = target_cell(&app, ClickAction::Key(KeyCode::Right, KeyModifiers::NONE));
        click(&mut app, next);
        assert_eq!(app.stats_view.as_ref().unwrap().category_name(), "bests");
        draw(&app);
        let previous = target_cell(&app, ClickAction::Key(KeyCode::Left, KeyModifiers::NONE));
        click(&mut app, previous);
        assert_eq!(app.stats_view.as_ref().unwrap().category_name(), "all");
        assert_eq!(previous.1, 0, "arrows sit on the title row");
    }

    #[test]
    fn clicking_a_menu_option_starts_it() {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        draw(&app);
        let thirty = app.menu.iter().position(|m| m.label == "30s").unwrap();
        let cell = target_cell(&app, ClickAction::Menu(thirty));
        click(&mut app, cell);
        assert_eq!(app.screen, Screen::Typing);
        assert_eq!(app.mode, Mode::Time(30));
    }

    /// WCAG 2.5.2: pressing on an option and releasing elsewhere only
    /// highlights it — nothing starts.
    #[test]
    fn menu_press_then_slide_off_cancels() {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        draw(&app);
        let cell = target_cell(&app, ClickAction::Menu(5));
        handle_mouse(
            &mut app,
            mouse(MouseEventKind::Down(MouseButton::Left), cell),
        );
        handle_mouse(
            &mut app,
            mouse(MouseEventKind::Up(MouseButton::Left), (0, 0)),
        );
        assert_eq!(app.screen, Screen::Menu);
        assert_eq!(app.menu_index, 5);
    }

    /// Press on one option, release on another: nothing starts. A release
    /// with no press (some terminals send them) does nothing either.
    #[test]
    fn drag_between_targets_or_stray_release_does_nothing() {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        draw(&app);
        let from = target_cell(&app, ClickAction::Menu(0));
        let to = target_cell(&app, ClickAction::Menu(3));
        handle_mouse(
            &mut app,
            mouse(MouseEventKind::Down(MouseButton::Left), from),
        );
        handle_mouse(&mut app, mouse(MouseEventKind::Up(MouseButton::Left), to));
        assert_eq!(app.screen, Screen::Menu);

        let quit = target_cell(
            &app,
            ClickAction::Key(KeyCode::Char('q'), KeyModifiers::NONE),
        );
        handle_mouse(&mut app, mouse(MouseEventKind::Up(MouseButton::Left), quit));
        assert!(!app.should_quit);
    }

    #[test]
    fn clicking_a_footer_hint_acts_like_its_key() {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        draw(&app);
        let stats = ClickAction::Key(KeyCode::Char('s'), KeyModifiers::NONE);
        let cell = target_cell(&app, stats);
        click(&mut app, cell);
        assert_eq!(app.screen, Screen::Stats);

        // Stats footer: "Esc / m menu" goes back — on the same last row as
        // the menu's footer.
        draw(&app);
        let menu = ClickAction::Key(KeyCode::Esc, KeyModifiers::NONE);
        let back = target_cell(&app, menu);
        assert_eq!(back.1, cell.1, "footer row moved between screens");
        click(&mut app, back);
        assert_eq!(app.screen, Screen::Menu);
    }

    /// Short terminal: the menu scrolls so the selected row stays visible
    /// and clickable (the old List widget did this; a Paragraph doesn't).
    #[test]
    fn menu_scrolls_to_selection_on_short_terminal() {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        app.menu_move_row(false); // wraps to "quit", the last row
        let quit = app.menu_index;
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(80, 18)).unwrap();
        terminal.draw(|f| ui::render(f, &app)).unwrap();
        let rows: Vec<String> = terminal
            .backend()
            .buffer()
            .content()
            .chunks(80)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect())
            .collect();
        // Selected last row is on screen, directly under "stats" (no gap).
        let quit_row = rows.iter().position(|r| r.contains("➤ quit")).unwrap();
        assert!(rows[quit_row - 1].contains("│  stats"));
        target_cell(&app, ClickAction::Menu(quit)); // panics if not clickable
    }

    #[test]
    fn click_closes_help_overlay() {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        handle_key(&mut app, KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(app.screen, Screen::Help);
        click(&mut app, (0, 0));
        assert_eq!(app.screen, Screen::Menu);
    }

    #[test]
    fn scroll_wheel_moves_menu_rows() {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        handle_mouse(&mut app, mouse(MouseEventKind::ScrollDown, (0, 0)));
        assert_eq!(app.menu[app.menu_index].group, "words");
        handle_mouse(&mut app, mouse(MouseEventKind::ScrollUp, (0, 0)));
        assert_eq!(app.menu[app.menu_index].group, "time");
    }

    // ── v0.4.0 ───────────────────────────────────────────────────────────────

    /// A zero count would start a session with nothing to type, and a huge
    /// one would build billions of words before the first frame; clap rejects
    /// both before the terminal is touched. Counts in range still parse.
    #[test]
    fn out_of_range_counts_are_rejected() {
        for (flag, max) in [("--time", 3600), ("--words", 10_000), ("--symbols", 10_000)] {
            for bad in [
                "0".to_string(),
                (max + 1).to_string(),
                "99999999999999".into(),
            ] {
                let err = Cli::try_parse_from(["typerush", flag, &bad]).unwrap_err();
                assert!(
                    err.to_string().contains(&format!("between 1 and {max}")),
                    "{flag} {bad}: {err}"
                );
            }
            for good in ["1", "25", &max.to_string()] {
                assert!(
                    Cli::try_parse_from(["typerush", flag, good]).is_ok(),
                    "{flag} {good}"
                );
            }
        }
    }

    #[test]
    fn code_flag_accepts_every_documented_alias() {
        use crate::words::CodeLang::*;
        for (name, lang) in [
            ("rust", Rust),
            ("rs", Rust),
            ("python", Python),
            ("py", Python),
            ("js", JavaScript),
            ("JavaScript", JavaScript),
            ("go", Go),
            ("golang", Go),
            ("java", Java),
            ("sql", Sql),
            ("shell", Shell),
            ("sh", Shell),
            ("bash", Shell),
        ] {
            assert_eq!(code_lang_from_cli(name).unwrap(), lang, "{name}");
        }
        let err = code_lang_from_cli("cobol").unwrap_err().to_string();
        assert!(err.contains("cobol"), "{err}");
    }

    /// Index of the menu option with this group and label.
    fn option(app: &App, group: &str, label: &str) -> usize {
        app.menu
            .iter()
            .position(|m| m.group == group && m.label == label)
            .unwrap_or_else(|| panic!("no {group} · {label} option"))
    }

    #[test]
    fn enter_on_placeholder_custom_option_explains_and_stays() {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        app.menu_index = option(&app, "custom", "custom");
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.screen, Screen::Menu);
        assert!(app.error_message.as_deref().unwrap().contains("--file"));
    }

    /// Picking a snippet starts it, remembers it in state.json, and keeps it
    /// highlighted for when the user comes back to the menu.
    #[test]
    fn picking_a_snippet_starts_and_remembers_it() {
        let dir = tempfile::tempdir().unwrap();
        let snippet_path = dir.path().join("drill.txt");
        std::fs::write(&snippet_path, "fn main ( ) { }").unwrap();
        let state_file = dir.path().join("state.json");

        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        app.load_custom_sources(
            words::snippets::discover_in(dir.path()),
            Default::default(),
            Some(state_file.clone()),
        );
        app.menu_index = option(&app, "custom", "drill");
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);

        assert_eq!(app.screen, Screen::Typing);
        assert_eq!(app.mode, Mode::Custom);
        assert_eq!(app.words[0].text, "fn");
        assert_eq!(
            state::load_from_path(&state_file)
                .last_custom_file
                .as_deref(),
            snippet_path.to_str()
        );
        assert_eq!(app.menu[app.menu_index].label, "drill");
    }

    /// A snippet deleted after startup: a clear error naming the file, and
    /// the previously remembered file is not replaced.
    #[test]
    fn deleted_snippet_errors_and_keeps_remembered_file() {
        let dir = tempfile::tempdir().unwrap();
        let snippet_path = dir.path().join("gone.txt");
        std::fs::write(&snippet_path, "hello").unwrap();
        let state_file = dir.path().join("state.json");
        let remembered = state::AppState {
            last_custom_file: Some("/kept/notes.txt".into()),
        };
        state::save_to_path(&state_file, &remembered).unwrap();

        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        app.load_custom_sources(
            words::snippets::discover_in(dir.path()),
            remembered.clone(),
            Some(state_file.clone()),
        );
        std::fs::remove_file(&snippet_path).unwrap();
        app.menu_index = option(&app, "custom", "gone");
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);

        assert_eq!(app.screen, Screen::Menu);
        assert!(app.error_message.as_deref().unwrap().contains("gone.txt"));
        assert_eq!(state::load_from_path(&state_file), remembered);
    }

    /// Draw `app` at `width`×`height`; returns the screen as text rows.
    fn screen(app: &App, width: u16, height: u16) -> Vec<String> {
        let mut terminal =
            Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| ui::render(f, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .chunks(width as usize)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect())
            .collect()
    }

    /// Every menu option is clickable in the frame just drawn.
    fn all_options_clickable(app: &App) -> bool {
        let targets = app.click_targets.borrow();
        (0..app.menu.len()).all(|i| targets.iter().any(|(_, a)| *a == ClickAction::Menu(i)))
    }

    /// A menu as a returning v0.4 user sees it: a remembered file and a few
    /// snippets in the custom row.
    fn full_menu_app() -> App {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        let snippet = |name: &str| words::snippets::Snippet {
            name: name.into(),
            path: PathBuf::from(format!("/s/{name}.txt")),
            canonical_path: None,
        };
        app.load_custom_sources(
            vec![snippet("drill"), snippet("essay"), snippet("hello")],
            state::AppState {
                last_custom_file: Some("/home/me/notes.txt".into()),
            },
            None,
        );
        app
    }

    /// Regression: v0.4's extra rows pushed custom, stats and quit off a
    /// standard 80×24 terminal at launch, with no sign there was more. Now
    /// the whole menu is on screen at launch — the plain one and one with a
    /// remembered file and snippets — with the banner and no "more" hint.
    #[test]
    fn whole_menu_fits_80x24_at_launch() {
        for app in [
            App::new(None, ThemePalette::default(), Mode::Time(15)),
            full_menu_app(),
        ] {
            let rows = screen(&app, 80, 24);
            let dump = rows.join("\n");
            assert!(all_options_clickable(&app), "{dump}");
            assert!(!dump.contains("more"), "{dump}");
            assert!(dump.contains("▀█▀"), "compact banner missing:\n{dump}");
            for text in ["stats", "quit", "symbols", "custom", "punctuation off"] {
                assert!(dump.contains(text), "{text:?} missing:\n{dump}");
            }
        }
    }

    /// When not every blank line between categories fits, none are drawn:
    /// a mix (dense at the top, spaced below) looks misaligned.
    #[test]
    fn category_gaps_are_all_or_nothing() {
        let app = App::new(None, ThemePalette::default(), Mode::Time(15));
        let rows = screen(&app, 80, 24);
        let dump = rows.join("\n");
        let first = rows.iter().position(|r| r.contains("time")).unwrap();
        let last = rows.iter().position(|r| r.contains("quit")).unwrap();
        let blanks = rows[first..last]
            .iter()
            .filter(|r| r.trim_matches(['│', ' ']).is_empty())
            .count();
        assert_eq!(blanks, 0, "{dump}");
    }

    /// With room to spare the menu looks like v0.3: the big banner, and a
    /// blank line between categories.
    #[test]
    fn tall_terminal_gets_big_banner_and_gaps() {
        let app = full_menu_app();
        let rows = screen(&app, 100, 40);
        let dump = rows.join("\n");
        // Title row, blank row, then the box: a blank line, then the banner.
        assert!(rows[0].starts_with("  ◆ select mode"), "{dump}");
        assert!(rows[3].trim_matches(['│', ' ']).is_empty(), "{dump}");
        assert!(rows[4].contains("████████"), "{dump}");
        let time = rows.iter().position(|r| r.contains("│➤ time")).unwrap();
        assert!(rows[time + 2].trim_matches(['│', ' ']).is_empty(), "{dump}");
        assert!(rows[time + 3].contains("words"), "{dump}");
        assert!(all_options_clickable(&app), "{dump}");
    }

    /// At 80 columns the seven code options fit on one line. On a narrow
    /// terminal they wrap, every one stays inside the box and clickable, and
    /// clicking the last one starts it.
    #[test]
    fn code_row_fits_at_80_and_wraps_when_narrow() {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        app.menu_index = option(&app, "code", "rust");
        let code_lines = |app: &App| -> Vec<ratatui::layout::Rect> {
            let targets = app.click_targets.borrow();
            app.menu
                .iter()
                .enumerate()
                .filter(|(_, m)| m.group == "code")
                .map(|(i, _)| {
                    targets
                        .iter()
                        .find(|(_, a)| *a == ClickAction::Menu(i))
                        .map(|(area, _)| *area)
                        .unwrap_or_else(|| panic!("code option {i} not clickable"))
                })
                .collect()
        };
        screen(&app, 80, 24);
        let code = code_lines(&app);
        assert!(
            code.iter().all(|a| a.y == code[0].y),
            "code row wrapped at 80"
        );

        screen(&app, 50, 40);
        let code = code_lines(&app);
        let lines: std::collections::BTreeSet<u16> = code.iter().map(|a| a.y).collect();
        assert!(lines.len() > 1, "expected the code row to wrap");
        // Chips on one line never overlap.
        for pair in code.windows(2) {
            if pair[0].y == pair[1].y {
                assert!(pair[0].x + pair[0].width <= pair[1].x);
            }
        }
        let shell = option(&app, "code", "shell");
        let cell = target_cell(&app, ClickAction::Menu(shell));
        click(&mut app, cell);
        assert_eq!(app.mode, Mode::Code(crate::words::CodeLang::Shell));
    }

    /// Every menu option can be scrolled into view and clicked on a standard
    /// 80×24 terminal, including the new rows below zen.
    #[test]
    fn every_option_reachable_at_80x24() {
        let mut app = full_menu_app();
        for index in 0..app.menu.len() {
            app.menu_index = index;
            draw(&app);
            target_cell(&app, ClickAction::Menu(index));
        }
    }

    /// Too short for the whole list: it scrolls to the selection, and the
    /// border says which way there is more — nothing is hidden silently.
    #[test]
    fn short_terminal_scrolls_with_more_hints() {
        let mut app = full_menu_app();
        let rows = screen(&app, 80, 12);
        let dump = rows.join("\n");
        assert!(
            dump.contains("▼ more") && !dump.contains("▲ more"),
            "{dump}"
        );
        assert!(!dump.contains("▀█▀"), "no room for a banner:\n{dump}");

        app.menu_index = app.menu.len() - 1; // quit
        let rows = screen(&app, 80, 12);
        let dump = rows.join("\n");
        assert!(
            dump.contains("▲ more") && !dump.contains("▼ more"),
            "{dump}"
        );
        target_cell(&app, ClickAction::Menu(app.menu_index));
    }

    /// `p`, `n`, `b` toggle the word settings, the border shows each one's
    /// state in words, and the next run's label follows.
    #[test]
    fn menu_keys_toggle_word_settings() {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(30));
        for code in ['p', 'n', 'b'] {
            handle_key(&mut app, KeyCode::Char(code), KeyModifiers::NONE);
        }
        assert!(app.word_decor.punctuation && app.word_decor.numbers);
        assert_eq!(app.word_pool, WordPool::Extended);
        let dump = screen(&app, 80, 24).join("\n");
        for text in ["punctuation on", "numbers on", "10k words on"] {
            assert!(dump.contains(text), "{text:?} missing:\n{dump}");
        }
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.session_label(), "time-30s+10k+p+n");

        // And back off again.
        app.screen = Screen::Menu;
        for code in ['p', 'n', 'b'] {
            handle_key(&mut app, KeyCode::Char(code), KeyModifiers::NONE);
        }
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.session_label(), "time-30s");
    }

    /// The settings on the border are clickable and act like their keys.
    #[test]
    fn clicking_a_word_setting_toggles_it() {
        let mut app = App::new(None, ThemePalette::default(), Mode::Time(15));
        draw(&app);
        let numbers = ClickAction::Key(KeyCode::Char('n'), KeyModifiers::NONE);
        let cell = target_cell(&app, numbers);
        click(&mut app, cell);
        assert!(app.word_decor.numbers);
        assert_eq!(app.screen, Screen::Menu);
    }

    #[test]
    fn cli_switch_pairs_resolve_last_one_wins() {
        let overrides = |args: &[&str]| {
            let cli = Cli::try_parse_from([&["typerush"], args].concat()).unwrap();
            let o = cli.overrides();
            (o.word_pool, o.punctuation, o.numbers)
        };
        assert_eq!(overrides(&[]), (None, None, None));
        assert_eq!(
            overrides(&["--big", "--punctuation", "--numbers"]),
            (Some(WordPool::Extended), Some(true), Some(true))
        );
        assert_eq!(
            overrides(&["--no-big", "--no-punctuation", "--no-numbers"]),
            (Some(WordPool::Common), Some(false), Some(false))
        );
        assert_eq!(
            overrides(&["--punctuation", "--no-punctuation", "--no-big", "--big"]),
            (Some(WordPool::Extended), Some(false), None)
        );
    }

    /// Keys a typist is still pressing when the run ends land on Results:
    /// letters, space, digits and `?` do nothing there. Only Enter / F5,
    /// Esc, Tab and F1 act.
    #[test]
    fn stray_typing_keys_do_not_leave_results() {
        let mut app = make_typing_app();
        for ch in "hello world".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.screen, Screen::Results);
        for ch in "rmsq? the quick brown 123".chars() {
            handle_key(&mut app, KeyCode::Char(ch), KeyModifiers::NONE);
            handle_key(&mut app, KeyCode::Char(ch), KeyModifiers::SHIFT);
        }
        handle_key(&mut app, KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(app.screen, Screen::Results);
        assert!(!app.should_quit);

        handle_key(&mut app, KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(app.screen, Screen::Help);
        handle_key(&mut app, KeyCode::F(1), KeyModifiers::NONE);
        handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.screen, Screen::Stats);
        app.screen = Screen::Results;
        handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(app.screen, Screen::Menu);
        app.screen = Screen::Results;
        handle_key(&mut app, KeyCode::F(5), KeyModifiers::NONE);
        assert_eq!(app.screen, Screen::Typing);
        // Ctrl+R restarts here too, as while typing; a plain r doesn't.
        app.screen = Screen::Results;
        handle_key(&mut app, KeyCode::Char('r'), KeyModifiers::CONTROL);
        assert_eq!(app.screen, Screen::Typing);
    }

    /// Stats opened straight after a run shows that run's mode; [all] (or
    /// `a`) goes back to every session; from the menu Stats keeps the
    /// category that was showing.
    #[test]
    fn stats_after_a_run_opens_on_its_mode() {
        let dir = tempfile::tempdir().unwrap();
        let stats_file = dir.path().join("stats.json");
        let mut app = make_typing_app(); // words-2
        let (mut saved, mut last) = (false, app.screen);
        let mut step = |app: &mut App| {
            after_input(app, last, &mut saved, &stats_file);
            last = app.screen;
        };
        step(&mut app);
        app.started_at = Some(std::time::Instant::now() - Duration::from_secs(5));
        for ch in "hello world".chars() {
            app.handle_char(ch);
        }
        step(&mut app);
        assert!(app.session_just_saved);

        handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
        step(&mut app);
        assert_eq!(app.stats_view.as_ref().unwrap().category_name(), "words-2");

        // Click [all] on the title row.
        draw(&app);
        let all = target_cell(
            &app,
            ClickAction::Key(KeyCode::Char('a'), KeyModifiers::NONE),
        );
        click(&mut app, all);
        assert_eq!(app.stats_view.as_ref().unwrap().category_name(), "all");

        // Menu → Stats keeps the category that was showing.
        handle_key(&mut app, KeyCode::Right, KeyModifiers::NONE); // bests
        handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        step(&mut app);
        handle_key(&mut app, KeyCode::Char('s'), KeyModifiers::NONE);
        step(&mut app);
        assert_eq!(app.stats_view.as_ref().unwrap().category_name(), "bests");
        handle_key(&mut app, KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(app.stats_view.as_ref().unwrap().category_name(), "all");
    }

    // ── saving ───────────────────────────────────────────────────────────────

    /// Regression: opening help from Results and closing it used to save the
    /// same session a second time (the flag reset on any screen change).
    #[test]
    fn help_detour_from_results_does_not_save_twice() {
        let dir = tempfile::tempdir().unwrap();
        let stats_file = dir.path().join("stats.json");
        let mut app = make_typing_app();
        let mut saved = false;
        let mut last = app.screen;
        let mut step = |app: &mut App| {
            after_input(app, last, &mut saved, &stats_file);
            last = app.screen;
        };
        step(&mut app);

        app.started_at = Some(std::time::Instant::now() - Duration::from_secs(5));
        for ch in "hello world".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.screen, Screen::Results);
        step(&mut app);
        assert!(app.session_just_saved);

        handle_key(&mut app, KeyCode::F(1), KeyModifiers::NONE); // open help
        step(&mut app);
        handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE); // close it
        step(&mut app);
        assert_eq!(app.screen, Screen::Results);

        let history = storage::load_sessions_from_path(&stats_file).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(app.stats_cache.as_ref().map(Vec::len), Some(1));

        // A new game is saved again as normal.
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE); // restart
        step(&mut app);
        app.started_at = Some(std::time::Instant::now() - Duration::from_secs(5));
        app.words = vec![Word::new("hi".into())];
        app.mode = Mode::Words(1);
        app.handle_char('h');
        app.handle_char('i');
        step(&mut app);
        assert_eq!(
            storage::load_sessions_from_path(&stats_file).unwrap().len(),
            2
        );
    }
}
