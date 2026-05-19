//! The application's central state machine.
//!
//! `App` owns everything that lives between renders: which screen we're on,
//! the words being typed, the running stats, the menu position, etc. The event
//! loop in `main.rs` mutates an `App` and the UI modules read from it — there
//! is no other shared state.
//!
//! The flow looks roughly like this:
//!
//! ```text
//! Menu  ──Enter──►  Typing  ──finish/Esc──►  Results
//!   ▲                                            │
//!   └────────────── Esc / 'm' ───────────────────┘
//! ```

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::config::load::{CodeLangKind, DefaultMode};
use crate::game::{get_char_states, CharState};
use crate::state::{self, AppState};
use crate::storage::{self, AggregateStats, SessionRecord};
use crate::theme::ThemePalette;
use crate::words::{snippets::Snippet, WordDecor, WordPool};

/// One of the high-level screens the user can be looking at. The current
/// `Screen` drives both the renderer dispatch in `ui::render` and the keymap
/// dispatch in `main::handle_key`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    /// Main menu — pick a mode.
    Menu,
    /// The typing game is active.
    Typing,
    /// Final stats for the just-finished session.
    Results,
    /// Browse historical sessions (`~/.typerush/stats.json`).
    Stats,
    /// Floating keybindings overlay (any screen can toggle it on).
    Help,
}

/// A typing-game mode. Each mode determines:
///   - which word source to use (random English, quotes, code, custom file…)
///   - when the session ends (timer up, word count reached, last char typed)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Type as many words as possible in N seconds.
    Time(u64),
    /// Type exactly N words.
    Words(usize),
    /// A single programming quote.
    Quote,
    /// A short real-world code snippet in the given language.
    Code(crate::words::CodeLang),
    /// No timer, no stats. Just type.
    Zen,
    /// Words sourced from a user-supplied text file (`--file path.txt` or a
    /// snippet picked from `~/.typerush/snippets/`).
    Custom,
    /// Drill programming punctuation: type N short symbol tokens like `=>`,
    /// `(){};` and other characters typists usually under-train.
    Symbols(usize),
}

impl Mode {
    /// Short tag used when saving sessions to disk and in the UI ("time-30s",
    /// "words-50", "code-rust", …). Keeping the format stable means old stats
    /// stay readable across releases.
    ///
    /// `Mode::Code(JavaScript)` deliberately serialises as `"code-javascript"`
    /// rather than the slug `"js"` — old session records use that string and
    /// per-mode PB lookup matches by exact label, so we must not regress it.
    pub fn label(&self) -> String {
        match self {
            Mode::Time(seconds) => format!("time-{}s", seconds),
            Mode::Words(count) => format!("words-{}", count),
            Mode::Quote => "quote".to_string(),
            Mode::Code(lang) => format!("code-{:?}", lang).to_lowercase(),
            Mode::Zen => "zen".to_string(),
            Mode::Custom => "custom".to_string(),
            Mode::Symbols(count) => format!("symbols-{}", count),
        }
    }
}

/// A single word in the typing prompt — the target text plus what the user
/// has actually typed so far.
#[derive(Clone, Debug)]
pub struct Word {
    /// What the user is supposed to type.
    pub text: String,
    /// What the user has typed so far (may be incomplete, may have errors).
    pub typed: String,
    /// True once the user has pressed space (or otherwise advanced past it).
    pub submitted: bool,
}

impl Word {
    pub fn new(text: String) -> Self {
        Self {
            text,
            typed: String::new(),
            submitted: false,
        }
    }
}

/// One row in the main menu.
pub struct MenuItem {
    /// User-visible row label. Owned `String` so dynamic rows (e.g. the
    /// "Custom · last/file.txt" row, or one per discovered snippet) can show
    /// runtime data.
    pub label: String,
    /// Set for "start a game" rows; `None` for rows like "Stats" or "Quit"
    /// or visual section separators.
    pub mode: Option<Mode>,
    pub action: MenuAction,
    /// For `MenuAction::StartCustom`, the snippet path to use as the word
    /// source. `None` for every other row.
    pub custom_path: Option<String>,
}

/// What pressing Enter on a menu item should do.
#[derive(Clone, Copy, Debug)]
pub enum MenuAction {
    /// Start the game in `MenuItem::mode`.
    Start,
    /// Start a custom-file session using `MenuItem::custom_path`. If the path
    /// is `None`, falls back to the remembered last custom file from
    /// `state.json`; if that's also missing, surfaces a friendly error.
    StartCustom,
    /// Jump to the historical stats screen.
    ShowStats,
    /// Quit the application.
    Quit,
    /// Decorative section header / spacer — Enter does nothing and the
    /// keymap skips over it when navigating with arrow keys.
    Separator,
}

/// Translate a `DefaultMode` (the config-side enum) into a runtime `Mode`.
fn mode_for(default_mode: DefaultMode) -> Mode {
    use crate::words::CodeLang;
    match default_mode {
        DefaultMode::Time(seconds) => Mode::Time(seconds),
        DefaultMode::Words(count) => Mode::Words(count),
        DefaultMode::Quote => Mode::Quote,
        DefaultMode::Code(CodeLangKind::Rust) => Mode::Code(CodeLang::Rust),
        DefaultMode::Code(CodeLangKind::Python) => Mode::Code(CodeLang::Python),
        DefaultMode::Code(CodeLangKind::JavaScript) => Mode::Code(CodeLang::JavaScript),
        DefaultMode::Code(CodeLangKind::Go) => Mode::Code(CodeLang::Go),
        DefaultMode::Code(CodeLangKind::Java) => Mode::Code(CodeLang::Java),
        DefaultMode::Code(CodeLangKind::Sql) => Mode::Code(CodeLang::Sql),
        DefaultMode::Code(CodeLangKind::Shell) => Mode::Code(CodeLang::Shell),
        DefaultMode::Zen => Mode::Zen,
        DefaultMode::Symbols(count) => Mode::Symbols(count),
    }
}

/// Pick which menu row to highlight given the configured default mode.
///
/// Exact match wins (e.g. `time_seconds = 30` → "Time · 30s"). Otherwise we
/// fall back to the first row of the same mode family (so `time_seconds = 45`
/// still pre-selects a time row rather than landing on something unrelated).
fn best_menu_match(menu: &[MenuItem], default_mode: DefaultMode) -> usize {
    let exact = mode_for(default_mode);
    if let Some(index) = menu.iter().position(|item| item.mode == Some(exact)) {
        return index;
    }
    let family_match = menu.iter().position(|item| {
        matches!(
            (item.mode, default_mode),
            (Some(Mode::Time(_)), DefaultMode::Time(_))
                | (Some(Mode::Words(_)), DefaultMode::Words(_))
                | (Some(Mode::Code(_)), DefaultMode::Code(_))
                | (Some(Mode::Quote), DefaultMode::Quote)
                | (Some(Mode::Zen), DefaultMode::Zen)
                | (Some(Mode::Symbols(_)), DefaultMode::Symbols(_))
        )
    });
    // Skip separator rows when no other match exists.
    family_match
        .or_else(|| {
            menu.iter()
                .position(|item| !matches!(item.action, MenuAction::Separator))
        })
        .unwrap_or(0)
}

/// Build a menu row representing a visual section header. Rows with
/// `MenuAction::Separator` are skipped by arrow-key navigation and rendered
/// in muted style by the menu UI.
fn separator(label: &str) -> MenuItem {
    MenuItem {
        label: label.to_string(),
        mode: None,
        action: MenuAction::Separator,
        custom_path: None,
    }
}

/// Convenience constructor for a "start this mode" row.
fn start_row(label: impl Into<String>, mode: Mode) -> MenuItem {
    MenuItem {
        label: label.into(),
        mode: Some(mode),
        action: MenuAction::Start,
        custom_path: None,
    }
}

/// Build the default menu shown on startup, plus any user-discovered
/// snippets. `last_custom_file` (if any) populates the "Custom" row with the
/// last path so a one-key restart works.
pub fn build_menu(snippets: &[Snippet], last_custom_file: Option<&str>) -> Vec<MenuItem> {
    use crate::words::CodeLang;
    let mut menu: Vec<MenuItem> = vec![
        separator("── Time ──"),
        start_row("Time · 15s", Mode::Time(15)),
        start_row("Time · 30s", Mode::Time(30)),
        start_row("Time · 60s", Mode::Time(60)),
        start_row("Time · 120s", Mode::Time(120)),
        separator("── Words ──"),
        start_row("Words · 10", Mode::Words(10)),
        start_row("Words · 25", Mode::Words(25)),
        start_row("Words · 50", Mode::Words(50)),
        start_row("Words · 100", Mode::Words(100)),
        separator("── Quote ──"),
        start_row("Quote", Mode::Quote),
        separator("── Code ──"),
        start_row("Code · Rust", Mode::Code(CodeLang::Rust)),
        start_row("Code · Python", Mode::Code(CodeLang::Python)),
        start_row("Code · JavaScript", Mode::Code(CodeLang::JavaScript)),
        start_row("Code · Go", Mode::Code(CodeLang::Go)),
        start_row("Code · Java", Mode::Code(CodeLang::Java)),
        start_row("Code · SQL", Mode::Code(CodeLang::Sql)),
        start_row("Code · Shell", Mode::Code(CodeLang::Shell)),
        separator("── Symbols ──"),
        start_row("Symbols · 25", Mode::Symbols(25)),
        start_row("Symbols · 50", Mode::Symbols(50)),
        separator("── Zen ──"),
        start_row("Zen", Mode::Zen),
        separator("── Custom ──"),
        custom_row(last_custom_file),
    ];
    for snippet in snippets {
        menu.push(MenuItem {
            label: format!("Snippet · {}", snippet.name),
            mode: Some(Mode::Custom),
            action: MenuAction::StartCustom,
            custom_path: Some(snippet.path.to_string_lossy().to_string()),
        });
    }
    menu.push(separator("── More ──"));
    menu.push(MenuItem {
        label: "Stats".to_string(),
        mode: None,
        action: MenuAction::ShowStats,
        custom_path: None,
    });
    menu.push(MenuItem {
        label: "Quit".to_string(),
        mode: None,
        action: MenuAction::Quit,
        custom_path: None,
    });
    menu
}

/// "Custom" menu row. Shows the last-used path (truncated) when one exists,
/// otherwise a hint that the user should pass `--file` or drop snippets.
fn custom_row(last_custom_file: Option<&str>) -> MenuItem {
    let label = match last_custom_file {
        Some(path) => format!("Custom · {}", truncate_for_menu(path, 48)),
        None => "Custom · (pass --file or drop a .txt in ~/.typerush/snippets/)".to_string(),
    };
    MenuItem {
        label,
        mode: Some(Mode::Custom),
        action: MenuAction::StartCustom,
        custom_path: None,
    }
}

/// Shorten `path` to at most `max` characters (not bytes) by replacing the
/// middle with `…`, keeping the head and (more importantly) the file name
/// visible.
fn truncate_for_menu(path: &str, max: usize) -> String {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() <= max {
        return path.to_string();
    }
    // Reserve one slot for the ellipsis itself; weight the kept text toward
    // the tail so the file name on the right stays visible.
    let keep = max.saturating_sub(1);
    let head = keep / 3;
    let tail = keep - head;
    let head_s: String = chars.iter().take(head).collect();
    let tail_s: String = chars.iter().skip(chars.len() - tail).collect();
    format!("{}…{}", head_s, tail_s)
}

/// The entire mutable state of the application.
///
/// Every field is `pub` so renderers and keyboard handlers can both read and
/// (where appropriate) update it without a layer of getters/setters.
pub struct App {
    // --- screen / menu state ---
    /// Currently displayed screen.
    pub screen: Screen,
    /// Screen we should return to when the user closes the Help overlay.
    pub previous_screen: Screen,
    /// All menu rows.
    pub menu: Vec<MenuItem>,
    /// Highlight index inside `menu`.
    pub menu_index: usize,

    // --- game state ---
    /// Active mode while in `Screen::Typing` or `Screen::Results`.
    pub mode: Mode,
    /// All target words for this session.
    pub words: Vec<Word>,
    /// Index into `words` — the word the user is currently typing.
    pub current_word: usize,
    /// When the user pressed the first key of this session (set lazily).
    pub started_at: Option<Instant>,
    /// When the session finished (timer up, all words done, or Esc pressed).
    pub ended_at: Option<Instant>,

    // --- counters used for WPM / accuracy ---
    /// Characters typed that matched the target. Drives both WPM and accuracy.
    pub correct_chars: usize,
    /// Every character the user has typed during the session, including
    /// spaces (a successfully submitted space counts as one extra correct char).
    pub total_typed_chars: usize,
    /// Total backspaces pressed — informational only.
    pub backspaces: usize,

    // --- misc ---
    /// Path passed to `--file`, if any.
    pub custom_file: Option<String>,
    /// Set to `true` from any handler to exit the main loop cleanly.
    pub should_quit: bool,
    /// Transient error message rendered as a modal overlay.
    pub error_message: Option<String>,
    /// Counts every `tick()`. Used to throttle costly UI updates if needed.
    pub tick_count: u64,
    /// Active color palette — read by every UI module on every frame.
    pub theme: ThemePalette,
    /// Which English-word pool to draw from in Time / Words modes. Picked at
    /// startup from config + CLI; can be toggled at runtime from the menu.
    pub word_pool: WordPool,
    /// Punctuation / numbers decoration to apply to randomly-picked words.
    pub word_decor: WordDecor,
    /// Persistent UI state (last custom file, etc.) loaded once at startup.
    pub app_state: AppState,

    // --- per-key accuracy (v0.3.0) ---
    /// Number of times each key was typed at the correct position.
    pub key_hits: HashMap<char, u64>,
    /// Number of times each key was typed but did not match (wrong key or extra).
    pub key_misses: HashMap<char, u64>,

    // --- stats caching (v0.3.0) ---
    /// Full session history, loaded once when the Stats screen is entered and
    /// invalidated on each session save. `None` until the first Stats visit.
    pub stats_cache: Option<Vec<SessionRecord>>,
    /// Running aggregate of per-key hit/miss totals across all sessions.
    /// Loaded from `aggregate.json` at startup; kept current in memory after
    /// each save so the key-accuracy panel never re-reads the full history.
    pub aggregate: AggregateStats,
}

impl App {
    /// Construct a fresh `App` sitting on the main menu.
    ///
    /// `default_mode` (from `~/.typerush/config.toml`) controls which menu row
    /// is pre-selected and what `app.mode` starts as. `palette` is the active
    /// color theme — UI modules read it on every frame.
    ///
    /// Discovers `~/.typerush/snippets/*.txt` and loads `state.json` so the
    /// "Custom" row and snippet rows are populated immediately.
    pub fn new(
        custom_file: Option<String>,
        palette: ThemePalette,
        default_mode: DefaultMode,
        word_pool: WordPool,
        word_decor: WordDecor,
    ) -> Self {
        let app_state = state::load_state();
        let snippets = crate::words::snippets::discover_snippets();
        // `--file` from the CLI always wins for the initial custom path; if
        // unset, fall back to the last-used file we persisted in state.json so
        // the "Custom" menu row works on a bare `typerush` launch.
        let effective_custom_file = custom_file
            .clone()
            .or_else(|| app_state.last_custom_file.clone());
        let menu = build_menu(&snippets, effective_custom_file.as_deref());
        let initial_mode = mode_for(default_mode);
        let menu_index = best_menu_match(&menu, default_mode);
        Self {
            screen: Screen::Menu,
            previous_screen: Screen::Menu,
            menu,
            menu_index,
            mode: initial_mode,
            words: vec![],
            current_word: 0,
            started_at: None,
            ended_at: None,
            correct_chars: 0,
            total_typed_chars: 0,
            backspaces: 0,
            custom_file: effective_custom_file,
            should_quit: false,
            error_message: None,
            tick_count: 0,
            theme: palette,
            word_pool,
            word_decor,
            app_state,
            key_hits: HashMap::new(),
            key_misses: HashMap::new(),
            stats_cache: None,
            aggregate: {
                let mut agg = storage::load_aggregate();
                // One-time O(n) rebuild when upgrading from a version that
                // predates aggregate.json — after this the file exists and
                // subsequent startups are O(1).
                if agg.key_hits.is_empty() && agg.key_misses.is_empty() {
                    if let Ok(sessions) = storage::load_sessions() {
                        for s in &sessions {
                            storage::apply_session_to_aggregate(&mut agg, s);
                        }
                        if !agg.key_hits.is_empty() || !agg.key_misses.is_empty() {
                            storage::save_aggregate(&agg);
                        }
                    }
                }
                agg
            },
        }
    }

    /// Record that the user just ran a custom-file session against `path` and
    /// persist that to `~/.typerush/state.json` so the next launch's "Custom"
    /// menu row points back at it. Best-effort.
    pub fn remember_custom_file(&mut self, path: &str) {
        self.app_state.last_custom_file = Some(path.to_string());
        self.custom_file = Some(path.to_string());
        state::save_state(&self.app_state);
        // Rebuild the menu so the Custom row's label reflects the new path.
        let snippets = crate::words::snippets::discover_snippets();
        self.menu = build_menu(&snippets, Some(path));
    }

    /// How long the user has been (or was) typing during the current session.
    /// Returns `Duration::ZERO` until the first keypress.
    pub fn elapsed(&self) -> Duration {
        match (self.started_at, self.ended_at) {
            (Some(start), Some(end)) => end.duration_since(start),
            (Some(start), None) => start.elapsed(),
            _ => Duration::ZERO,
        }
    }

    /// Convenience: `elapsed()` expressed in minutes (for the WPM formula).
    pub fn elapsed_minutes(&self) -> f64 {
        self.elapsed().as_secs_f64() / 60.0
    }

    /// Live (or final) words-per-minute.
    ///
    /// Uses the industry-standard formula: a "word" is 5 characters, so
    /// `wpm = (correct_chars / 5) / minutes_elapsed`.
    pub fn wpm(&self) -> f64 {
        let minutes = self.elapsed_minutes();
        if minutes <= 0.0 {
            return 0.0;
        }
        (self.correct_chars as f64 / 5.0) / minutes
    }

    /// Accuracy as a percentage: `correct_chars / total_typed_chars * 100`.
    /// Returns 100% before the user has typed anything.
    pub fn accuracy(&self) -> f64 {
        if self.total_typed_chars == 0 {
            return 100.0;
        }
        (self.correct_chars as f64 / self.total_typed_chars as f64) * 100.0
    }

    /// In `Mode::Time`, how long is left on the clock. `None` for all other modes.
    pub fn time_remaining(&self) -> Option<Duration> {
        if let Mode::Time(seconds) = self.mode {
            let total = Duration::from_secs(seconds);
            let elapsed = self.elapsed();
            if elapsed >= total {
                Some(Duration::ZERO)
            } else {
                Some(total - elapsed)
            }
        } else {
            None
        }
    }

    /// In `Mode::Words` / `Mode::Symbols`, `(items_completed, items_total)`.
    /// `None` for all other modes.
    pub fn progress(&self) -> Option<(usize, usize)> {
        match self.mode {
            Mode::Words(target) | Mode::Symbols(target) => {
                Some((self.current_word.min(target), target))
            }
            _ => None,
        }
    }

    /// Reset all session state and load a fresh word list for `mode`.
    ///
    /// On success the app is left on `Screen::Typing` with the timer un-armed
    /// (it'll start on the first keystroke). Returns `Err` if the chosen mode
    /// can't be initialised (e.g. `--file` was passed an empty file).
    pub fn start_game(&mut self, mode: Mode) -> anyhow::Result<()> {
        use crate::words;
        self.mode = mode;
        let pool = self.word_pool;
        let decor = self.word_decor;
        self.words = match mode {
            // Time mode just needs *enough* words that no one runs out.
            Mode::Time(_) => words::random_words_from(300, pool, decor)
                .into_iter()
                .map(Word::new)
                .collect(),
            Mode::Words(count) => words::random_words_from(count, pool, decor)
                .into_iter()
                .map(Word::new)
                .collect(),
            Mode::Quote => words::random_quote().into_iter().map(Word::new).collect(),
            Mode::Code(lang) => words::random_code_snippet(lang)
                .into_iter()
                .map(Word::new)
                .collect(),
            // Zen intentionally ignores decoration toggles to stay calm.
            Mode::Zen => words::random_words_from(500, pool, WordDecor::default())
                .into_iter()
                .map(Word::new)
                .collect(),
            Mode::Custom => {
                if let Some(path) = &self.custom_file {
                    let loaded = words::words_from_file(path)?;
                    if loaded.is_empty() {
                        return Err(anyhow::anyhow!("custom file is empty"));
                    }
                    loaded.into_iter().map(Word::new).collect()
                } else {
                    return Err(anyhow::anyhow!(
                        "no custom file. Pass --file <path> or drop a .txt in ~/.typerush/snippets/"
                    ));
                }
            }
            Mode::Symbols(count) => words::random_symbol_tokens(count)
                .into_iter()
                .map(Word::new)
                .collect(),
        };
        self.current_word = 0;
        self.started_at = None;
        self.ended_at = None;
        self.correct_chars = 0;
        self.total_typed_chars = 0;
        self.backspaces = 0;
        self.key_hits.clear();
        self.key_misses.clear();
        self.screen = Screen::Typing;
        Ok(())
    }

    /// Restart the most recent mode with a fresh word list.
    pub fn restart(&mut self) -> anyhow::Result<()> {
        let mode = self.mode;
        self.start_game(mode)
    }

    /// Arm the session timer on the user's first keystroke. Idempotent.
    pub fn ensure_timer_started(&mut self) {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
    }

    /// Stop the timer and move to the results screen. Safe to call twice.
    pub fn finish_game(&mut self) {
        if self.ended_at.is_none() {
            self.ended_at = Some(Instant::now());
        }
        self.screen = Screen::Results;
    }

    /// Handle a single visible character keypress (anything that isn't a
    /// control key — letters, digits, punctuation, and the space bar).
    ///
    /// Space is special: it submits the current word and advances.
    pub fn handle_char(&mut self, typed_char: char) {
        self.ensure_timer_started();
        if self.current_word >= self.words.len() {
            return;
        }

        // The space bar is the "submit this word" key.
        if typed_char == ' ' {
            self.submit_word();
            return;
        }

        let active_word = &mut self.words[self.current_word];
        let target_chars: Vec<char> = active_word.text.chars().collect();
        let cursor_position = active_word.typed.chars().count();

        active_word.typed.push(typed_char);
        self.total_typed_chars += 1;

        // Only chars that match the target's expected character at the cursor
        // count as "correct". Anything else (wrong letter or typed past the
        // end of the target word) does NOT add to correct_chars.
        if let Some(&expected_char) = target_chars.get(cursor_position) {
            if expected_char == typed_char {
                self.correct_chars += 1;
                *self.key_hits.entry(typed_char).or_insert(0) += 1;
            } else {
                *self.key_misses.entry(typed_char).or_insert(0) += 1;
            }
        } else {
            // Extra character typed past the end of the target word.
            *self.key_misses.entry(typed_char).or_insert(0) += 1;
        }
    }

    /// Handle backspace. With `delete_whole_word == true` (Ctrl/Alt+Backspace)
    /// erase everything typed for the current word; otherwise erase one char.
    ///
    /// If the current word is already empty and the user is not on the first
    /// word, walks the cursor back to the previous word so they can fix a typo
    /// in a word they've already submitted.
    pub fn handle_backspace(&mut self, delete_whole_word: bool) {
        if self.current_word >= self.words.len() {
            return;
        }
        let active_word = &mut self.words[self.current_word];

        // Cursor at the very start of the current word → step back to the previous one.
        if active_word.typed.is_empty() {
            if self.current_word > 0 {
                self.current_word -= 1;
                self.words[self.current_word].submitted = false;
            }
            return;
        }

        if delete_whole_word {
            self.backspaces += active_word.typed.chars().count();
            active_word.typed.clear();
        } else {
            active_word.typed.pop();
            self.backspaces += 1;
        }
    }

    /// Called when the user presses space — submits the current word as final
    /// and advances `current_word`. Will also `finish_game()` if this submission
    /// reaches the configured word/quote/code target.
    fn submit_word(&mut self) {
        if self.current_word >= self.words.len() {
            return;
        }
        // The space itself is a typed character (it occupies a column on screen).
        self.total_typed_chars += 1;

        let active_word = &mut self.words[self.current_word];
        active_word.submitted = true;

        // A "perfectly typed" word means: typed length == target length and
        // every character is Correct. In that case the trailing space also
        // counts as a correct keystroke (matching how most typing trainers score).
        let states = get_char_states(&active_word.text, &active_word.typed);
        let typed_perfectly = !states.is_empty()
            && states.iter().all(|(_, s)| *s == CharState::Correct)
            && active_word.typed.chars().count() == active_word.text.chars().count();
        if typed_perfectly {
            self.correct_chars += 1;
        }

        self.current_word += 1;

        // Word-count modes: stop once the user has hit the target.
        match self.mode {
            Mode::Words(target) | Mode::Symbols(target) => {
                if self.current_word >= target {
                    self.finish_game();
                    return;
                }
            }
            _ => {}
        }
        // Quote / code / custom-file modes: stop when there's nothing left to type.
        if matches!(self.mode, Mode::Quote | Mode::Code(_) | Mode::Custom)
            && self.current_word >= self.words.len()
        {
            self.finish_game();
        }
    }

    /// Called by the main loop every 100ms.
    ///
    /// Increments `tick_count` (for UI animation throttling) and, if we're in
    /// a time-limited mode, ends the game when the timer expires.
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        if self.screen != Screen::Typing {
            return;
        }
        if let Some(remaining) = self.time_remaining() {
            if self.started_at.is_some() && remaining.is_zero() {
                self.finish_game();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn best_menu_match_exact_time() {
        let menu = build_menu(&[], None);
        let index = best_menu_match(&menu, DefaultMode::Time(30));
        assert_eq!(menu[index].label, "Time · 30s");
    }

    #[test]
    fn best_menu_match_exact_words() {
        let menu = build_menu(&[], None);
        let index = best_menu_match(&menu, DefaultMode::Words(100));
        assert_eq!(menu[index].label, "Words · 100");
    }

    #[test]
    fn best_menu_match_falls_back_to_first_time_row() {
        // 45 isn't one of the four standard time rows; we expect the first
        // time row ("Time · 15s") rather than something unrelated.
        let menu = build_menu(&[], None);
        let index = best_menu_match(&menu, DefaultMode::Time(45));
        assert_eq!(menu[index].label, "Time · 15s");
    }

    #[test]
    fn best_menu_match_falls_back_to_first_words_row() {
        let menu = build_menu(&[], None);
        let index = best_menu_match(&menu, DefaultMode::Words(7));
        assert_eq!(menu[index].label, "Words · 10");
    }

    #[test]
    fn best_menu_match_picks_zen_row() {
        let menu = build_menu(&[], None);
        let index = best_menu_match(&menu, DefaultMode::Zen);
        assert_eq!(menu[index].label, "Zen");
    }

    // ── per-key accuracy tracking (v0.3.0) ──────────────────────────────────

    fn make_app_with_word(word: &str) -> App {
        let mut app = App::new(
            None,
            crate::theme::ThemePalette::default(),
            DefaultMode::Time(15),
            Default::default(),
            Default::default(),
        );
        app.screen = Screen::Typing;
        app.mode = Mode::Words(1);
        app.words = vec![Word::new(word.into())];
        app
    }

    #[test]
    fn correct_char_increments_key_hits() {
        let mut app = make_app_with_word("abc");
        app.handle_char('a');
        assert_eq!(*app.key_hits.get(&'a').unwrap_or(&0), 1);
        assert_eq!(*app.key_misses.get(&'a').unwrap_or(&0), 0);
    }

    #[test]
    fn wrong_char_increments_key_misses() {
        let mut app = make_app_with_word("abc");
        app.handle_char('z'); // expected 'a', typed 'z'
        assert_eq!(*app.key_hits.get(&'z').unwrap_or(&0), 0);
        assert_eq!(*app.key_misses.get(&'z').unwrap_or(&0), 1);
    }

    #[test]
    fn extra_char_increments_key_misses() {
        let mut app = make_app_with_word("ab");
        app.handle_char('a');
        app.handle_char('b');
        // Now at position 2, past the end of "ab".
        app.handle_char('x');
        assert_eq!(*app.key_misses.get(&'x').unwrap_or(&0), 1);
        assert_eq!(*app.key_hits.get(&'x').unwrap_or(&0), 0);
    }

    #[test]
    fn key_tracking_accumulates_across_chars() {
        let mut app = make_app_with_word("aaa");
        app.handle_char('a'); // hit
        app.handle_char('a'); // hit
        app.handle_char('b'); // miss (expected 'a', typed 'b')
        assert_eq!(*app.key_hits.get(&'a').unwrap_or(&0), 2);
        assert_eq!(*app.key_misses.get(&'b').unwrap_or(&0), 1);
    }

    #[test]
    fn start_game_clears_key_tracking() {
        let mut app = make_app_with_word("abc");
        app.handle_char('a'); // builds up some key_hits
        assert!(!app.key_hits.is_empty());
        // Restart to clear.
        app.start_game(Mode::Words(1)).unwrap();
        assert!(app.key_hits.is_empty());
        assert!(app.key_misses.is_empty());
    }

    /// Regression: custom-file mode must auto-finish when the user completes
    /// the last word. Previously it sat waiting for Esc, contradicting the
    /// documented behavior in docs/USAGE.md.
    #[test]
    fn custom_mode_finishes_after_last_word() {
        let palette = crate::theme::ThemePalette::default();
        let mut app = App::new(
            None,
            palette,
            DefaultMode::Time(15),
            Default::default(),
            Default::default(),
        );
        app.mode = Mode::Custom;
        app.words = vec![Word::new("hi".into()), Word::new("bye".into())];
        app.screen = Screen::Typing;

        // Type "hi" + space + "bye" + space.
        for ch in "hi bye ".chars() {
            app.handle_char(ch);
        }

        assert_eq!(app.screen, Screen::Results);
        assert!(app.ended_at.is_some());
    }

    // ── v0.4.0 ─────────────────────────────────────────────────────────────

    /// Symbols mode behaves like Words mode: it finishes after the user
    /// submits the configured number of tokens.
    #[test]
    fn symbols_mode_finishes_after_target_tokens() {
        let palette = crate::theme::ThemePalette::default();
        let mut app = App::new(
            None,
            palette,
            DefaultMode::Time(15),
            Default::default(),
            Default::default(),
        );
        app.mode = Mode::Symbols(3);
        app.words = vec![
            Word::new("()".into()),
            Word::new("=>".into()),
            Word::new("{}".into()),
        ];
        app.screen = Screen::Typing;

        for ch in "() => {} ".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.screen, Screen::Results);
    }

    /// `Mode::Symbols` exposes progress() like `Mode::Words` so the gauge
    /// works.
    #[test]
    fn symbols_mode_reports_progress() {
        let palette = crate::theme::ThemePalette::default();
        let mut app = App::new(
            None,
            palette,
            DefaultMode::Time(15),
            Default::default(),
            Default::default(),
        );
        app.mode = Mode::Symbols(10);
        app.current_word = 3;
        assert_eq!(app.progress(), Some((3, 10)));
    }

    /// Per-mode label for Symbols mode is "symbols-N" so per-mode PB tracking
    /// keys by token count.
    #[test]
    fn symbols_mode_label_includes_count() {
        assert_eq!(Mode::Symbols(25).label(), "symbols-25");
        assert_eq!(Mode::Symbols(50).label(), "symbols-50");
    }

    /// Code mode labels stay backwards-compatible: `code-javascript` for the
    /// JS variant (not the slug "js"), and lowercase for new variants.
    #[test]
    fn code_mode_labels_are_stable_for_pb_lookup() {
        use crate::words::CodeLang;
        assert_eq!(Mode::Code(CodeLang::Rust).label(), "code-rust");
        assert_eq!(Mode::Code(CodeLang::Python).label(), "code-python");
        assert_eq!(Mode::Code(CodeLang::JavaScript).label(), "code-javascript");
        assert_eq!(Mode::Code(CodeLang::Go).label(), "code-go");
        assert_eq!(Mode::Code(CodeLang::Java).label(), "code-java");
        assert_eq!(Mode::Code(CodeLang::Sql).label(), "code-sql");
        assert_eq!(Mode::Code(CodeLang::Shell).label(), "code-shell");
    }

    /// The menu must include exactly one row per built-in mode plus
    /// separator rows for visual grouping. Separator rows are decorative;
    /// only one row should match the default-mode pre-selection.
    #[test]
    fn menu_contains_all_new_v04_modes() {
        let menu = build_menu(&[], None);
        let labels: Vec<&str> = menu.iter().map(|m| m.label.as_str()).collect();

        assert!(labels.iter().any(|l| l.contains("Symbols · 25")));
        assert!(labels.iter().any(|l| l.contains("Symbols · 50")));
        assert!(labels.iter().any(|l| l.contains("Code · Go")));
        assert!(labels.iter().any(|l| l.contains("Code · Java")));
        assert!(labels.iter().any(|l| l.contains("Code · SQL")));
        assert!(labels.iter().any(|l| l.contains("Code · Shell")));
        assert!(labels.iter().any(|l| l.starts_with("Custom")));
    }

    /// Visual separators are present in the menu and clearly distinguishable
    /// from start rows by their `MenuAction::Separator` action.
    #[test]
    fn menu_has_visual_section_separators() {
        let menu = build_menu(&[], None);
        let separator_count = menu
            .iter()
            .filter(|m| matches!(m.action, MenuAction::Separator))
            .count();
        // Time, Words, Quote, Code, Symbols, Zen, Custom, More → 8 sections.
        assert!(
            separator_count >= 7,
            "expected ≥7 separators, got {separator_count}"
        );
    }

    /// The custom row labels with the truncated path when one is provided.
    #[test]
    fn custom_row_shows_last_path_when_known() {
        let menu = build_menu(&[], Some("/tmp/my-typing-fodder.txt"));
        let custom = menu
            .iter()
            .find(|m| matches!(m.action, MenuAction::StartCustom))
            .expect("custom row missing");
        assert!(custom.label.contains("Custom"));
        assert!(custom.label.contains(".txt"));
    }

    /// Long paths are truncated in the middle so the file name remains visible.
    #[test]
    fn truncate_for_menu_collapses_long_paths() {
        let long = "/very/deeply/nested/and/long/directory/structure/somewhere/finally/file.txt";
        let truncated = truncate_for_menu(long, 30);
        // Character count — not byte count — since `…` is a 3-byte char.
        let char_count = truncated.chars().count();
        assert!(
            char_count <= 30,
            "got {} chars: {:?}",
            char_count,
            truncated
        );
        assert!(truncated.contains('…'));
        assert!(truncated.ends_with("file.txt"));
    }

    /// Short paths are passed through unchanged.
    #[test]
    fn truncate_for_menu_passes_short_paths_through() {
        let short = "/tmp/x.txt";
        assert_eq!(truncate_for_menu(short, 30), short);
    }

    /// Snippet rows are appended to the menu under the Custom section.
    #[test]
    fn snippets_appear_as_their_own_rows() {
        use crate::words::snippets::Snippet;
        let snippets = vec![
            Snippet {
                name: "alpha".into(),
                path: std::path::PathBuf::from("/tmp/alpha.txt"),
            },
            Snippet {
                name: "beta".into(),
                path: std::path::PathBuf::from("/tmp/beta.txt"),
            },
        ];
        let menu = build_menu(&snippets, None);
        let snippet_rows: Vec<&MenuItem> = menu
            .iter()
            .filter(|m| {
                matches!(m.action, MenuAction::StartCustom) && m.label.starts_with("Snippet ·")
            })
            .collect();
        assert_eq!(snippet_rows.len(), 2);
        assert!(snippet_rows[0].label.contains("alpha"));
        assert!(snippet_rows[1].label.contains("beta"));
        assert_eq!(
            snippet_rows[0].custom_path.as_deref(),
            Some("/tmp/alpha.txt")
        );
    }

    /// Starting a session uses the configured word pool. With the extended
    /// pool the words list comes from the larger dictionary; the test just
    /// confirms `start_game` doesn't crash with either setting.
    #[test]
    fn start_game_with_extended_pool() {
        let palette = crate::theme::ThemePalette::default();
        let mut app = App::new(
            None,
            palette,
            DefaultMode::Words(10),
            WordPool::Extended,
            Default::default(),
        );
        assert_eq!(app.word_pool, WordPool::Extended);
        app.start_game(Mode::Words(10)).unwrap();
        assert_eq!(app.words.len(), 10);
    }

    /// Punctuation decoration produces words that contain non-alphabetic
    /// characters at least some of the time. With a 25% chance per slot,
    /// 200 words is virtually guaranteed.
    #[test]
    fn start_game_with_punctuation_decor_adds_punctuation() {
        let palette = crate::theme::ThemePalette::default();
        let decor = WordDecor {
            punctuation: true,
            numbers: false,
        };
        let mut app = App::new(
            None,
            palette,
            DefaultMode::Words(200),
            WordPool::Common,
            decor,
        );
        app.start_game(Mode::Words(200)).unwrap();
        let any_punct = app.words.iter().any(|w| {
            w.text
                .chars()
                .any(|c| !c.is_ascii_alphanumeric() && !c.is_whitespace())
        });
        assert!(any_punct, "expected at least one punctuated word");
    }

    /// Zen mode ignores decoration toggles to keep the screen calm.
    #[test]
    fn zen_mode_ignores_punctuation_toggle() {
        let palette = crate::theme::ThemePalette::default();
        let decor = WordDecor {
            punctuation: true,
            numbers: true,
        };
        let mut app = App::new(None, palette, DefaultMode::Zen, WordPool::Common, decor);
        app.start_game(Mode::Zen).unwrap();
        let plain = app
            .words
            .iter()
            .all(|w| w.text.chars().all(|c| c.is_ascii_alphabetic()));
        assert!(plain, "zen-mode words were decorated: {:?}", app.words);
    }
}
