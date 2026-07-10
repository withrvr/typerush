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
    /// In-app settings: theme picker, default-mode picker, reset (v0.5.0).
    Settings,
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
    /// The daily challenge (v0.5.0): a deterministic seed-of-the-day word
    /// list — everyone gets the same words on the same date. The date is
    /// stored so the saved label (`daily-YYYY-MM-DD`) pins the PB to that
    /// day's challenge.
    Daily(chrono::NaiveDate),
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
            Mode::Daily(date) => format!("daily-{}", date.format("%Y-%m-%d")),
        }
    }

    /// Inverse of [`Mode::label`]: parse a saved label back into a runtime
    /// `Mode` so a recorded session can be replayed with its original timer /
    /// completion semantics (v0.5.0). Returns `None` for labels this version
    /// doesn't recognise (e.g. from a future release).
    pub fn parse_label(label: &str) -> Option<Mode> {
        use crate::words::CodeLang;
        match label {
            "quote" => return Some(Mode::Quote),
            "zen" => return Some(Mode::Zen),
            "custom" => return Some(Mode::Custom),
            _ => {}
        }
        if let Some(rest) = label.strip_prefix("time-") {
            return rest.strip_suffix('s')?.parse().ok().map(Mode::Time);
        }
        if let Some(rest) = label.strip_prefix("words-") {
            return rest.parse().ok().map(Mode::Words);
        }
        if let Some(rest) = label.strip_prefix("symbols-") {
            return rest.parse().ok().map(Mode::Symbols);
        }
        if let Some(rest) = label.strip_prefix("daily-") {
            return chrono::NaiveDate::parse_from_str(rest, "%Y-%m-%d")
                .ok()
                .map(Mode::Daily);
        }
        if let Some(rest) = label.strip_prefix("code-") {
            let lang = match rest {
                "rust" => CodeLang::Rust,
                "python" => CodeLang::Python,
                "javascript" => CodeLang::JavaScript,
                "go" => CodeLang::Go,
                "java" => CodeLang::Java,
                "sql" => CodeLang::Sql,
                "shell" => CodeLang::Shell,
                _ => return None,
            };
            return Some(Mode::Code(lang));
        }
        None
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
    /// Open the in-app settings screen (v0.5.0).
    ShowSettings,
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
        separator("── Daily ──"),
        start_row(
            "Daily challenge",
            Mode::Daily(chrono::Local::now().date_naive()),
        ),
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
        label: "Settings".to_string(),
        mode: None,
        action: MenuAction::ShowSettings,
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

/// Number of rows on the Settings screen: theme, default mode, reset.
pub const SETTINGS_ROWS: usize = 3;

/// Persist a default-mode preset to the config file: `defaults.mode` plus
/// whichever paired key (`time_seconds`, `word_count`, `code_lang`,
/// `symbol_count`) pins the preset down. Quote and Zen need only the mode.
fn persist_default_mode(
    config_path: &std::path::Path,
    default_mode: DefaultMode,
) -> Result<(), String> {
    use crate::config::edit::set_value;
    let (mode_name, paired) = match default_mode {
        DefaultMode::Time(seconds) => {
            ("time", Some(("defaults.time_seconds", seconds.to_string())))
        }
        DefaultMode::Words(count) => ("words", Some(("defaults.word_count", count.to_string()))),
        DefaultMode::Quote => ("quote", None),
        DefaultMode::Code(lang) => {
            let slug = match lang {
                CodeLangKind::Rust => "rust",
                CodeLangKind::Python => "python",
                CodeLangKind::JavaScript => "js",
                CodeLangKind::Go => "go",
                CodeLangKind::Java => "java",
                CodeLangKind::Sql => "sql",
                CodeLangKind::Shell => "shell",
            };
            ("code", Some(("defaults.code_lang", slug.to_string())))
        }
        DefaultMode::Zen => ("zen", None),
        DefaultMode::Symbols(count) => (
            "symbols",
            Some(("defaults.symbol_count", count.to_string())),
        ),
    };
    set_value(config_path, "defaults.mode", mode_name)?;
    if let Some((key, value)) = paired {
        set_value(config_path, key, &value)?;
    }
    Ok(())
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
    /// Highlight index in the Stats screen's recent-sessions table (v0.5.0).
    /// 0 = the newest displayed session; reset on each Stats entry.
    pub stats_selected: usize,
    /// Highlight index on the Settings screen (v0.5.0): 0 = theme,
    /// 1 = default mode, 2 = reset to defaults.
    pub settings_index: usize,
    /// Name of the active theme ("dark", "monokai", …) for the Settings
    /// screen. `"custom"` when the palette doesn't match any built-in (the
    /// user has per-slot `[colors]` overrides).
    pub theme_name: String,
    /// The current default-mode selection, shown and cycled on the Settings
    /// screen and persisted to `defaults.mode` in the config.
    pub default_mode: DefaultMode,

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
    /// When the current pause began (v0.5.0). `Some` while the session is
    /// paused; `None` while typing normally.
    pub paused_at: Option<Instant>,
    /// Total time spent paused during this session, excluding any pause that
    /// is still in progress. Subtracted from `elapsed()` so WPM and the
    /// time-mode countdown freeze while paused.
    pub total_paused: Duration,

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
    /// User snippet library discovered at startup from
    /// `~/.typerush/snippets/*.txt`. Held here so the menu can be rebuilt
    /// (e.g. when the Custom row label changes) without re-scanning the disk.
    pub snippets: Vec<Snippet>,

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
        // Recover the theme's name from its palette for the Settings screen.
        // Per-slot [colors] overrides produce a palette that matches no
        // built-in — shown as "custom" until the user picks a theme.
        let theme_name = crate::theme::builtin::ALL
            .iter()
            .find(|(_, p)| *p == palette)
            .map(|(name, _)| (*name).to_string())
            .unwrap_or_else(|| "custom".to_string());
        Self {
            screen: Screen::Menu,
            previous_screen: Screen::Menu,
            menu,
            menu_index,
            stats_selected: 0,
            settings_index: 0,
            theme_name,
            default_mode,
            mode: initial_mode,
            words: vec![],
            current_word: 0,
            started_at: None,
            ended_at: None,
            paused_at: None,
            total_paused: Duration::ZERO,
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
            snippets,
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
    ///
    /// Re-uses the snippet list cached on `App` rather than re-scanning the
    /// snippets directory — snippets don't change while the app is running,
    /// and Enter-on-a-snippet is on the keystroke path.
    pub fn remember_custom_file(&mut self, path: &str) {
        let owned = path.to_string();
        self.custom_file = Some(owned.clone());
        self.app_state.last_custom_file = Some(owned);
        state::save_state(&self.app_state);
        // Preserve the highlight on whichever row the user is currently on
        // (e.g. the snippet row they just pressed Enter on). `build_menu`
        // produces a stable row order, so the index stays valid.
        let previous_index = self.menu_index;
        self.menu = build_menu(&self.snippets, self.custom_file.as_deref());
        if previous_index < self.menu.len() {
            self.menu_index = previous_index;
        }
    }

    // ── Settings screen (v0.5.0) ─────────────────────────────────────────────

    /// Cycle the value of the highlighted Settings row by `delta` (±1).
    /// `config_path` is where changes persist — parameterized so tests can
    /// point at a temp file instead of the user's real config.
    pub fn settings_cycle(&mut self, delta: i32, config_path: &std::path::Path) {
        match self.settings_index {
            0 => self.cycle_theme(delta, config_path),
            1 => self.cycle_default_mode(delta, config_path),
            _ => {}
        }
    }

    /// Enter on the highlighted Settings row: pickers advance to the next
    /// value; the reset row restores defaults.
    pub fn settings_activate(&mut self, config_path: &std::path::Path) {
        match self.settings_index {
            0 | 1 => self.settings_cycle(1, config_path),
            2 => self.reset_settings(config_path),
            _ => {}
        }
    }

    /// Switch to the next/previous built-in theme, apply it to the live UI,
    /// and persist `theme = "<name>"`. A "custom" palette (per-slot overrides
    /// in the config) enters the cycle at its first entry; the overrides
    /// themselves stay in the file and re-apply on top of the newly chosen
    /// theme at next launch.
    fn cycle_theme(&mut self, delta: i32, config_path: &std::path::Path) {
        use crate::theme::builtin;
        let names: Vec<&str> = builtin::names();
        let len = names.len() as i32;
        let next_index = match names.iter().position(|n| *n == self.theme_name) {
            Some(current) => (current as i32 + delta).rem_euclid(len) as usize,
            // Not on a built-in (custom palette): start from the first entry.
            None => 0,
        };
        let (name, palette) = builtin::ALL[next_index];
        self.theme = palette;
        self.theme_name = name.to_string();
        if let Err(message) = crate::config::edit::set_value(config_path, "theme", name) {
            self.error_message = Some(message);
        }
    }

    /// The canonical default-mode choices offered by the Settings picker —
    /// one per standard menu row.
    pub fn default_mode_presets() -> Vec<DefaultMode> {
        use CodeLangKind::*;
        vec![
            DefaultMode::Time(15),
            DefaultMode::Time(30),
            DefaultMode::Time(60),
            DefaultMode::Time(120),
            DefaultMode::Words(10),
            DefaultMode::Words(25),
            DefaultMode::Words(50),
            DefaultMode::Words(100),
            DefaultMode::Quote,
            DefaultMode::Code(Rust),
            DefaultMode::Code(Python),
            DefaultMode::Code(JavaScript),
            DefaultMode::Code(Go),
            DefaultMode::Code(Java),
            DefaultMode::Code(Sql),
            DefaultMode::Code(Shell),
            DefaultMode::Zen,
            DefaultMode::Symbols(25),
            DefaultMode::Symbols(50),
        ]
    }

    /// Display label for a default-mode preset — same strings as the saved
    /// session labels ("time-30s", "code-rust", …).
    pub fn default_mode_label(default_mode: DefaultMode) -> String {
        mode_for(default_mode).label()
    }

    /// Switch to the next/previous default-mode preset, move the menu
    /// highlight to the matching row, and persist `defaults.mode` plus the
    /// paired count/lang key.
    fn cycle_default_mode(&mut self, delta: i32, config_path: &std::path::Path) {
        let presets = Self::default_mode_presets();
        let len = presets.len() as i32;
        let next_index = match presets.iter().position(|p| *p == self.default_mode) {
            Some(current) => (current as i32 + delta).rem_euclid(len) as usize,
            // A non-standard configured default (e.g. time_seconds = 45)
            // enters the cycle at its first entry.
            None => 0,
        };
        self.default_mode = presets[next_index];
        self.menu_index = best_menu_match(&self.menu, self.default_mode);
        if let Err(message) = persist_default_mode(config_path, self.default_mode) {
            self.error_message = Some(message);
        }
    }

    /// Reset-to-defaults row: clear the config file (backed up to
    /// `config.toml.bak` by `config::edit::reset`) and return the live app to
    /// the built-in dark theme and time-15s default.
    fn reset_settings(&mut self, config_path: &std::path::Path) {
        if let Err(message) = crate::config::edit::reset(config_path) {
            self.error_message = Some(message);
            return;
        }
        self.theme = crate::theme::builtin::DARK;
        self.theme_name = "dark".to_string();
        self.default_mode = DefaultMode::Time(15);
        self.word_pool = WordPool::default();
        self.word_decor = WordDecor::default();
        self.menu_index = best_menu_match(&self.menu, self.default_mode);
    }

    /// How long the user has been (or was) actively typing during the current
    /// session. Time spent paused is excluded, so WPM and the time-mode
    /// countdown freeze while paused. Returns `Duration::ZERO` until the
    /// first keypress.
    pub fn elapsed(&self) -> Duration {
        let wall = match (self.started_at, self.ended_at) {
            (Some(start), Some(end)) => end.duration_since(start),
            (Some(start), None) => start.elapsed(),
            _ => return Duration::ZERO,
        };
        // Subtract completed pauses plus the pause currently in progress.
        let paused = self.total_paused
            + self
                .paused_at
                .map(|p| p.elapsed())
                .unwrap_or(Duration::ZERO);
        wall.saturating_sub(paused)
    }

    /// Whether the session is currently paused (v0.5.0).
    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    /// Pause or resume the current session (v0.5.0, `Ctrl+P` while typing).
    ///
    /// A no-op before the first keystroke (there is no running timer to
    /// pause yet) and after the session has ended.
    pub fn toggle_pause(&mut self) {
        if self.started_at.is_none() || self.ended_at.is_some() {
            return;
        }
        match self.paused_at.take() {
            // Resuming: bank the pause span we just finished.
            Some(paused_at) => self.total_paused += paused_at.elapsed(),
            // Pausing: stamp the pause start.
            None => self.paused_at = Some(Instant::now()),
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

    /// In `Mode::Words` / `Mode::Symbols` / `Mode::Daily`,
    /// `(items_completed, items_total)`. `None` for all other modes.
    pub fn progress(&self) -> Option<(usize, usize)> {
        match self.mode {
            Mode::Words(target) | Mode::Symbols(target) => {
                Some((self.current_word.min(target), target))
            }
            // The daily challenge's target is simply its (fixed) word list.
            Mode::Daily(_) => {
                let target = self.words.len();
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
            // Deterministic seed-of-the-day list — same words for everyone
            // on `date`, independent of pool / decoration settings.
            Mode::Daily(date) => words::daily_words(date)
                .into_iter()
                .map(Word::new)
                .collect(),
        };
        self.reset_session_counters();
        Ok(())
    }

    /// Start a session that replays a previously recorded word list
    /// word-for-word (v0.5.0). `mode` should be the original session's mode
    /// so the timer / completion semantics and the saved label match the
    /// first run.
    ///
    /// Returns `Err` for an empty word list — records saved before v0.5.0
    /// don't carry replay data.
    pub fn start_replay(&mut self, mode: Mode, words: Vec<String>) -> anyhow::Result<()> {
        if words.is_empty() {
            return Err(anyhow::anyhow!(
                "no replay data for this session (recorded before v0.5.0)"
            ));
        }
        self.mode = mode;
        self.words = words.into_iter().map(Word::new).collect();
        self.reset_session_counters();
        Ok(())
    }

    /// Zero every per-session counter and land on the typing screen with the
    /// timer un-armed. Shared tail of `start_game` and `start_replay`.
    fn reset_session_counters(&mut self) {
        self.current_word = 0;
        self.started_at = None;
        self.ended_at = None;
        self.paused_at = None;
        self.total_paused = Duration::ZERO;
        self.correct_chars = 0;
        self.total_typed_chars = 0;
        self.backspaces = 0;
        self.key_hits.clear();
        self.key_misses.clear();
        self.screen = Screen::Typing;
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
    ///
    /// If the session is paused when it ends (Esc pressed mid-pause), the
    /// open pause is folded into `total_paused` first so the final elapsed
    /// time stays frozen at the moment the pause began.
    pub fn finish_game(&mut self) {
        if let Some(paused_at) = self.paused_at.take() {
            self.total_paused += paused_at.elapsed();
        }
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
        // Quote / code / custom-file / daily modes: stop when there's nothing
        // left to type.
        if matches!(
            self.mode,
            Mode::Quote | Mode::Code(_) | Mode::Custom | Mode::Daily(_)
        ) && self.current_word >= self.words.len()
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

    /// `remember_custom_file` updates the persisted path and the menu's
    /// Custom row label, and preserves the user's current menu position.
    /// Calling it twice in a row also works without re-rebuilding the menu
    /// from a stale snippet snapshot.
    #[test]
    fn remember_custom_file_updates_menu_and_state() {
        let palette = crate::theme::ThemePalette::default();
        let mut app = App::new(
            None,
            palette,
            DefaultMode::Time(15),
            Default::default(),
            Default::default(),
        );
        let original_index = app.menu_index;
        app.remember_custom_file("/tmp/abc.txt");
        assert_eq!(app.custom_file.as_deref(), Some("/tmp/abc.txt"));
        assert_eq!(
            app.app_state.last_custom_file.as_deref(),
            Some("/tmp/abc.txt")
        );
        // Menu index is preserved (build_menu produces a stable row order).
        assert_eq!(app.menu_index, original_index);
        // The custom row reflects the new path.
        let custom_row = app
            .menu
            .iter()
            .find(|m| matches!(m.action, MenuAction::StartCustom) && m.custom_path.is_none())
            .expect("custom row missing");
        assert!(custom_row.label.contains("abc.txt"));

        // Second call rotates to a different path without going stale.
        app.remember_custom_file("/tmp/xyz.txt");
        let custom_row = app
            .menu
            .iter()
            .find(|m| matches!(m.action, MenuAction::StartCustom) && m.custom_path.is_none())
            .expect("custom row missing");
        assert!(custom_row.label.contains("xyz.txt"));
        assert!(!custom_row.label.contains("abc.txt"));
    }

    // ── v0.5.0: pause / resume ──────────────────────────────────────────────

    /// Pausing before the first keystroke is a no-op — there's no running
    /// timer to pause yet.
    #[test]
    fn pause_before_first_key_is_noop() {
        let mut app = make_app_with_word("abc");
        app.toggle_pause();
        assert!(!app.is_paused());
        assert_eq!(app.elapsed(), Duration::ZERO);
    }

    /// Ctrl+P toggles: pause sets `paused_at`, resume banks the pause span
    /// into `total_paused` and clears it.
    #[test]
    fn toggle_pause_sets_and_clears() {
        let mut app = make_app_with_word("abc");
        app.handle_char('a'); // arms the timer
        app.toggle_pause();
        assert!(app.is_paused());
        app.toggle_pause();
        assert!(!app.is_paused());
    }

    /// While paused, `elapsed()` is frozen: the in-flight pause span is
    /// subtracted from wall-clock time.
    #[test]
    fn elapsed_is_frozen_while_paused() {
        let mut app = make_app_with_word("abc");
        // Simulate a session that started 2s ago and has been paused the
        // whole time — deterministic, no sleeps.
        app.started_at = Some(Instant::now() - Duration::from_secs(2));
        app.paused_at = app.started_at;
        assert!(app.elapsed() < Duration::from_millis(100));
    }

    /// A paused time-mode session must never be finished by `tick()` — the
    /// countdown is frozen along with `elapsed()`.
    #[test]
    fn tick_does_not_finish_paused_time_session() {
        let mut app = make_app_with_word("abc");
        app.mode = Mode::Time(1);
        // Started 5s ago (well past the 1s limit) but paused from the start.
        app.started_at = Some(Instant::now() - Duration::from_secs(5));
        app.paused_at = app.started_at;
        app.tick();
        assert_eq!(app.screen, Screen::Typing);
        assert!(app.ended_at.is_none());
    }

    /// Ending a session mid-pause folds the open pause first, so the final
    /// elapsed time stays frozen at the moment the pause began.
    #[test]
    fn finish_while_paused_folds_open_pause() {
        let mut app = make_app_with_word("abc");
        // Typed for ~2s, then paused for ~1s, then finished.
        app.started_at = Some(Instant::now() - Duration::from_secs(3));
        app.paused_at = Some(Instant::now() - Duration::from_secs(1));
        app.finish_game();
        assert!(!app.is_paused());
        let secs = app.elapsed().as_secs_f64();
        assert!(
            (1.9..=2.1).contains(&secs),
            "expected ~2s of active typing, got {secs}"
        );
    }

    /// Restarting a session clears any leftover pause state.
    #[test]
    fn start_game_clears_pause_state() {
        let mut app = make_app_with_word("abc");
        app.handle_char('a');
        app.toggle_pause();
        assert!(app.is_paused());
        app.start_game(Mode::Words(1)).unwrap();
        assert!(!app.is_paused());
        assert_eq!(app.total_paused, Duration::ZERO);
    }

    // ── v0.5.0: settings screen ─────────────────────────────────────────────

    fn make_settings_app() -> (tempfile::TempDir, std::path::PathBuf, App) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let app = App::new(
            None,
            crate::theme::ThemePalette::default(),
            DefaultMode::Time(15),
            Default::default(),
            Default::default(),
        );
        (dir, path, app)
    }

    /// Cycling the theme applies the palette live, tracks the name, and
    /// persists `theme = "<name>"` to the config file.
    #[test]
    fn settings_theme_cycle_applies_and_persists() {
        use crate::theme::builtin;
        let (_dir, path, mut app) = make_settings_app();
        assert_eq!(app.theme_name, "dark");
        app.settings_index = 0;
        app.settings_cycle(1, &path);
        assert_eq!(app.theme_name, "light");
        assert_eq!(app.theme, builtin::LIGHT);
        assert_eq!(
            crate::config::edit::get_value(&path, "theme")
                .unwrap()
                .as_deref(),
            Some("light")
        );
    }

    /// The theme picker wraps around in both directions.
    #[test]
    fn settings_theme_cycle_wraps() {
        use crate::theme::builtin;
        let (_dir, path, mut app) = make_settings_app();
        app.settings_index = 0;
        // dark → (back one) → last theme in the list.
        app.settings_cycle(-1, &path);
        let last = builtin::ALL.last().unwrap();
        assert_eq!(app.theme_name, last.0);
        // …and forward again returns to dark.
        app.settings_cycle(1, &path);
        assert_eq!(app.theme_name, "dark");
    }

    /// A "custom" palette (per-slot overrides) starts the cycle at the first
    /// built-in instead of panicking or refusing.
    #[test]
    fn settings_theme_cycle_from_custom_palette() {
        let (_dir, path, mut app) = make_settings_app();
        app.theme_name = "custom".to_string();
        app.settings_index = 0;
        app.settings_cycle(1, &path);
        assert_eq!(app.theme_name, "dark");
    }

    /// Cycling the default mode persists `defaults.mode` + the paired key
    /// and moves the menu highlight to the matching row.
    #[test]
    fn settings_default_mode_cycle_persists_and_moves_menu() {
        let (_dir, path, mut app) = make_settings_app();
        app.settings_index = 1;
        // Time(15) → Time(30).
        app.settings_cycle(1, &path);
        assert_eq!(app.default_mode, DefaultMode::Time(30));
        assert_eq!(app.menu[app.menu_index].label, "Time · 30s");
        assert_eq!(
            crate::config::edit::get_value(&path, "defaults.mode")
                .unwrap()
                .as_deref(),
            Some("time")
        );
        assert_eq!(
            crate::config::edit::get_value(&path, "defaults.time_seconds")
                .unwrap()
                .as_deref(),
            Some("30")
        );
    }

    /// Every preset persists cleanly through the config editor (mode name +
    /// paired key valid for the loader).
    #[test]
    fn settings_every_default_mode_preset_persists() {
        let (_dir, path, mut app) = make_settings_app();
        app.settings_index = 1;
        for _ in 0..App::default_mode_presets().len() {
            app.settings_cycle(1, &path);
            assert!(
                app.error_message.is_none(),
                "persisting {:?} failed: {:?}",
                app.default_mode,
                app.error_message
            );
        }
        // Full lap lands back on the starting preset.
        assert_eq!(app.default_mode, DefaultMode::Time(15));
    }

    /// The reset row clears the file (with backup) and restores the live app
    /// to dark / time-15s.
    #[test]
    fn settings_reset_restores_defaults() {
        use crate::theme::builtin;
        let (_dir, path, mut app) = make_settings_app();
        // Dirty the state first.
        app.settings_index = 0;
        app.settings_cycle(1, &path); // theme = light, file written
        app.settings_index = 1;
        app.settings_cycle(1, &path); // default mode = time-30s
        assert!(path.exists());

        app.settings_index = 2;
        app.settings_activate(&path);
        assert!(!path.exists(), "config file should be removed");
        assert!(path.with_extension("toml.bak").exists(), "backup missing");
        assert_eq!(app.theme, builtin::DARK);
        assert_eq!(app.theme_name, "dark");
        assert_eq!(app.default_mode, DefaultMode::Time(15));
    }

    /// Enter on a picker row advances it, same as →.
    #[test]
    fn settings_enter_advances_picker() {
        let (_dir, path, mut app) = make_settings_app();
        app.settings_index = 0;
        app.settings_activate(&path);
        assert_eq!(app.theme_name, "light");
    }

    /// The menu carries a Settings row under the More section.
    #[test]
    fn menu_contains_settings_row() {
        let menu = build_menu(&[], None);
        assert!(menu
            .iter()
            .any(|m| matches!(m.action, MenuAction::ShowSettings) && m.label == "Settings"));
    }

    // ── v0.5.0: daily challenge ─────────────────────────────────────────────

    fn july_10() -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(2026, 7, 10).unwrap()
    }

    /// The daily label embeds the date so per-mode PB naturally means "best
    /// on that day's challenge", and it round-trips through parse_label.
    #[test]
    fn daily_label_embeds_date_and_round_trips() {
        let mode = Mode::Daily(july_10());
        assert_eq!(mode.label(), "daily-2026-07-10");
        assert_eq!(Mode::parse_label("daily-2026-07-10"), Some(mode));
        assert_eq!(Mode::parse_label("daily-not-a-date"), None);
    }

    /// Starting the daily challenge loads the deterministic word list.
    #[test]
    fn daily_start_game_loads_seeded_words() {
        let mut app = make_app_with_word("placeholder");
        app.start_game(Mode::Daily(july_10())).unwrap();
        let expected = crate::words::daily_words(july_10());
        let actual: Vec<String> = app.words.iter().map(|w| w.text.clone()).collect();
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), crate::words::DAILY_WORD_COUNT);
    }

    /// The daily challenge finishes after the last word, like Words mode.
    #[test]
    fn daily_finishes_after_last_word() {
        let mut app = make_app_with_word("placeholder");
        app.mode = Mode::Daily(july_10());
        app.words = vec![Word::new("hi".into()), Word::new("yo".into())];
        for ch in "hi yo ".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.screen, Screen::Results);
    }

    /// The daily challenge reports progress so the gauge renders.
    #[test]
    fn daily_reports_progress() {
        let mut app = make_app_with_word("placeholder");
        app.start_game(Mode::Daily(july_10())).unwrap();
        app.current_word = 3;
        assert_eq!(app.progress(), Some((3, crate::words::DAILY_WORD_COUNT)));
    }

    /// The menu carries a Daily challenge row in its own section.
    #[test]
    fn menu_contains_daily_challenge_row() {
        let menu = build_menu(&[], None);
        let daily = menu
            .iter()
            .find(|m| m.label == "Daily challenge")
            .expect("Daily challenge row missing");
        assert!(matches!(daily.mode, Some(Mode::Daily(_))));
        assert!(matches!(daily.action, MenuAction::Start));
    }

    // ── v0.5.0: replay ──────────────────────────────────────────────────────

    /// `Mode::parse_label` is the exact inverse of `Mode::label` for every
    /// mode this version can save.
    #[test]
    fn mode_label_round_trips_through_parse_label() {
        use crate::words::CodeLang;
        let modes = [
            Mode::Time(30),
            Mode::Time(120),
            Mode::Words(50),
            Mode::Quote,
            Mode::Code(CodeLang::Rust),
            Mode::Code(CodeLang::JavaScript),
            Mode::Code(CodeLang::Shell),
            Mode::Zen,
            Mode::Custom,
            Mode::Symbols(25),
        ];
        for mode in modes {
            assert_eq!(
                Mode::parse_label(&mode.label()),
                Some(mode),
                "label {:?} did not round-trip",
                mode.label()
            );
        }
    }

    /// Unknown / malformed labels parse to `None` instead of panicking.
    #[test]
    fn parse_label_rejects_unknown_labels() {
        for bad in [
            "",
            "hyperspeed",
            "time-",
            "time-abcs",
            "time-30", // missing the trailing 's'
            "words-",
            "words-abc",
            "code-cobol",
            "symbols-x",
        ] {
            assert_eq!(Mode::parse_label(bad), None, "{bad:?} should not parse");
        }
    }

    /// Replay uses the supplied word list verbatim — no re-randomisation.
    #[test]
    fn start_replay_uses_exact_word_list() {
        let mut app = make_app_with_word("placeholder");
        app.start_replay(
            Mode::Words(3),
            vec!["alpha".into(), "beta".into(), "gamma".into()],
        )
        .unwrap();
        let texts: Vec<&str> = app.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["alpha", "beta", "gamma"]);
        assert_eq!(app.mode, Mode::Words(3));
        assert_eq!(app.screen, Screen::Typing);
        assert_eq!(app.current_word, 0);
        assert!(app.started_at.is_none());
    }

    /// Records saved before v0.5.0 carry no word list — replay must fail
    /// with a friendly error, not an empty session.
    #[test]
    fn start_replay_rejects_empty_word_list() {
        let mut app = make_app_with_word("abc");
        app.screen = Screen::Results;
        let err = app.start_replay(Mode::Words(5), vec![]).unwrap_err();
        assert!(err.to_string().contains("no replay data"));
        // App state untouched — still on the Results screen.
        assert_eq!(app.screen, Screen::Results);
    }

    /// A replayed session finishes with the original mode's semantics.
    #[test]
    fn replayed_words_session_finishes_on_last_word() {
        let mut app = make_app_with_word("placeholder");
        app.start_replay(Mode::Words(2), vec!["hi".into(), "yo".into()])
            .unwrap();
        for ch in "hi yo ".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.screen, Screen::Results);
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
