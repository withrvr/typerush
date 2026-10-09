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

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use crate::config::load::{CodeLangKind, DefaultMode};
use crate::storage::{ResultsComparison, SessionRecord, StatsSummary};
use crate::theme::ThemePalette;

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
    /// Words sourced from a user-supplied text file (`--file path.txt`).
    Custom,
}

impl Mode {
    /// Short tag used when saving sessions to disk and in the UI ("time-30s",
    /// "words-50", "code-rust", …). Keeping the format stable means old stats
    /// stay readable across releases.
    pub fn label(&self) -> String {
        match self {
            Mode::Time(seconds) => format!("time-{}s", seconds),
            Mode::Words(count) => format!("words-{}", count),
            Mode::Quote => "quote".to_string(),
            Mode::Code(lang) => format!("code-{:?}", lang).to_lowercase(),
            Mode::Zen => "zen".to_string(),
            Mode::Custom => "custom".to_string(),
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
    /// True when a non-space key was typed where the trailing space belongs —
    /// the space is rendered as an error until backspaced.
    pub space_missed: bool,
}

impl Word {
    pub fn new(text: String) -> Self {
        Self {
            text,
            typed: String::new(),
            space_missed: false,
        }
    }
}

/// What a mouse click on a drawn region does. Every click maps onto an
/// existing keyboard action, so the mouse never reaches anything the
/// keyboard can't.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickAction {
    /// Behave exactly like this key press (footer hints).
    Key(KeyCode, KeyModifiers),
    /// Select this menu option and activate it, like ←/→ then Enter.
    Menu(usize),
}

/// One selectable option in the main menu. Consecutive items that share a
/// `group` are drawn under one heading, side by side ("15s  30s  60s  120s").
pub struct MenuItem {
    /// Category the option belongs to — the row's heading.
    pub group: &'static str,
    /// Option text within the row. Same as `group` for single-option rows.
    pub label: &'static str,
    /// Set for "start a game" rows; `None` for rows like "Stats" or "Quit".
    pub mode: Option<Mode>,
    pub action: MenuAction,
}

/// What pressing Enter on a menu item should do.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Start the game in `MenuItem::mode`.
    Start,
    /// Jump to the historical stats screen.
    ShowStats,
    /// Quit the application.
    Quit,
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
        DefaultMode::Zen => Mode::Zen,
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
        )
    });
    family_match.unwrap_or(0)
}

/// Build the default menu shown on startup.
pub fn default_menu() -> Vec<MenuItem> {
    use crate::words::CodeLang;
    let start = |group, label, mode| MenuItem {
        group,
        label,
        mode: Some(mode),
        action: MenuAction::Start,
    };
    vec![
        start("time", "15s", Mode::Time(15)),
        start("time", "30s", Mode::Time(30)),
        start("time", "60s", Mode::Time(60)),
        start("time", "120s", Mode::Time(120)),
        start("words", "10", Mode::Words(10)),
        start("words", "25", Mode::Words(25)),
        start("words", "50", Mode::Words(50)),
        start("words", "100", Mode::Words(100)),
        start("code", "rust", Mode::Code(CodeLang::Rust)),
        start("code", "python", Mode::Code(CodeLang::Python)),
        start("code", "javascript", Mode::Code(CodeLang::JavaScript)),
        start("quote", "quote", Mode::Quote),
        start("zen", "zen", Mode::Zen),
        MenuItem {
            group: "stats",
            label: "stats",
            mode: None,
            action: MenuAction::ShowStats,
        },
        MenuItem {
            group: "quit",
            label: "quit",
            mode: None,
            action: MenuAction::Quit,
        },
    ]
}

/// Index range of the menu row (group) containing `index`.
pub fn menu_row(menu: &[MenuItem], index: usize) -> std::ops::Range<usize> {
    let group = menu[index].group;
    let start = menu[..index]
        .iter()
        .rposition(|item| item.group != group)
        .map_or(0, |i| i + 1);
    let end = menu[index..]
        .iter()
        .position(|item| item.group != group)
        .map_or(menu.len(), |i| index + i);
    start..end
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
    /// spaces (a correctly typed space also counts as a correct char).
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

    // --- per-key accuracy (v0.3.0) ---
    /// Per expected character: how many times it was typed correctly.
    pub key_hits: HashMap<char, u64>,
    /// Per expected character: how many times something else was typed instead.
    pub key_misses: HashMap<char, u64>,

    // --- stats caching (v0.3.0) ---
    /// Full session history, read from disk the first time the Stats or
    /// Results screen needs it and replaced by the updated list on every
    /// save. `None` until first needed (or after a failed save).
    pub stats_cache: Option<Vec<SessionRecord>>,
    /// Summary figures for the Stats screen, computed from `stats_cache`
    /// each time that screen is entered. `None` until the first Stats visit.
    pub stats_summary: Option<StatsSummary>,
    /// What the Results screen compares against, computed when that screen
    /// is entered (after the session is saved).
    pub results_comparison: Option<ResultsComparison>,
    /// Whether the session currently shown on the Results screen was actually
    /// written to stats.json. False for Zen, sub-1-second, and zero-keystroke
    /// sessions (and on disk failure) — the Results screen uses this to know
    /// whether the last history entry is the current session or a previous one.
    pub session_just_saved: bool,

    // --- mouse ---
    /// Clickable regions drawn in the last frame. Rebuilt by `ui::render` on
    /// every frame (hence the `RefCell`: rendering only gets `&App`) and
    /// read by the mouse handler in `main.rs`.
    pub click_targets: RefCell<Vec<(Rect, ClickAction)>>,
    /// Target under the pointer when the left button went down; a release
    /// only acts if it lands on this same target.
    pub pressed_target: Option<ClickAction>,
}

impl App {
    /// Construct a fresh `App` sitting on the main menu.
    ///
    /// `default_mode` (from `~/.typerush/config.toml`) controls which menu row
    /// is pre-selected and what `app.mode` starts as. `palette` is the active
    /// color theme — UI modules read it on every frame.
    pub fn new(
        custom_file: Option<String>,
        palette: ThemePalette,
        default_mode: DefaultMode,
    ) -> Self {
        let menu = default_menu();
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
            custom_file,
            should_quit: false,
            error_message: None,
            tick_count: 0,
            theme: palette,
            key_hits: HashMap::new(),
            key_misses: HashMap::new(),
            stats_cache: None,
            stats_summary: None,
            results_comparison: None,
            session_just_saved: false,
            click_targets: RefCell::new(Vec::new()),
            pressed_target: None,
        }
    }

    /// Menu ↑/↓: jump to the previous/next row (wrapping), keeping the same
    /// column where the target row has one, else its last option.
    pub fn menu_move_row(&mut self, down: bool) {
        let row = menu_row(&self.menu, self.menu_index);
        let column = self.menu_index - row.start;
        let target = if down {
            if row.end == self.menu.len() {
                0
            } else {
                row.end
            }
        } else if row.start == 0 {
            self.menu.len() - 1
        } else {
            row.start - 1
        };
        let target_row = menu_row(&self.menu, target);
        self.menu_index = (target_row.start + column).min(target_row.end - 1);
    }

    /// Menu ←/→: previous/next option within the current row (wrapping).
    pub fn menu_move_column(&mut self, right: bool) {
        let row = menu_row(&self.menu, self.menu_index);
        let len = row.len();
        let column = self.menu_index - row.start;
        let column = if right {
            (column + 1) % len
        } else {
            (column + len - 1) % len
        };
        self.menu_index = row.start + column;
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

    /// In `Mode::Words`, `(words_completed, words_total)`. `None` for all other modes.
    pub fn progress(&self) -> Option<(usize, usize)> {
        if let Mode::Words(target) = self.mode {
            Some((self.current_word.min(target), target))
        } else {
            None
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
        self.words = match mode {
            // Time mode just needs *enough* words that no one runs out.
            Mode::Time(_) => words::random_words(300)
                .into_iter()
                .map(Word::new)
                .collect(),
            Mode::Words(count) => words::random_words(count)
                .into_iter()
                .map(Word::new)
                .collect(),
            Mode::Quote => words::random_quote().into_iter().map(Word::new).collect(),
            Mode::Code(lang) => words::random_code_snippet(lang)
                .into_iter()
                .map(Word::new)
                .collect(),
            Mode::Zen => words::random_words(500)
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
                    return Err(anyhow::anyhow!("no custom file provided"));
                }
            }
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
    /// Pure character check: the text is one stream of characters, the space
    /// between words included. The key is compared with the character under
    /// the cursor — match is correct, anything else is wrong — and the cursor
    /// moves on one slot either way. Space is not special: mid-word it is just
    /// a wrong character.
    pub fn handle_char(&mut self, typed_char: char) {
        self.ensure_timer_started();
        if self.current_word >= self.words.len() {
            return;
        }

        let is_last_word = self.current_word + 1 >= self.words.len();
        let active_word = &mut self.words[self.current_word];
        let cursor_position = active_word.typed.chars().count();
        let word_len = active_word.text.chars().count();
        self.total_typed_chars += 1;

        // Past the end of the word the cursor sits on the space after it.
        let expected_char = active_word.text.chars().nth(cursor_position);
        let target_char = expected_char.unwrap_or(' ');
        let is_correct = typed_char == target_char;
        // Per-key stats are kept for the key that *should* have been pressed.
        let key_counts = if is_correct {
            self.correct_chars += 1;
            &mut self.key_hits
        } else {
            &mut self.key_misses
        };
        *key_counts.entry(target_char).or_insert(0) += 1;

        if expected_char.is_none() {
            // Cursor on the space after the word: that slot is a character too.
            active_word.space_missed = !is_correct;
            self.advance_word();
            return;
        }

        active_word.typed.push(typed_char);

        // Last character of the last word typed: there is no trailing space.
        if is_last_word && cursor_position + 1 == word_len {
            self.advance_word();
        }
    }

    /// Handle backspace. With `delete_whole_word == true` (Ctrl/Alt+Backspace)
    /// erase everything typed for the current word; otherwise erase one char.
    ///
    /// If the current word is already empty and the user is not on the first
    /// word, erases the space before it — the cursor walks back to the previous
    /// word so they can fix a typo in a word they've already typed.
    pub fn handle_backspace(&mut self, delete_whole_word: bool) {
        if self.current_word >= self.words.len() {
            return;
        }
        let active_word = &mut self.words[self.current_word];

        // Cursor at the very start of the current word → step back to the previous one.
        if active_word.typed.is_empty() {
            if self.current_word > 0 {
                self.current_word -= 1;
                self.words[self.current_word].space_missed = false;
                self.backspaces += 1;
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

    /// Moves the cursor to the next word. Will also `finish_game()` if this
    /// reaches the configured word/quote/code target.
    fn advance_word(&mut self) {
        self.current_word += 1;

        // Word-count modes: stop once the user has hit the target.
        if let Mode::Words(target) = self.mode {
            if self.current_word >= target {
                self.finish_game();
                return;
            }
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
        let menu = default_menu();
        let index = best_menu_match(&menu, DefaultMode::Time(30));
        assert_eq!(menu[index].label, "30s");
    }

    #[test]
    fn best_menu_match_exact_words() {
        let menu = default_menu();
        let index = best_menu_match(&menu, DefaultMode::Words(100));
        assert_eq!(menu[index].label, "100");
    }

    #[test]
    fn best_menu_match_falls_back_to_first_time_row() {
        // 45 isn't one of the four standard time rows; we expect the first
        // time row ("Time · 15s") rather than something unrelated.
        let menu = default_menu();
        let index = best_menu_match(&menu, DefaultMode::Time(45));
        assert_eq!(menu[index].label, "15s");
    }

    #[test]
    fn best_menu_match_falls_back_to_first_words_row() {
        let menu = default_menu();
        let index = best_menu_match(&menu, DefaultMode::Words(7));
        assert_eq!(menu[index].label, "10");
    }

    #[test]
    fn best_menu_match_picks_zen_row() {
        let menu = default_menu();
        let index = best_menu_match(&menu, DefaultMode::Zen);
        assert_eq!(menu[index].label, "zen");
    }

    fn menu_app() -> App {
        App::new(
            None,
            crate::theme::ThemePalette::default(),
            DefaultMode::Time(30),
        )
    }

    #[test]
    fn menu_left_right_stays_in_row_and_wraps() {
        let mut app = menu_app(); // time · 30s
        app.menu_move_column(true);
        assert_eq!(app.menu[app.menu_index].label, "60s");
        app.menu_move_column(true);
        app.menu_move_column(true);
        assert_eq!(app.menu[app.menu_index].label, "15s"); // wrapped
        app.menu_move_column(false);
        assert_eq!(app.menu[app.menu_index].label, "120s");
    }

    #[test]
    fn menu_up_down_keeps_column_and_clamps() {
        let mut app = menu_app(); // time · 30s (column 1)
        app.menu_move_row(true);
        assert_eq!(app.menu[app.menu_index].label, "25");
        app.menu_move_row(true);
        assert_eq!(app.menu[app.menu_index].label, "python");
        app.menu_move_row(true);
        assert_eq!(app.menu[app.menu_index].label, "quote"); // clamped
        app.menu_move_row(false);
        assert_eq!(app.menu[app.menu_index].label, "rust");
    }

    #[test]
    fn menu_up_down_wraps_between_first_and_last_row() {
        let mut app = menu_app();
        app.menu_move_row(false);
        assert_eq!(app.menu[app.menu_index].label, "quit");
        app.menu_move_row(true);
        assert_eq!(app.menu[app.menu_index].group, "time");
    }

    // ── per-key accuracy tracking (v0.3.0) ──────────────────────────────────

    fn make_app_with_word(word: &str) -> App {
        let mut app = App::new(
            None,
            crate::theme::ThemePalette::default(),
            DefaultMode::Time(15),
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
        app.handle_char('z'); // expected 'a', typed 'z' — the miss belongs to 'a'
        assert_eq!(*app.key_misses.get(&'a').unwrap_or(&0), 1);
        assert!(!app.key_misses.contains_key(&'z'));
        assert!(app.key_hits.is_empty());
    }

    #[test]
    fn space_slot_tracks_hit_and_miss() {
        let mut app = custom_app(&["ab", "cd"]);
        app.handle_char('a');
        app.handle_char('b');
        app.handle_char('x'); // wrong key where the space belongs
        assert_eq!(*app.key_misses.get(&' ').unwrap_or(&0), 1);
        app.handle_backspace(false);
        app.handle_char(' '); // correct space
        assert_eq!(*app.key_hits.get(&' ').unwrap_or(&0), 1);
    }

    #[test]
    fn key_tracking_accumulates_across_chars() {
        let mut app = make_app_with_word("aaa");
        app.handle_char('a'); // hit
        app.handle_char('a'); // hit
        app.handle_char('b'); // miss on 'a' (expected 'a', typed 'b')
        assert_eq!(*app.key_hits.get(&'a').unwrap_or(&0), 2);
        assert_eq!(*app.key_misses.get(&'a').unwrap_or(&0), 1);
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
        let mut app = App::new(None, palette, DefaultMode::Time(15));
        app.mode = Mode::Custom;
        app.words = vec![Word::new("hi".into()), Word::new("bye".into())];
        app.screen = Screen::Typing;

        // Type "hi" + space + "bye" — the last character ends the run.
        for ch in "hi bye".chars() {
            app.handle_char(ch);
        }

        assert_eq!(app.screen, Screen::Results);
        assert!(app.ended_at.is_some());
        // Every slot right, the space included.
        assert_eq!(app.correct_chars, 6);
        assert_eq!(app.total_typed_chars, 6);
    }

    /// Words mode also ends on the last character, with no trailing space.
    #[test]
    fn words_mode_finishes_on_last_char() {
        let mut app = custom_app(&["hi", "bye"]);
        app.mode = Mode::Words(2);

        for ch in "hi by".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.screen, Screen::Typing);

        app.handle_char('e');
        assert_eq!(app.screen, Screen::Results);
    }

    fn custom_app(words: &[&str]) -> App {
        let palette = crate::theme::ThemePalette::default();
        let mut app = App::new(None, palette, DefaultMode::Time(15));
        app.mode = Mode::Custom;
        app.words = words.iter().map(|w| Word::new((*w).into())).collect();
        app.screen = Screen::Typing;
        app
    }

    /// Regression (#8): a letter typed where the space belongs used to pile up
    /// as extras on the current word. The space is a character like any other:
    /// a wrong key there is a wrong character and the cursor moves on one slot.
    #[test]
    fn wrong_key_on_space_is_wrong_char() {
        let mut app = custom_app(&["hi", "bye"]);

        for ch in "hix".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.words[0].typed, "hi");
        assert!(app.words[0].space_missed);
        assert_eq!(app.current_word, 1);
        assert_eq!(app.words[1].typed, "");

        // Next word lines up; the last char ends the run (no trailing space).
        for ch in "bye".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.correct_chars, 5);
        assert_eq!(app.total_typed_chars, 6);
        assert_eq!(app.screen, Screen::Results);
    }

    /// Space mid-word is just a wrong character — it must not jump words.
    #[test]
    fn space_mid_word_is_wrong_char() {
        let mut app = custom_app(&["hi", "bye"]);

        for ch in "h ".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.words[0].typed, "h ");
        assert_eq!(app.current_word, 0);
        assert_eq!(app.correct_chars, 1);
        assert_eq!(app.total_typed_chars, 2);

        // Cursor is on the space now; the stray 'i' is wrong there too.
        app.handle_char('i');
        assert!(app.words[0].space_missed);
        assert_eq!(app.current_word, 1);
    }

    /// Backspace from the start of a word erases the space before it.
    #[test]
    fn backspace_erases_wrong_space() {
        let mut app = custom_app(&["hi", "bye"]);

        for ch in "hix".chars() {
            app.handle_char(ch);
        }
        app.handle_backspace(false);

        assert_eq!(app.current_word, 0);
        assert_eq!(app.words[0].typed, "hi");
        assert!(!app.words[0].space_missed);
    }
}
