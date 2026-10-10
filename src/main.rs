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
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::{App, ClickAction, MenuAction, Mode, Screen};
use crate::config::load::{code_lang_kind, CliOverrides};
use crate::storage::SessionRecord;
use crate::theme::builtin;
use crate::words::{CodeLang, WordPool};

/// Value parser for counts that must be at least 1. A zero count would start
/// a session with nothing to type that can only be left with Esc.
fn positive<T>() -> clap::builder::RangedU64ValueParser<T>
where
    T: TryFrom<u64> + Clone + Send + Sync + 'static,
    <T as TryFrom<u64>>::Error: std::fmt::Display,
{
    clap::builder::RangedU64ValueParser::<T>::new().range(1..)
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
    #[arg(long, value_parser = positive::<u64>())]
    time: Option<u64>,

    /// Start directly in words mode for N words.
    #[arg(long, value_parser = positive::<usize>())]
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
    #[arg(long, value_parser = positive::<usize>())]
    symbols: Option<usize>,

    /// Use the 10,000-word English pool instead of the default common words.
    #[arg(long)]
    big: bool,

    /// Add punctuation to random words (time and words modes).
    #[arg(long)]
    punctuation: bool,

    /// Mix numbers in with random words (time and words modes).
    #[arg(long)]
    numbers: bool,

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
    if let Err(error) = run_result {
        // The report Rust prints for an `Err` from `main`, line by line, but
        // with any control characters (from a `--file` path) shown as `?`.
        let report = format!("{error:?}");
        let lines: Vec<_> = report.lines().map(text::printable).collect();
        eprintln!("Error: {}", lines.join("\n"));
        std::process::exit(1);
    }
    Ok(())
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
            text::printable(&snippet.path.display().to_string())
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
    // CLI switches only ever turn a setting on; leaving one off keeps config.
    let (resolved, warnings) = config::load::load_or_default_with(CliOverrides {
        theme: cli.theme.as_deref(),
        word_pool: cli.big.then_some(WordPool::Extended),
        punctuation: cli.punctuation.then_some(true),
        numbers: cli.numbers.then_some(true),
    });
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
/// (one table, `config::load::code_lang_kind`), but an unknown name is an
/// error here rather than a fall-back-with-warning.
fn code_lang_from_cli(name: &str) -> Result<CodeLang> {
    code_lang_kind(name).map(CodeLang::from).ok_or_else(|| {
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
    // '?' (or F1) toggles the help overlay everywhere except during typing
    // (where '?' is a valid character to type).
    if matches!(code, KeyCode::Char('?') | KeyCode::F(1)) && app.screen != Screen::Typing {
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
/// an option within it, Enter (or Space) to act.
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
                            app.error_message = Some(e.to_string());
                        }
                    }
                }
                MenuAction::StartCustom => {
                    // The placeholder option has no path: `start_custom`
                    // then explains how to add a custom file.
                    if let Err(e) = app.start_custom(custom_path) {
                        // `{:#}` includes the cause ("can't read …: No such file").
                        app.error_message = Some(format!("{e:#}"));
                    }
                }
                MenuAction::ShowStats => app.screen = Screen::Stats,
                MenuAction::Quit => app.should_quit = true,
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
                app.error_message = Some(e.to_string());
            }
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
    if app.screen == Screen::Stats && last_screen != Screen::Stats {
        let sessions = app.stats_cache.as_deref().unwrap_or(&[]);
        app.stats_summary = Some(storage::StatsSummary::new(sessions));
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
    use crate::config::load::DefaultMode;
    use crate::theme::ThemePalette;

    fn make_typing_app() -> App {
        let mut app = App::new(None, ThemePalette::default(), DefaultMode::Time(15));
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

    #[test]
    fn clicking_a_menu_option_starts_it() {
        let mut app = App::new(None, ThemePalette::default(), DefaultMode::Time(15));
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
        let mut app = App::new(None, ThemePalette::default(), DefaultMode::Time(15));
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
        let mut app = App::new(None, ThemePalette::default(), DefaultMode::Time(15));
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
        let mut app = App::new(None, ThemePalette::default(), DefaultMode::Time(15));
        draw(&app);
        let stats = ClickAction::Key(KeyCode::Char('s'), KeyModifiers::NONE);
        let cell = target_cell(&app, stats);
        click(&mut app, cell);
        assert_eq!(app.screen, Screen::Stats);

        // Stats footer: "m / esc menu" goes back.
        draw(&app);
        let menu = ClickAction::Key(KeyCode::Char('m'), KeyModifiers::NONE);
        let cell = target_cell(&app, menu);
        click(&mut app, cell);
        assert_eq!(app.screen, Screen::Menu);
    }

    /// Short terminal: the menu scrolls so the selected row stays visible
    /// and clickable (the old List widget did this; a Paragraph doesn't).
    #[test]
    fn menu_scrolls_to_selection_on_short_terminal() {
        let mut app = App::new(None, ThemePalette::default(), DefaultMode::Time(15));
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
        let quit_row = rows.iter().position(|r| r.contains("➤  quit")).unwrap();
        assert!(rows[quit_row - 1].contains("    stats"));
        target_cell(&app, ClickAction::Menu(quit)); // panics if not clickable
    }

    #[test]
    fn click_closes_help_overlay() {
        let mut app = App::new(None, ThemePalette::default(), DefaultMode::Time(15));
        handle_key(&mut app, KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(app.screen, Screen::Help);
        click(&mut app, (0, 0));
        assert_eq!(app.screen, Screen::Menu);
    }

    #[test]
    fn scroll_wheel_moves_menu_rows() {
        let mut app = App::new(None, ThemePalette::default(), DefaultMode::Time(15));
        handle_mouse(&mut app, mouse(MouseEventKind::ScrollDown, (0, 0)));
        assert_eq!(app.menu[app.menu_index].group, "words");
        handle_mouse(&mut app, mouse(MouseEventKind::ScrollUp, (0, 0)));
        assert_eq!(app.menu[app.menu_index].group, "time");
    }

    // ── v0.4.0 ───────────────────────────────────────────────────────────────

    /// A zero count would start a session with nothing to type; clap rejects
    /// it before the terminal is touched. Positive counts still parse.
    #[test]
    fn zero_counts_are_rejected() {
        for flag in ["--time", "--words", "--symbols"] {
            assert!(
                Cli::try_parse_from(["typerush", flag, "0"]).is_err(),
                "{flag} 0"
            );
            assert!(
                Cli::try_parse_from(["typerush", flag, "25"]).is_ok(),
                "{flag} 25"
            );
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
        let mut app = App::new(None, ThemePalette::default(), DefaultMode::Time(15));
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

        let mut app = App::new(None, ThemePalette::default(), DefaultMode::Time(15));
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

        let mut app = App::new(None, ThemePalette::default(), DefaultMode::Time(15));
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

    /// At 80×24 the seven code options don't fit on one line: they wrap,
    /// every one stays inside the menu box and is clickable, and clicking
    /// the last one starts it.
    #[test]
    fn code_row_wraps_and_every_option_is_clickable() {
        let mut app = App::new(None, ThemePalette::default(), DefaultMode::Time(15));
        app.menu_index = option(&app, "code", "rust");
        draw(&app);
        let targets = app.click_targets.borrow().clone();
        let code: Vec<_> = app
            .menu
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
            .collect();
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
        let mut app = App::new(None, ThemePalette::default(), DefaultMode::Time(15));
        for index in 0..app.menu.len() {
            app.menu_index = index;
            draw(&app);
            target_cell(&app, ClickAction::Menu(index));
        }
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
