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

use std::time::{Duration, Instant};

use crate::config::load::{CodeLangKind, DefaultMode};
use crate::game::{get_char_states, CharState};
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
    pub label: &'static str,
    /// Set for "start a game" rows; `None` for rows like "Stats" or "Quit".
    pub mode: Option<Mode>,
    pub action: MenuAction,
}

/// What pressing Enter on a menu item should do.
#[derive(Clone, Copy)]
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
    vec![
        MenuItem {
            label: "Time · 15s",
            mode: Some(Mode::Time(15)),
            action: MenuAction::Start,
        },
        MenuItem {
            label: "Time · 30s",
            mode: Some(Mode::Time(30)),
            action: MenuAction::Start,
        },
        MenuItem {
            label: "Time · 60s",
            mode: Some(Mode::Time(60)),
            action: MenuAction::Start,
        },
        MenuItem {
            label: "Time · 120s",
            mode: Some(Mode::Time(120)),
            action: MenuAction::Start,
        },
        MenuItem {
            label: "Words · 10",
            mode: Some(Mode::Words(10)),
            action: MenuAction::Start,
        },
        MenuItem {
            label: "Words · 25",
            mode: Some(Mode::Words(25)),
            action: MenuAction::Start,
        },
        MenuItem {
            label: "Words · 50",
            mode: Some(Mode::Words(50)),
            action: MenuAction::Start,
        },
        MenuItem {
            label: "Words · 100",
            mode: Some(Mode::Words(100)),
            action: MenuAction::Start,
        },
        MenuItem {
            label: "Quote",
            mode: Some(Mode::Quote),
            action: MenuAction::Start,
        },
        MenuItem {
            label: "Code · Rust",
            mode: Some(Mode::Code(CodeLang::Rust)),
            action: MenuAction::Start,
        },
        MenuItem {
            label: "Code · Python",
            mode: Some(Mode::Code(CodeLang::Python)),
            action: MenuAction::Start,
        },
        MenuItem {
            label: "Code · JavaScript",
            mode: Some(Mode::Code(CodeLang::JavaScript)),
            action: MenuAction::Start,
        },
        MenuItem {
            label: "Zen",
            mode: Some(Mode::Zen),
            action: MenuAction::Start,
        },
        MenuItem {
            label: "Stats",
            mode: None,
            action: MenuAction::ShowStats,
        },
        MenuItem {
            label: "Quit",
            mode: None,
            action: MenuAction::Quit,
        },
    ]
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
        }
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
            }
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
        assert_eq!(menu[index].label, "Time · 30s");
    }

    #[test]
    fn best_menu_match_exact_words() {
        let menu = default_menu();
        let index = best_menu_match(&menu, DefaultMode::Words(100));
        assert_eq!(menu[index].label, "Words · 100");
    }

    #[test]
    fn best_menu_match_falls_back_to_first_time_row() {
        // 45 isn't one of the four standard time rows; we expect the first
        // time row ("Time · 15s") rather than something unrelated.
        let menu = default_menu();
        let index = best_menu_match(&menu, DefaultMode::Time(45));
        assert_eq!(menu[index].label, "Time · 15s");
    }

    #[test]
    fn best_menu_match_falls_back_to_first_words_row() {
        let menu = default_menu();
        let index = best_menu_match(&menu, DefaultMode::Words(7));
        assert_eq!(menu[index].label, "Words · 10");
    }

    #[test]
    fn best_menu_match_picks_zen_row() {
        let menu = default_menu();
        let index = best_menu_match(&menu, DefaultMode::Zen);
        assert_eq!(menu[index].label, "Zen");
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

        // Type "hi" + space + "bye" + space.
        for ch in "hi bye ".chars() {
            app.handle_char(ch);
        }

        assert_eq!(app.screen, Screen::Results);
        assert!(app.ended_at.is_some());
    }
}
