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
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use crate::options::{KeySound, StopOnError};
use crate::state::{self, AppState};
use crate::storage::{ResultsComparison, SessionRecord, StatsView};
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
    /// Change the saved settings (`~/.typerush/config.toml`).
    Settings,
    /// Level, totals, bests side by side and the activity calendar.
    Profile,
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
    /// Words sourced from a user-supplied text file (`--file path.txt`, the
    /// remembered last file, or a snippet from `~/.typerush/snippets/`).
    Custom,
    /// Programming-symbols drill: type N short tokens like `=>` or `(){};`.
    Symbols(usize),
    /// The daily challenge: the same words for everyone on a given day.
    Daily,
}

impl Mode {
    /// Short tag used when saving sessions to disk and in the UI ("time-30s",
    /// "words-50", "code-rust", …). Keeping the format stable means old stats
    /// stay readable across releases.
    ///
    /// Code labels come from the variant name, so JavaScript stays
    /// `"code-javascript"` and the new languages are `"code-go"`,
    /// `"code-java"`, `"code-sql"`, `"code-shell"`.
    pub fn label(&self) -> String {
        match self {
            Mode::Time(seconds) => format!("time-{}s", seconds),
            Mode::Words(count) => format!("words-{}", count),
            Mode::Quote => "quote".to_string(),
            Mode::Code(lang) => format!("code-{:?}", lang).to_lowercase(),
            Mode::Zen => "zen".to_string(),
            Mode::Custom => "custom".to_string(),
            Mode::Symbols(count) => format!("symbols-{}", count),
            Mode::Daily => "daily".to_string(),
        }
    }

    /// The mode a saved label was played in — the reverse of [`Mode::label`]
    /// (and of [`App::session_label`]: word-setting and replay suffixes such
    /// as `+p` are ignored, `custom-notes` is `Custom`). `None` for a label
    /// TypeRush never writes, or a count out of range (stats.json can be
    /// edited by hand).
    pub fn from_label(label: &str) -> Option<Mode> {
        use crate::config::load::{check_count, code_lang, MAX_COUNT, MAX_SECONDS};
        let base = label.split('+').next().unwrap_or(label);
        fn count<T: std::str::FromStr + PartialOrd + From<u16>>(text: &str, max: u16) -> Option<T> {
            check_count(text.parse().ok()?, max).ok()
        }
        if let Some(seconds) = base.strip_prefix("time-").and_then(|t| t.strip_suffix('s')) {
            return Some(Mode::Time(count(seconds, MAX_SECONDS)?));
        }
        if let Some(n) = base.strip_prefix("words-") {
            return Some(Mode::Words(count(n, MAX_COUNT)?));
        }
        if let Some(n) = base.strip_prefix("symbols-") {
            return Some(Mode::Symbols(count(n, MAX_COUNT)?));
        }
        if let Some(lang) = base.strip_prefix("code-") {
            return code_lang(lang).map(Mode::Code);
        }
        match base {
            "quote" => Some(Mode::Quote),
            "zen" => Some(Mode::Zen),
            "daily" => Some(Mode::Daily),
            _ if base == "custom" || base.starts_with("custom-") => Some(Mode::Custom),
            _ => None,
        }
    }
}

/// A session being typed again word for word (see [`App::start_replay`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Replay {
    /// The original session's label, without any `+replay`; the replay is
    /// saved as this plus `+replay`, so typing words you have already seen
    /// never sets the mode's real best.
    pub label: String,
    /// The mode the original was played in: how the replay ends.
    pub mode: Mode,
    /// The original's words, in order.
    pub words: Vec<String>,
    /// Word settings of the original (from its label), for the fresh words
    /// a time replay adds once you get past where the original stopped.
    pub pool: WordPool,
    pub decor: WordDecor,
    /// The original's WPM, which Results compares the replay with.
    pub wpm: f64,
    /// When the original was played.
    pub timestamp: chrono::DateTime<chrono::Local>,
}

impl Replay {
    /// A replay of `record`, or why it can't be replayed. The text comes
    /// from stats.json, so it is held to the same rules as a custom file:
    /// characters that can't be typed are dropped, and its size is bounded.
    pub fn of(record: &SessionRecord) -> anyhow::Result<Self> {
        use crate::storage::MAX_TEXT_BYTES;
        if record.text.len() > MAX_TEXT_BYTES {
            anyhow::bail!("this session's text is too long to replay");
        }
        let words: Vec<String> = record
            .text
            .split_whitespace()
            .map(|word| {
                word.chars()
                    .filter(|&c| !crate::text::is_untypeable(c))
                    .collect::<String>()
            })
            .filter(|word| !word.is_empty())
            .collect();
        if words.is_empty() {
            anyhow::bail!(
                "this session has no words saved to replay: sessions keep their text from TypeRush 0.5 on"
            );
        }
        let mut label = record.mode.as_str();
        while let Some(original) = label.strip_suffix("+replay") {
            label = original;
        }
        let suffixes: Vec<&str> = label.split('+').skip(1).collect();
        Ok(Self {
            label: label.to_string(),
            // A label TypeRush never writes is replayed to its last word.
            mode: Mode::from_label(label).unwrap_or(Mode::Custom),
            words,
            pool: if suffixes.contains(&"10k") {
                WordPool::Extended
            } else {
                WordPool::Common
            },
            decor: WordDecor {
                punctuation: suffixes.contains(&"p"),
                numbers: suffixes.contains(&"n"),
            },
            wpm: record.wpm,
            timestamp: record.timestamp,
        })
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

/// One second of a run (paused time left out): keys typed in it, and how
/// many of them were right and wrong. Drives raw speed per second, the
/// consistency figure and the Results chart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Second {
    pub typed: u32,
    pub correct: u32,
    pub errors: u32,
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
    /// Select this row of the list on screen (a Stats table row), like
    /// moving there with ↑/↓.
    Select(usize),
    /// Select this Settings row, then act like this key on it (‹ is ←,
    /// › is →, the value itself is Enter).
    Setting(usize, KeyCode),
}

/// One selectable option in the main menu. Consecutive items that share a
/// `group` are drawn under one heading, side by side ("15s  30s  60s  120s").
pub struct MenuItem {
    /// Category the option belongs to — the row's heading.
    pub group: &'static str,
    /// Option text within the row. Same as `group` for single-option rows.
    /// Owned because custom options show runtime names (snippet, file).
    pub label: String,
    /// Set for "start a game" rows; `None` for rows like "Stats" or "Quit".
    pub mode: Option<Mode>,
    pub action: MenuAction,
    /// File a `StartCustom` option types from. `None` everywhere else, and
    /// on the placeholder "custom" option shown when there's nothing to offer.
    pub custom_path: Option<PathBuf>,
}

/// What pressing Enter on a menu item should do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    /// Start the game in `MenuItem::mode`.
    Start,
    /// Start a custom-file session from `MenuItem::custom_path`.
    StartCustom,
    /// Jump to the historical stats screen.
    ShowStats,
    /// Open the Settings screen.
    ShowSettings,
    /// Open the Profile screen.
    ShowProfile,
    /// Quit the application.
    Quit,
}

/// Pick which menu row to highlight given the configured default mode.
///
/// Exact match wins (e.g. `time_seconds = 30` → "Time · 30s"). Otherwise we
/// fall back to the first row of the same mode family (so `time_seconds = 45`
/// still pre-selects a time row rather than landing on something unrelated).
pub fn best_menu_match(menu: &[MenuItem], default_mode: Mode) -> usize {
    let same_family =
        |mode: &Mode| std::mem::discriminant(mode) == std::mem::discriminant(&default_mode);
    menu.iter()
        .position(|item| item.mode == Some(default_mode))
        .or_else(|| {
            menu.iter()
                .position(|item| item.mode.as_ref().is_some_and(same_family))
        })
        .unwrap_or(0)
}

/// The menu with no custom sources — what tests and a fresh `App` start from.
#[cfg(test)]
pub fn default_menu() -> Vec<MenuItem> {
    build_menu(&[], None)
}

/// Build the main menu. The `custom` row offers `last_custom_file` (unless it
/// is one of the snippets) followed by every snippet; with neither, it shows
/// a single placeholder option that explains how to add one.
///
/// Categories with several options come first (time, words, code, symbols,
/// custom), then the single runs (daily, quote, zen), then one row with
/// the actions that don't start a game (`stats`, `profile`, `settings`,
/// `quit`) side by side.
pub fn build_menu(snippets: &[Snippet], last_custom_file: Option<&Path>) -> Vec<MenuItem> {
    use crate::words::CodeLang;
    let start = |group, label: &str, mode| MenuItem {
        group,
        label: label.to_string(),
        mode: Some(mode),
        action: MenuAction::Start,
        custom_path: None,
    };
    // The actions that don't start a game share the last row, with no
    // heading: "stats  profile  settings  quit".
    let other = |label: &str, action| MenuItem {
        group: "app",
        label: label.to_string(),
        mode: None,
        action,
        custom_path: None,
    };
    let custom = |label: String, path: Option<PathBuf>| MenuItem {
        group: "custom",
        label,
        mode: Some(Mode::Custom),
        action: MenuAction::StartCustom,
        custom_path: path,
    };

    let mut menu = vec![
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
        start("code", "js", Mode::Code(CodeLang::JavaScript)),
        start("code", "go", Mode::Code(CodeLang::Go)),
        start("code", "java", Mode::Code(CodeLang::Java)),
        start("code", "sql", Mode::Code(CodeLang::Sql)),
        start("code", "shell", Mode::Code(CodeLang::Shell)),
        start("symbols", "25", Mode::Symbols(25)),
        start("symbols", "50", Mode::Symbols(50)),
    ];

    let (snippet_labels, last_label) = custom_labels(snippets, last_custom_file);
    if let (Some(last), Some(label)) = (last_custom_file, last_label) {
        menu.push(custom(label, Some(last.to_path_buf())));
    }
    for (snippet, label) in snippets.iter().zip(snippet_labels) {
        menu.push(custom(label, Some(snippet.path.clone())));
    }
    if menu.last().is_some_and(|item| item.group != "custom") {
        menu.push(custom("custom".to_string(), None));
    }
    // The rows with a choice of options come first, then the single runs.
    menu.push(start("daily", "daily", Mode::Daily));
    menu.push(start("quote", "quote", Mode::Quote));
    menu.push(start("zen", "zen", Mode::Zen));

    menu.push(other("stats", MenuAction::ShowStats));
    menu.push(other("profile", MenuAction::ShowProfile));
    menu.push(other("settings", MenuAction::ShowSettings));
    menu.push(other("quit", MenuAction::Quit));
    menu
}

/// Widest custom option label, in terminal cells. Longer snippet or file
/// names are shortened in the middle so one name can't take over the row.
pub const MAX_OPTION_WIDTH: usize = 24;

/// Labels for the custom row: one per snippet, plus one for the remembered
/// file unless it is one of the snippets (`None` then).
///
/// Labels are exactly what is drawn — unsafe characters shown as `?` and
/// long names shortened to `MAX_OPTION_WIDTH` — and no two are the same, so
/// every option can be told apart on screen:
///
/// - a snippet is labelled by its name, or its full file name when two
///   snippets share a name (`notes.txt` / `notes.TXT`), using the snippet set
///   alone — so which file is remembered never changes a snippet's label;
/// - the remembered file is labelled by its file name, or `folder/name` when
///   a snippet already uses that;
/// - labels that still clash: the first keeps it, the rest take the first
///   " (n)" not already used — natural names are claimed before any number,
///   so a real `notes (2).txt` keeps its name.
fn custom_labels(snippets: &[Snippet], last: Option<&Path>) -> (Vec<String>, Option<String>) {
    use crate::text::{printable, shorten};
    use std::collections::HashSet;

    let display = |label: &str| shorten(&printable(label), MAX_OPTION_WIDTH).into_owned();
    let file_name = |path: &Path| {
        path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    };

    let mut name_count: HashMap<&str, usize> = HashMap::new();
    for snippet in snippets {
        *name_count.entry(snippet.name.as_str()).or_insert(0) += 1;
    }
    let mut bases: Vec<String> = snippets
        .iter()
        .map(|snippet| {
            if name_count[snippet.name.as_str()] > 1 {
                file_name(&snippet.path)
            } else {
                snippet.name.clone()
            }
        })
        .collect();
    let mut displays: Vec<String> = bases.iter().map(|base| display(base)).collect();

    let last = last.filter(|last| {
        // Compare canonical forms too, so `./notes.txt` or a symlink still
        // matches its snippet. Paths that can't be canonicalized (missing)
        // only match when literally equal.
        let canonical = std::fs::canonicalize(last).ok();
        !snippets
            .iter()
            .any(|s| s.path == *last || (canonical.is_some() && s.canonical_path == canonical))
    });
    if let Some(last) = last {
        let name = file_name(last);
        // `last` is absolute (see `offered_custom_file`), so it normally has a
        // folder; one that doesn't (`/notes.txt`, `../notes.txt`) falls back
        // to the numbering below, which still keeps labels distinct.
        let folder = last
            .parent()
            .and_then(Path::file_name)
            .map(|folder| folder.to_string_lossy().into_owned());
        let base = match folder {
            Some(folder) if displays.contains(&display(&name)) => {
                format!("{folder}{}{name}", std::path::MAIN_SEPARATOR)
            }
            _ => name,
        };
        displays.push(display(&base));
        bases.push(base);
    }

    // Each natural label belongs to the first option that wants it, and all
    // of them are taken before any number is handed out — so a generated
    // "notes (2)" can never take a name another option has.
    let mut owner: HashMap<&str, usize> = HashMap::new();
    for (index, text) in displays.iter().enumerate() {
        owner.entry(text.as_str()).or_insert(index);
    }
    let mut in_use: HashSet<String> = displays.iter().cloned().collect();
    let mut labels: Vec<String> = Vec::with_capacity(bases.len());
    for (index, (base, text)) in bases.iter().zip(&displays).enumerate() {
        if owner[text.as_str()] == index {
            labels.push(text.clone());
            continue;
        }
        // A repeat: the first free " (n)".
        let mut n = 2;
        let label = loop {
            let candidate = display(&format!("{base} ({n})"));
            if !in_use.contains(&candidate) {
                break candidate;
            }
            n += 1;
        };
        in_use.insert(label.clone());
        labels.push(label);
    }

    let last_label = last.and_then(|_| labels.pop());
    (labels, last_label)
}

/// `path` made absolute against the current directory, so a remembered file
/// still works when TypeRush is next started somewhere else. Not
/// canonicalized: symlinks and the user's spelling of the path are kept.
fn absolute(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }
    std::env::current_dir().map_or(path.clone(), |dir| dir.join(&path))
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
    /// When the current pause began (`Ctrl+P` / `F2`); `None` while running.
    pub paused_at: Option<Instant>,
    /// Time spent paused in earlier pauses of this session; never counted.
    pub paused_for: Duration,

    // --- counters used for WPM / accuracy ---
    /// Characters typed that matched the target. Drives both WPM and accuracy.
    pub correct_chars: usize,
    /// Every character the user has typed during the session, including
    /// spaces (a correctly typed space also counts as a correct char).
    pub total_typed_chars: usize,

    // --- misc ---
    /// File the next `Mode::Custom` session types from: `--file`, or the
    /// custom option picked in the menu. Kept for Ctrl+R / Enter restarts.
    pub custom_file: Option<PathBuf>,
    /// Set to `true` from any handler to exit the main loop cleanly.
    pub should_quit: bool,
    /// Transient error message rendered as a modal overlay.
    pub error_message: Option<String>,
    /// Transient message that isn't an error ("exported 42 sessions to …"),
    /// shown and dismissed like `error_message`.
    pub notice: Option<String>,
    /// Active color palette — read by every UI module on every frame.
    pub theme: ThemePalette,
    /// How a run behaves and looks (the `[typing]` settings, v0.7.0).
    pub options: crate::options::TypingOptions,

    // --- per-key accuracy (v0.3.0) ---
    /// Per expected character: how many times it was typed correctly.
    pub key_hits: HashMap<char, u64>,
    /// Per expected character: how many times something else was typed instead.
    pub key_misses: HashMap<char, u64>,

    // --- deeper results (v0.6.0) ---
    /// The run second by second: index = whole seconds of typing elapsed
    /// when the key was pressed. ponytail: one entry per second typed —
    /// 12 bytes a second, so even a day of zen is under a megabyte.
    pub timeline: Vec<Second>,
    /// The run was ended with Esc, not by its clock or its last character.
    pub ended_early: bool,
    /// The last key was refused (stop on error): the cursor shows it.
    pub rejected_key: bool,
    /// The key just typed should ring the terminal bell (`typing.sound`);
    /// the main loop rings it and clears this.
    pub bell: bool,
    /// Characters of the text left untyped when the run ended
    /// ([`App::missed_chars`]), worked out once then — Results shows it on
    /// every frame, and a big custom file has a lot of text left.
    pub missed: usize,

    // --- stats caching (v0.3.0) ---
    /// Full session history, read from disk the first time the Stats or
    /// Results screen needs it and replaced by the updated list on every
    /// save. `None` until first needed (or after a failed save).
    pub stats_cache: Option<Vec<SessionRecord>>,
    /// What the Stats screen shows (category, its summary, every mode's
    /// best), built from `stats_cache` when that screen is entered and when
    /// the category changes. `None` until the first Stats visit.
    pub stats_view: Option<StatsView>,
    /// What the Results screen compares against, computed when that screen
    /// is entered (after the session is saved).
    pub results_comparison: Option<ResultsComparison>,
    /// Whether the session currently shown on the Results screen was actually
    /// written to stats.json. False for Zen, sub-1-second, and zero-keystroke
    /// sessions (and on disk failure) — the Results screen uses this to know
    /// whether the last history entry is the current session or a previous one.
    pub session_just_saved: bool,

    // --- word sources (v0.4.0) ---
    /// English pool for Time / Words (`--big` / `--no-big`, `[words] pool`,
    /// or `b` in the menu). Zen always uses the common pool.
    pub word_pool: WordPool,
    /// Punctuation / numbers decoration for Time / Words (`p` / `n` in the
    /// menu, or the CLI / config switches). Never applied to Zen.
    pub word_decor: WordDecor,
    /// Snippet library found in `~/.typerush/snippets/` at startup. Kept so
    /// the menu can be rebuilt without rescanning the directory.
    pub snippets: Vec<Snippet>,
    /// Persisted UI state (the remembered last custom file).
    pub app_state: AppState,
    /// Where `app_state` is saved. `None` means "don't persist" — the
    /// default, so tests never write to the real home directory.
    pub state_file: Option<PathBuf>,
    /// Runs started since `app_state` was last saved: counted on a run's
    /// first key (never in the key path's disk writes) and saved by
    /// [`App::save_run_count`] when a run ends and on quit.
    pub runs_started_unsaved: u64,
    /// When the first of those runs started, for `counting_since`.
    pub first_unsaved_run: Option<chrono::DateTime<chrono::Local>>,
    /// Where `e` on the Stats screen writes the CSV export
    /// (`~/.typerush/stats.csv`). `None` — the default, and in tests unless
    /// they set one — means exporting is off.
    pub export_file: Option<PathBuf>,

    // --- settings (v0.5.0) ---
    /// The config file the Settings screen saves to
    /// (`~/.typerush/config.toml`). `None` — the default, so tests never
    /// write to the real home directory — means nothing is saved.
    pub config_file: Option<PathBuf>,
    /// What the Settings screen shows, read from `config_file` when the
    /// screen is entered and after each change.
    pub settings: Option<crate::settings::SettingsView>,

    // --- profile (v0.6.0) ---
    /// What the Profile screen shows, worked out from the history when the
    /// screen is entered (not coming back from help).
    pub profile: Option<crate::progress::Profile>,

    // --- replay (v0.5.0) ---
    /// The session being typed again, while replaying one. Restarting
    /// replays it again; starting any other game clears it.
    pub replay: Option<Replay>,

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
    ///
    /// Touches no disk: snippets and the remembered file are added later by
    /// [`App::load_custom_sources`]. A `custom_file` (`--file`) is offered in
    /// the menu's custom row straight away.
    pub fn new(custom_file: Option<PathBuf>, palette: ThemePalette, default_mode: Mode) -> Self {
        // Always absolute (see `absolute`): labels and state.json then never
        // depend on the directory TypeRush happens to be in.
        let custom_file = custom_file.map(absolute);
        let menu = build_menu(&[], custom_file.as_deref());
        let initial_mode = default_mode;
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
            paused_at: None,
            paused_for: Duration::ZERO,
            correct_chars: 0,
            total_typed_chars: 0,
            custom_file,
            should_quit: false,
            error_message: None,
            notice: None,
            theme: palette,
            options: Default::default(),
            key_hits: HashMap::new(),
            key_misses: HashMap::new(),
            timeline: Vec::new(),
            ended_early: false,
            rejected_key: false,
            bell: false,
            missed: 0,
            stats_cache: None,
            stats_view: None,
            results_comparison: None,
            session_just_saved: false,
            word_pool: WordPool::Common,
            word_decor: WordDecor::default(),
            snippets: Vec::new(),
            app_state: AppState::default(),
            state_file: None,
            runs_started_unsaved: 0,
            first_unsaved_run: None,
            export_file: None,
            config_file: None,
            settings: None,
            profile: None,
            replay: None,
            click_targets: RefCell::new(Vec::new()),
            pressed_target: None,
        }
    }

    /// The label this session is saved, compared and shown under: the mode
    /// label, plus a suffix for each setting that makes time / words harder —
    /// `+10k` (extended pool), `+p` (punctuation), `+n` (numbers). A harder
    /// run therefore never competes with plain runs for a personal best.
    ///
    /// Custom sessions are labelled by the file they type (`custom-notes`
    /// for `notes.txt`), so every snippet or file has its own personal best
    /// rather than a 5-word drill and a long essay sharing one.
    ///
    /// Plain runs, and every other mode, keep exactly their v0.3 labels. A
    /// replay is its original's label plus `+replay`.
    pub fn session_label(&self) -> String {
        if let Some(replay) = &self.replay {
            return format!("{}+replay", replay.label);
        }
        let mut label = self.mode.label();
        if self.mode == Mode::Custom {
            // Lossy and made safe: the label is drawn on screen and stored
            // in stats.json, and a file name can contain anything.
            let name = self.custom_file.as_deref().and_then(Path::file_stem);
            if let Some(name) = name.filter(|name| !name.is_empty()) {
                label = format!(
                    "{label}-{}",
                    crate::text::printable(&name.to_string_lossy())
                );
            }
        }
        if matches!(self.mode, Mode::Time(_) | Mode::Words(_)) {
            if self.word_pool == WordPool::Extended {
                label.push_str("+10k");
            }
            if self.word_decor.punctuation {
                label.push_str("+p");
            }
            if self.word_decor.numbers {
                label.push_str("+n");
            }
        }
        label
    }

    /// Close the error or notice modal if one is open; whether one was.
    /// Any key or click does this before acting on anything else.
    pub fn dismiss_modal(&mut self) -> bool {
        let open = self.error_message.is_some() || self.notice.is_some();
        self.error_message = None;
        self.notice = None;
        open
    }

    /// Show `error` in the error modal, with its cause chain ("can't read
    /// notes.txt: No such file or directory").
    pub fn show_error(&mut self, error: &anyhow::Error) {
        self.error_message = Some(format!("{error:#}"));
    }

    /// Choose the English pool and decoration for Time / Words.
    pub fn set_word_source(&mut self, pool: WordPool, decor: WordDecor) {
        self.word_pool = pool;
        self.word_decor = decor;
    }

    /// Offer the snippet library and the remembered last custom file in the
    /// menu, and remember future custom picks in `state_file` (`None`: don't).
    pub fn load_custom_sources(
        &mut self,
        snippets: Vec<Snippet>,
        app_state: AppState,
        state_file: Option<PathBuf>,
    ) {
        self.snippets = snippets;
        self.app_state = app_state;
        self.state_file = state_file;
        self.rebuild_menu();
    }

    /// The file the custom row offers first: `--file` / the current pick,
    /// else the one remembered from a previous run.
    fn offered_custom_file(&self) -> Option<PathBuf> {
        // `custom_file` is already absolute; a hand-edited state.json might
        // not be, so normalize the remembered path the same way.
        self.custom_file.clone().or_else(|| {
            self.app_state
                .last_custom_file
                .as_ref()
                .map(|path| absolute(PathBuf::from(path)))
        })
    }

    /// Rebuild the menu (custom row contents changed) keeping the highlight
    /// on the same option, or the first custom option if that one is gone.
    fn rebuild_menu(&mut self) {
        // `action` is part of the key: stats and quit have the same mode and
        // path (`None`), so without it a highlight on quit would land on stats.
        let selected = self
            .menu
            .get(self.menu_index)
            .map(|item| (item.mode, item.action, item.custom_path.clone()));
        let offered = self.offered_custom_file();
        self.menu = build_menu(&self.snippets, offered.as_deref());
        self.menu_index = selected
            .and_then(|(mode, action, path)| {
                self.menu
                    .iter()
                    .position(|item| {
                        item.mode == mode && item.action == action && item.custom_path == path
                    })
                    .or_else(|| {
                        if mode == Some(Mode::Custom) {
                            self.menu
                                .iter()
                                .position(|item| item.action == MenuAction::StartCustom)
                        } else {
                            None
                        }
                    })
            })
            .unwrap_or(self.menu_index)
            .min(self.menu.len() - 1);
    }

    /// Start a custom-file session from `path` (or from the current
    /// `custom_file`, e.g. `--file`, when `None`), then remember the file in
    /// `state.json` and the menu.
    ///
    /// If the file fails to load nothing changes — not the screen, the mode,
    /// the file a restart would use, nor the remembered file.
    pub fn start_custom(&mut self, path: Option<PathBuf>) -> anyhow::Result<()> {
        let previous = self.custom_file.clone();
        if let Some(path) = path {
            self.custom_file = Some(absolute(path));
        }
        if let Err(e) = self.start_game(Mode::Custom) {
            self.custom_file = previous;
            return Err(e);
        }
        self.persist_custom_file();
        Ok(())
    }

    /// Remember the current custom file in `state.json` and show it in the
    /// menu. Only called once the file has loaded.
    ///
    /// Non-UTF-8 paths aren't stored. Disk errors are ignored — losing this
    /// memory is never worth an error.
    fn persist_custom_file(&mut self) {
        let Some(path) = self.custom_file.clone() else {
            return;
        };
        // `custom_file` is always absolute: `new` and `start_custom` make it so.
        let Some(text) = path.to_str() else {
            return;
        };
        if self.app_state.last_custom_file.as_deref() == Some(text) {
            return;
        }
        let mut updated = self.app_state.clone();
        updated.last_custom_file = Some(text.to_string());
        // Only count the file as remembered once it is on disk, so a failed
        // write (disk full, read-only home) is retried the next time it starts.
        let saved = match &self.state_file {
            Some(file) => state::save_to_path(file, &updated).is_ok(),
            None => true,
        };
        if saved {
            self.app_state = updated;
        }
        self.rebuild_menu();
    }

    /// Runs started in all: the saved count, the runs since, and the saved
    /// sessions from before counting began (they were started too).
    pub fn runs_started(&self) -> u64 {
        let since = self.app_state.counting_since.or(self.first_unsaved_run);
        let before = self
            .stats_cache
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter(|s| since.is_none_or(|since| s.timestamp < since))
            .count() as u64;
        self.app_state.runs_started + self.runs_started_unsaved + before
    }

    /// Add the runs started since the last save to `state.json`. Called
    /// when a run ends and on quit; a failed write keeps them for the next
    /// try. Tests (no `state_file`) keep them in memory.
    pub fn save_run_count(&mut self) {
        if self.runs_started_unsaved == 0 {
            return;
        }
        let Some(file) = self.state_file.clone() else {
            return;
        };
        // Added to the count on disk, not the one read at start: another
        // TypeRush window may have saved its runs since.
        let mut updated = state::load_from_path(&file);
        updated.runs_started += self.runs_started_unsaved;
        updated.counting_since = updated.counting_since.or(self.first_unsaved_run);
        if state::save_to_path(&file, &updated).is_ok() {
            self.app_state.runs_started = updated.runs_started;
            self.app_state.counting_since = updated.counting_since;
            self.runs_started_unsaved = 0;
            self.first_unsaved_run = None;
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

    /// How long the user has been (or was) typing during the current session,
    /// paused time left out. Returns `Duration::ZERO` until the first keypress.
    pub fn elapsed(&self) -> Duration {
        let Some(start) = self.started_at else {
            return Duration::ZERO;
        };
        // While paused the clock stands still at the moment the pause began.
        let end = self
            .ended_at
            .or(self.paused_at)
            .unwrap_or_else(Instant::now);
        end.saturating_duration_since(start)
            .saturating_sub(self.paused_for)
    }

    /// Whether the session is paused: the clock is stopped and typing keys
    /// are ignored until it resumes.
    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    /// Pause the session, or resume it. Paused time never counts towards
    /// WPM or a time run's clock.
    pub fn toggle_pause(&mut self) {
        match self.paused_at.take() {
            // Before the first key the clock isn't running: nothing to subtract.
            Some(at) if self.started_at.is_some() => self.paused_for += at.elapsed(),
            Some(_) => {}
            None => self.paused_at = Some(Instant::now()),
        }
    }

    /// Live (or final) words-per-minute.
    ///
    /// Uses the industry-standard formula: a "word" is 5 characters, so
    /// `wpm = (correct_chars / 5) / minutes_elapsed`.
    pub fn wpm(&self) -> f64 {
        let minutes = self.elapsed().as_secs_f64() / 60.0;
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

    /// Raw words per minute: every key typed, right or wrong —
    /// `(total_typed_chars / 5) / minutes`. Next to `wpm` it shows how
    /// much speed the mistakes cost.
    pub fn raw_wpm(&self) -> f64 {
        let minutes = self.elapsed().as_secs_f64() / 60.0;
        if minutes <= 0.0 {
            return 0.0;
        }
        (self.total_typed_chars as f64 / 5.0) / minutes
    }

    /// How even the typing speed was, 0–100%: 100 minus the spread of the
    /// keys typed per whole second, as a percentage of their mean (100% =
    /// the same number of keys every second). Seconds with no key count, so
    /// stopping to think lowers it. `None` under two whole seconds.
    pub fn consistency(&self) -> Option<f64> {
        let seconds = self.elapsed().as_secs() as usize;
        if seconds < 2 {
            return None;
        }
        let typed = |i: usize| self.timeline.get(i).map_or(0.0, |s| f64::from(s.typed));
        let mean = (0..seconds).map(typed).sum::<f64>() / seconds as f64;
        if mean <= 0.0 {
            return None;
        }
        let variance =
            (0..seconds).map(|i| (typed(i) - mean).powi(2)).sum::<f64>() / seconds as f64;
        Some((100.0 * (1.0 - variance.sqrt() / mean)).clamp(0.0, 100.0))
    }

    /// Characters of the text never typed: what was left of a fixed text
    /// (words, quote, code, symbols, custom, daily) when the run ended
    /// early. Endless runs (time, zen) miss nothing — there is always more.
    pub fn missed_chars(&self) -> usize {
        if matches!(self.mode, Mode::Time(_) | Mode::Zen) {
            return 0;
        }
        let Some(current) = self.words.get(self.current_word) else {
            return 0;
        };
        let rest_of_word = current
            .text
            .chars()
            .count()
            .saturating_sub(current.typed.chars().count());
        // Each later word, and the space before it.
        let later: usize = self.words[self.current_word + 1..]
            .iter()
            .map(|word| word.text.chars().count() + 1)
            .sum();
        rest_of_word + later
    }

    /// End the run now (Esc): it counts as ended early, not completed.
    pub fn end_early(&mut self) {
        if self.ended_at.is_none() {
            self.ended_early = true;
        }
        self.finish_game();
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

    /// In `Mode::Words` / `Mode::Symbols` / `Mode::Daily`, `(completed,
    /// total)`. `None` for all other modes.
    pub fn progress(&self) -> Option<(usize, usize)> {
        match self.mode {
            Mode::Words(target) | Mode::Symbols(target) => {
                Some((self.current_word.min(target), target))
            }
            Mode::Daily => Some((self.current_word.min(self.words.len()), self.words.len())),
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
        let (pool, decor) = (self.word_pool, self.word_decor);
        // Build the word list before touching any state: if a custom file
        // fails to load, the app is left exactly as it was.
        let text: Vec<String> = match mode {
            // Endless modes: filled by `top_up_words` below, and topped up
            // as the typist nears the end, so no one ever runs out.
            Mode::Time(_) | Mode::Zen => Vec::new(),
            Mode::Words(count) => words::random_words_from(count, pool, decor),
            Mode::Quote => words::random_quote(),
            Mode::Code(lang) => words::random_code_snippet(lang),
            Mode::Symbols(count) => words::symbols::random_symbol_tokens(count),
            Mode::Daily => words::daily_words(chrono::Local::now().date_naive()),
            Mode::Custom => {
                let Some(path) = &self.custom_file else {
                    return Err(anyhow::anyhow!(
                        "no custom file yet: run `typerush --file <path>`, or drop .txt files in {}",
                        crate::text::printable(&words::snippets::snippets_dir().display().to_string())
                    ));
                };
                let loaded = words::words_from_file(path)?;
                if loaded.is_empty() {
                    return Err(anyhow::anyhow!(
                        "{} is empty",
                        crate::text::printable(&path.display().to_string())
                    ));
                }
                loaded
            }
        };
        self.replay = None;
        self.begin(mode, text);
        Ok(())
    }

    /// Type `record`'s words again, word for word. It ends the way the
    /// original did (a time run on the clock, anything else on its last
    /// word) and is saved under the original's label plus `+replay`.
    ///
    /// If the session can't be replayed (no saved text) nothing changes.
    pub fn start_replay(&mut self, record: &SessionRecord) -> anyhow::Result<()> {
        let replay = Replay::of(record)?;
        let (mode, words) = (replay.mode, replay.words.clone());
        self.replay = Some(replay);
        self.begin(mode, words);
        Ok(())
    }

    /// Type the run just finished again, the same words (`F6` on Results):
    /// a replay of it — saved as `+replay`, since the words are now seen,
    /// and compared with it on Results. A replay is simply replayed again.
    pub fn repeat_run(&mut self) {
        if self.replay.is_none() {
            let mut label = self.session_label();
            while let Some(original) = label.strip_suffix("+replay") {
                label = original.to_string();
            }
            self.replay = Some(Replay {
                label,
                mode: self.mode,
                words: self.session_text().split(' ').map(str::to_string).collect(),
                pool: self.word_pool,
                decor: self.word_decor,
                wpm: self.wpm(),
                timestamp: chrono::Local::now(),
            });
        }
        let _ = self.restart();
    }

    /// Reset all session state and start typing `text` in `mode`.
    fn begin(&mut self, mode: Mode, text: Vec<String>) {
        self.mode = mode;
        self.words = text.into_iter().map(Word::new).collect();
        self.current_word = 0;
        self.started_at = None;
        self.ended_at = None;
        self.paused_at = None;
        self.paused_for = Duration::ZERO;
        self.correct_chars = 0;
        self.total_typed_chars = 0;
        self.key_hits.clear();
        self.key_misses.clear();
        self.timeline.clear();
        self.ended_early = false;
        self.rejected_key = false;
        self.missed = 0;
        self.top_up_words();
        self.screen = Screen::Typing;
    }

    /// The text the Results best is limited to: today's words for the daily
    /// challenge (its best is today's best), `None` for everything else.
    pub fn best_text(&self) -> Option<String> {
        (self.mode == Mode::Daily && self.replay.is_none()).then(|| self.session_text())
    }

    /// The session's words as saved for replaying: every word of a run
    /// with a fixed text, only the words reached in an endless (time) run.
    /// Empty when longer than [`crate::storage::MAX_TEXT_BYTES`].
    pub fn session_text(&self) -> String {
        let words = match self.mode {
            Mode::Time(_) | Mode::Zen => {
                &self.words[..(self.current_word + 1).min(self.words.len())]
            }
            _ => &self.words[..],
        };
        let bytes = words.iter().map(|w| w.text.len() + 1).sum::<usize>();
        if bytes.saturating_sub(1) > crate::storage::MAX_TEXT_BYTES {
            return String::new();
        }
        let words: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        words.join(" ")
    }

    /// Time and zen runs end on the clock or on Esc, not on the last word:
    /// keep at least a batch of words ahead of the cursor.
    fn top_up_words(&mut self) {
        const BATCH: usize = 100;
        let (pool, decor) = match (self.mode, &self.replay) {
            // Past the end of a replayed time run: fresh words played the
            // way the original was.
            (Mode::Time(_), Some(replay)) => (replay.pool, replay.decor),
            (Mode::Time(_), None) => (self.word_pool, self.word_decor),
            // Zen stays calm and plain: the word settings are for time and
            // words runs, which are scored; zen never is.
            (Mode::Zen, _) => (WordPool::Common, WordDecor::default()),
            _ => return,
        };
        if self.words.len().saturating_sub(self.current_word) < BATCH {
            let more = crate::words::random_words_from(BATCH, pool, decor);
            self.words.extend(more.into_iter().map(Word::new));
        }
    }

    /// Restart the most recent mode with a fresh word list — or, while
    /// replaying, the same replay again.
    pub fn restart(&mut self) -> anyhow::Result<()> {
        if let Some(replay) = &self.replay {
            let (mode, words) = (replay.mode, replay.words.clone());
            self.begin(mode, words);
            return Ok(());
        }
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
    /// Finishing while paused ends the run at the moment it was paused.
    pub fn finish_game(&mut self) {
        if self.is_paused() {
            self.toggle_pause();
        }
        if self.ended_at.is_none() {
            self.ended_at = Some(Instant::now());
            self.missed = self.missed_chars();
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
        // Paused: the text is hidden and the clock stopped, so nothing types.
        if self.is_paused() {
            return;
        }
        // A run's first key starts it. Zen runs aren't counted: they are
        // never saved, so they could never be completed.
        if self.started_at.is_none() && self.mode != Mode::Zen {
            self.runs_started_unsaved += 1;
            self.first_unsaved_run
                .get_or_insert_with(chrono::Local::now);
        }
        self.ensure_timer_started();
        if self.current_word >= self.words.len() {
            return;
        }

        let is_last_word = self.current_word + 1 >= self.words.len();
        let second = self.elapsed().as_secs() as usize;
        let active_word = &mut self.words[self.current_word];
        let cursor_position = active_word.typed.chars().count();
        let word_len = active_word.text.chars().count();
        self.total_typed_chars += 1;

        // Past the end of the word the cursor sits on the space after it.
        let expected_char = active_word.text.chars().nth(cursor_position);
        let target_char = expected_char.unwrap_or(' ');
        // Stop on error: a key that may not go on is counted as a mistake
        // and changes nothing else. Letter: any wrong key. Word: anything
        // at the end of a word that still has a mistake in it (and at the
        // end of the last word, which has no space to type).
        let blocked = match self.options.stop_on_error {
            StopOnError::Off => false,
            StopOnError::Letter => typed_char != target_char,
            StopOnError::Word => {
                expected_char.is_none()
                    && (is_last_word || active_word.typed != active_word.text || typed_char != ' ')
            }
        };
        self.rejected_key = blocked;
        let is_correct = typed_char == target_char && !blocked;
        self.bell = match self.options.sound {
            KeySound::Off => false,
            KeySound::Errors => !is_correct,
            KeySound::Keys => true,
        };
        // Per-key stats are kept for the key that *should* have been pressed.
        let key_counts = if is_correct {
            self.correct_chars += 1;
            &mut self.key_hits
        } else {
            &mut self.key_misses
        };
        *key_counts.entry(target_char).or_insert(0) += 1;
        if self.timeline.len() <= second {
            self.timeline.resize(second + 1, Second::default());
        }
        let tally = &mut self.timeline[second];
        tally.typed += 1;
        if is_correct {
            tally.correct += 1;
        } else {
            tally.errors += 1;
        }

        if blocked {
            return;
        }
        if expected_char.is_none() {
            // Cursor on the space after the word: that slot is a character too.
            active_word.space_missed = !is_correct;
            self.advance_word();
            return;
        }

        active_word.typed.push(typed_char);

        // Last character of the last word typed: there is no trailing space.
        // Stopping on word errors, the run only ends once the word is right.
        let word_done = self.options.stop_on_error != StopOnError::Word
            || active_word.typed == active_word.text;
        if is_last_word && cursor_position + 1 == word_len && word_done {
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
        if self.is_paused() || self.current_word >= self.words.len() {
            return;
        }
        self.rejected_key = false;
        let active_word = &mut self.words[self.current_word];

        // Cursor at the very start of the current word → step back to the previous one.
        if active_word.typed.is_empty() {
            if self.current_word > 0 {
                self.current_word -= 1;
                self.words[self.current_word].space_missed = false;
            }
            return;
        }

        if delete_whole_word {
            active_word.typed.clear();
        } else {
            active_word.typed.pop();
        }
    }

    /// Moves the cursor to the next word. Will also `finish_game()` if this
    /// reaches the configured word/quote/code target.
    fn advance_word(&mut self) {
        self.current_word += 1;
        self.top_up_words();

        // Count modes (words, symbol tokens): stop once the target is reached.
        if let Mode::Words(target) | Mode::Symbols(target) = self.mode {
            if self.current_word >= target {
                self.finish_game();
                return;
            }
        }
        // Every mode with a fixed text stops when there's nothing left to
        // type — a count mode too, if its text is shorter than its count
        // (a replay of a hand-edited record), or no key would ever act.
        if !matches!(self.mode, Mode::Time(_) | Mode::Zen) && self.current_word >= self.words.len()
        {
            self.finish_game();
        }
    }

    /// Called by the main loop every 100ms: in a time-limited mode, ends the
    /// game when the timer expires.
    pub fn tick(&mut self) {
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
        let index = best_menu_match(&menu, Mode::Time(30));
        assert_eq!(menu[index].label, "30s");
    }

    #[test]
    fn best_menu_match_exact_words() {
        let menu = default_menu();
        let index = best_menu_match(&menu, Mode::Words(100));
        assert_eq!(menu[index].label, "100");
    }

    #[test]
    fn best_menu_match_falls_back_to_first_time_row() {
        // 45 isn't one of the four standard time rows; we expect the first
        // time row ("Time · 15s") rather than something unrelated.
        let menu = default_menu();
        let index = best_menu_match(&menu, Mode::Time(45));
        assert_eq!(menu[index].label, "15s");
    }

    #[test]
    fn best_menu_match_falls_back_to_first_words_row() {
        let menu = default_menu();
        let index = best_menu_match(&menu, Mode::Words(7));
        assert_eq!(menu[index].label, "10");
    }

    #[test]
    fn best_menu_match_picks_zen_row() {
        let menu = default_menu();
        let index = best_menu_match(&menu, Mode::Zen);
        assert_eq!(menu[index].label, "zen");
    }

    fn menu_app() -> App {
        App::new(None, crate::theme::ThemePalette::default(), Mode::Time(30))
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
        assert_eq!(app.menu[app.menu_index].label, "50"); // symbols
        app.menu_move_row(true);
        assert_eq!(app.menu[app.menu_index].label, "custom"); // clamped
        app.menu_move_row(false);
        assert_eq!(app.menu[app.menu_index].label, "25");
    }

    #[test]
    fn menu_up_down_wraps_between_first_and_last_row() {
        let mut app = menu_app(); // time · 30s (column 1)
        app.menu_move_row(false);
        assert_eq!(app.menu[app.menu_index].group, "app");
        assert_eq!(app.menu[app.menu_index].label, "profile");
        app.menu_move_row(true);
        assert_eq!(app.menu[app.menu_index].group, "time");
    }

    /// Time and zen runs never run out of words, however fast the typist.
    #[test]
    fn endless_modes_top_up_words() {
        for mode in [Mode::Time(60), Mode::Zen] {
            let mut app = menu_app();
            app.start_game(mode).unwrap();
            for _ in 0..1000 {
                let word = app.words[app.current_word].text.clone();
                for ch in word.chars().chain([' ']) {
                    app.handle_char(ch);
                }
            }
            assert_eq!(app.current_word, 1000, "{mode:?}");
            assert!(app.words.len() >= 1100, "{mode:?}");
            assert_eq!(app.screen, Screen::Typing, "{mode:?}");
        }
    }

    // ── pause (v0.5.0) ─────────────────────────────────────────────────────

    /// Paused time is left out of the clock: while paused it stands still,
    /// and after resuming (or finishing while paused) it never counts.
    #[test]
    fn paused_time_never_counts() {
        let mut app = make_app_with_word("hello");
        let now = Instant::now();
        app.started_at = Some(now - Duration::from_secs(10));
        app.paused_at = Some(now - Duration::from_secs(6));
        let frozen = app.elapsed();
        assert!((frozen.as_secs_f64() - 4.0).abs() < 0.1, "{frozen:?}");
        app.toggle_pause(); // resume
        assert!(!app.is_paused());
        assert!((app.elapsed().as_secs_f64() - 4.0).abs() < 0.1);

        // Pause again for 3 s, then finish while paused: still 4 s.
        app.paused_at = Some(Instant::now() - Duration::from_secs(3));
        app.paused_for = Duration::from_secs(6);
        app.started_at = Some(Instant::now() - Duration::from_secs(13));
        app.finish_game();
        assert!(!app.is_paused());
        assert!((app.elapsed().as_secs_f64() - 4.0).abs() < 0.1);
    }

    /// While paused, keys type nothing, the clock doesn't start, and a time
    /// run doesn't run out; a new game is never paused.
    #[test]
    fn paused_session_ignores_typing_and_clock() {
        let mut app = menu_app();
        app.start_game(Mode::Time(15)).unwrap();
        app.toggle_pause();
        app.handle_char('x');
        app.handle_backspace(true);
        assert_eq!(app.total_typed_chars, 0);
        assert!(app.started_at.is_none(), "clock started while paused");
        app.toggle_pause();
        assert_eq!(app.elapsed(), Duration::ZERO);

        app.started_at = Some(Instant::now() - Duration::from_secs(20));
        app.paused_at = Some(Instant::now() - Duration::from_secs(10));
        app.tick();
        assert_eq!(app.screen, Screen::Typing, "time ran out while paused");
        app.restart().unwrap();
        assert!(!app.is_paused());
        assert_eq!(app.paused_for, Duration::ZERO);
    }

    // ── practice (v0.7.0) ──────────────────────────────────────────────────

    /// F6 on Results types the same words again as a replay of the run
    /// just finished; a second F6 replays it again, not a replay of the
    /// replay. A run too long to keep its text still repeats nothing extra.
    #[test]
    fn repeat_types_the_same_words_as_a_replay() {
        let mut app = custom_app(&["red", "fox"]);
        app.started_at = Some(Instant::now() - Duration::from_secs(2));
        for ch in "red fox".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.screen, Screen::Results);
        let wpm = app.wpm();
        app.repeat_run();
        assert_eq!(app.screen, Screen::Typing);
        let words: Vec<_> = app.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(words, ["red", "fox"]);
        assert_eq!(app.session_label(), "custom+replay");
        assert_eq!(app.replay.as_ref().unwrap().wpm, wpm);
        app.end_early();
        app.repeat_run();
        assert_eq!(app.session_label(), "custom+replay");
        assert_eq!(app.replay.as_ref().unwrap().wpm, wpm, "still the first run");
    }

    /// Stopping on letters, a wrong key is a mistake that doesn't move the
    /// cursor; stopping on words, a word with a mistake can't be left — nor
    /// the run finished on one — until it's fixed.
    #[test]
    fn stop_on_error_letter_and_word() {
        let mut app = custom_app(&["ab", "cd"]);
        app.options.stop_on_error = StopOnError::Letter;
        app.handle_char('x');
        assert_eq!((app.words[0].typed.as_str(), app.rejected_key), ("", true));
        app.handle_char('a');
        app.handle_char('b');
        app.handle_char('x'); // where the space belongs
        assert_eq!(app.current_word, 0);
        app.handle_char(' ');
        assert_eq!((app.current_word, app.rejected_key), (1, false));
        assert_eq!((app.total_typed_chars, app.correct_chars), (5, 3));

        let mut app = custom_app(&["ab", "cd"]);
        app.options.stop_on_error = StopOnError::Word;
        app.handle_char('a');
        app.handle_char('x'); // typed, it's mid-word
        assert_eq!(app.words[0].typed, "ax");
        app.handle_char(' ');
        assert_eq!(app.current_word, 0, "can't leave a wrong word");
        app.handle_backspace(false);
        app.handle_char('b');
        app.handle_char(' ');
        assert_eq!(app.current_word, 1);
        app.handle_char('c');
        app.handle_char('x'); // last character, wrong
        assert_eq!(app.screen, Screen::Typing, "not finished on a mistake");
        app.handle_char('d'); // past the end: refused
        assert_eq!(app.words[1].typed, "cx");
        app.handle_backspace(false);
        app.handle_char('d');
        assert_eq!(app.screen, Screen::Results);
    }

    /// The bell is asked for on every key, on mistakes only (refused keys
    /// too), or never.
    #[test]
    fn key_sound_asks_for_the_bell() {
        for (sound, right, wrong) in [
            (KeySound::Off, false, false),
            (KeySound::Errors, false, true),
            (KeySound::Keys, true, true),
        ] {
            let mut app = custom_app(&["ab"]);
            app.options.sound = sound;
            app.handle_char('a');
            assert_eq!(app.bell, right, "{sound:?} right key");
            app.handle_char('x');
            assert_eq!(app.bell, wrong, "{sound:?} wrong key");
        }
    }

    // ── deeper results (v0.6.0) ────────────────────────────────────────────

    /// Each key lands in the second it was typed (paused time left out):
    /// counted as typed, and as right or wrong.
    #[test]
    fn keys_are_tallied_per_second() {
        let mut app = make_app_with_word("abcdef");
        app.started_at = Some(Instant::now() - Duration::from_millis(2500));
        app.handle_char('a');
        app.handle_char('x');
        assert_eq!(app.timeline.len(), 3);
        assert_eq!(app.timeline[0], Second::default());
        assert_eq!(
            app.timeline[2],
            Second {
                typed: 2,
                correct: 1,
                errors: 1
            }
        );
        app.restart().unwrap();
        assert!(app.timeline.is_empty());
    }

    /// Raw WPM counts every key; consistency is 100% for an even pace,
    /// lower for an uneven one, and needs two whole seconds.
    #[test]
    fn raw_wpm_and_consistency() {
        let mut app = make_app_with_word("abcdef");
        app.ended_at = Some(Instant::now());
        app.started_at = app.ended_at.map(|end| end - Duration::from_secs(60));
        app.total_typed_chars = 300;
        app.correct_chars = 250;
        assert!((app.raw_wpm() - 60.0).abs() < 1e-9);
        assert!((app.wpm() - 50.0).abs() < 1e-9);

        let even = Second {
            typed: 5,
            correct: 5,
            errors: 0,
        };
        app.timeline = vec![even; 60];
        assert_eq!(app.consistency(), Some(100.0));
        // Alternating 2 and 8 keys a second: mean 5, spread 3 → 40%.
        app.timeline = (0..60)
            .map(|i| Second {
                typed: if i % 2 == 0 { 2 } else { 8 },
                ..Second::default()
            })
            .collect();
        assert!((app.consistency().unwrap() - 40.0).abs() < 1e-9);
        // Seconds with no keys count, as zero: half the minute idle is a
        // spread as large as the mean.
        app.timeline = vec![even; 30];
        assert!(app.consistency().unwrap().abs() < 1e-9);

        app.started_at = app.ended_at.map(|end| end - Duration::from_millis(1900));
        assert_eq!(app.consistency(), None, "under two whole seconds");
    }

    /// Missed characters: what was left of a fixed text when the run ended
    /// early — the rest of the word, later words and the spaces before
    /// them. An endless run misses nothing.
    #[test]
    fn missed_characters_and_ending_early() {
        let mut app = custom_app(&["abc", "de", "fgh"]);
        for ch in "abc d".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.missed_chars(), 1 + 4); // "e", then " fgh"
        app.end_early();
        assert!(app.ended_early);
        assert_eq!(app.screen, Screen::Results);
        assert_eq!(app.missed, 5, "kept when the run ended");

        let mut app = custom_app(&["ab"]);
        app.handle_char('a');
        app.handle_char('b');
        assert_eq!(app.screen, Screen::Results);
        assert!(!app.ended_early, "finished by its last character");
        assert_eq!(app.missed_chars(), 0);
        app.end_early(); // Esc after the end changes nothing
        assert!(!app.ended_early);

        let mut app = menu_app();
        app.start_game(Mode::Time(15)).unwrap();
        assert_eq!(app.missed_chars(), 0);
    }

    /// A run counts as started on its first key — once, however many keys
    /// follow; zen doesn't count. Saved sessions from before counting began
    /// count too, and saving the count moves it to state.json.
    #[test]
    fn runs_started_are_counted_once_and_saved() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = menu_app();
        let old = chrono::Local::now() - chrono::Duration::days(3);
        app.stats_cache = Some(vec![
            SessionRecord {
                timestamp: old,
                ..SessionRecord::default()
            };
            2
        ]);
        assert_eq!(app.runs_started(), 2);
        app.start_game(Mode::Words(5)).unwrap();
        app.handle_char('x');
        app.handle_char('y');
        app.restart().unwrap();
        app.handle_char('x');
        app.start_game(Mode::Zen).unwrap();
        app.handle_char('x');
        assert_eq!(app.runs_started_unsaved, 2);
        assert_eq!(app.runs_started(), 4);

        // Another window saved 5 runs meanwhile: they add up.
        let file = dir.path().join("state.json");
        let other = state::AppState {
            runs_started: 5,
            counting_since: Some(old),
            ..Default::default()
        };
        state::save_to_path(&file, &other).unwrap();
        app.state_file = Some(file);
        app.save_run_count();
        assert_eq!(app.runs_started_unsaved, 0);
        let saved = state::load_from_path(&dir.path().join("state.json"));
        assert_eq!(saved.runs_started, 7);
        assert_eq!(saved.counting_since, Some(old), "the earliest start kept");
        // 7 counted since `old`; the two saved sessions at `old` aren't
        // before it, so they aren't added again.
        assert_eq!(app.runs_started(), 7);
    }

    // ── daily challenge (v0.5.0) ───────────────────────────────────────────

    /// The daily challenge types today's words, shows its progress, ends on
    /// the last word, is saved as "daily", and its best is limited to
    /// today's text. A replay of it is an ordinary replay.
    #[test]
    fn daily_challenge_types_todays_words() {
        let mut app = menu_app();
        app.start_game(Mode::Daily).unwrap();
        let today = crate::words::daily_words(chrono::Local::now().date_naive());
        let words: Vec<String> = app.words.iter().map(|w| w.text.clone()).collect();
        assert_eq!(words, today);
        assert_eq!(app.session_label(), "daily");
        assert_eq!(app.progress(), Some((0, crate::words::DAILY_WORDS)));
        assert_eq!(app.best_text(), Some(today.join(" ")));
        for ch in today.join(" ").chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.screen, Screen::Results);

        let record = SessionRecord {
            mode: "daily".into(),
            text: today.join(" "),
            ..SessionRecord::default()
        };
        app.start_replay(&record).unwrap();
        assert_eq!(app.mode, Mode::Daily);
        assert_eq!(app.session_label(), "daily+replay");
        assert_eq!(app.best_text(), None);
    }

    #[test]
    fn menu_offers_the_daily_challenge() {
        let menu = default_menu();
        let daily = best_menu_match(&menu, Mode::Daily);
        assert_eq!(menu[daily].label, "daily");
        assert_eq!(menu[daily].mode, Some(Mode::Daily));
    }

    // ── replay (v0.5.0) ────────────────────────────────────────────────────

    /// Every label TypeRush writes reads back as the mode it came from;
    /// word-setting and replay suffixes don't change the mode, and labels
    /// it never writes (or out-of-range counts) are `None`.
    #[test]
    fn mode_labels_read_back() {
        let menu = default_menu();
        for mode in menu.iter().filter_map(|item| item.mode) {
            assert_eq!(Mode::from_label(&mode.label()), Some(mode), "{mode:?}");
        }
        for mode in [Mode::Time(45), Mode::Words(7), Mode::Symbols(3)] {
            assert_eq!(Mode::from_label(&mode.label()), Some(mode));
        }
        assert_eq!(Mode::from_label("time-30s+10k+p+n"), Some(Mode::Time(30)));
        assert_eq!(Mode::from_label("words-50+p+replay"), Some(Mode::Words(50)));
        assert_eq!(Mode::from_label("custom-notes"), Some(Mode::Custom));
        for bad in [
            "",
            "time-0s",
            "time-99999s",
            "words-x",
            "code-cobol",
            "customs",
            "zenith",
        ] {
            assert_eq!(Mode::from_label(bad), None, "{bad:?}");
        }
    }

    fn record_with_text(mode: &str, text: &str) -> SessionRecord {
        SessionRecord {
            wpm: 62.5,
            mode: mode.into(),
            text: text.into(),
            ..SessionRecord::default()
        }
    }

    /// A replay types the original's words, is saved as `+replay`, ends on
    /// its last word, and restarting replays it again; any other game
    /// clears it.
    #[test]
    fn replay_types_the_same_words_again() {
        let mut app = menu_app();
        let record = record_with_text("words-3+p", "one, two three.");
        app.start_replay(&record).unwrap();
        let words = |app: &App| app.words.iter().map(|w| w.text.clone()).collect::<Vec<_>>();
        assert_eq!(words(&app), ["one,", "two", "three."]);
        assert_eq!(app.mode, Mode::Words(3));
        assert_eq!(app.session_label(), "words-3+p+replay");
        for ch in "one, two three.".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.screen, Screen::Results);
        assert_eq!(app.session_text(), "one, two three.");

        app.restart().unwrap();
        assert_eq!(words(&app), ["one,", "two", "three."]);
        assert_eq!(app.screen, Screen::Typing);

        // A replay of a replay is still just "+replay".
        app.start_replay(&record_with_text("words-3+p+replay", "a b c"))
            .unwrap();
        assert_eq!(app.session_label(), "words-3+p+replay");

        app.start_game(Mode::Words(3)).unwrap();
        assert!(app.replay.is_none());
        assert_eq!(app.session_label(), "words-3");
    }

    /// A time replay runs on the clock: the original's words first, then
    /// fresh ones played the way the original was (its `+10k`/`+p`/`+n`),
    /// whatever the menu's settings are now.
    #[test]
    fn time_replay_continues_with_the_originals_settings() {
        let mut app = menu_app();
        app.start_replay(&record_with_text("time-15s+n", "alpha beta"))
            .unwrap();
        assert_eq!(app.mode, Mode::Time(15));
        assert_eq!(app.words[0].text, "alpha");
        assert_eq!(app.words[1].text, "beta");
        assert!(app.words.len() >= 100, "topped up");
        let replay = app.replay.as_ref().unwrap();
        assert_eq!(replay.pool, WordPool::Common);
        assert!(replay.decor.numbers && !replay.decor.punctuation);
        // Only the words reached are kept when a time run is saved.
        for ch in "alpha be".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.session_text(), "alpha beta");
    }

    /// Replay text comes from stats.json: untypeable characters are dropped
    /// like a custom file's, a record without text explains itself, an
    /// oversized one is refused, and a failed replay changes nothing.
    #[test]
    fn replay_text_is_checked() {
        let mut app = menu_app();
        app.start_replay(&record_with_text("quote", "go\u{1b}[2J \u{feff} fast"))
            .unwrap();
        let words: Vec<_> = app.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(words, ["go[2J", "fast"]);

        app.start_game(Mode::Words(5)).unwrap();
        let error = app
            .start_replay(&record_with_text("time-30s", ""))
            .unwrap_err();
        assert!(error.to_string().contains("no words saved"), "{error}");
        let huge = "word ".repeat(crate::storage::MAX_TEXT_BYTES / 4);
        assert!(app
            .start_replay(&record_with_text("words-10", &huge))
            .is_err());
        assert_eq!(app.mode, Mode::Words(5));
        assert!(app.replay.is_none());
    }

    /// A count mode whose text is shorter than its count (a hand-edited
    /// record) still ends on the last word instead of ignoring every key.
    #[test]
    fn short_replay_still_finishes() {
        let mut app = menu_app();
        app.start_replay(&record_with_text("words-50", "hi"))
            .unwrap();
        app.handle_char('h');
        app.handle_char('i');
        assert_eq!(app.screen, Screen::Results);
    }

    /// The saved text of a run too long to keep is empty, not cut short.
    #[test]
    fn oversized_session_text_is_not_kept() {
        let mut app = menu_app();
        app.start_game(Mode::Words(1)).unwrap();
        app.words = vec![Word::new("x".repeat(crate::storage::MAX_TEXT_BYTES + 1))];
        assert_eq!(app.session_text(), "");
        app.words = vec![Word::new("x".repeat(crate::storage::MAX_TEXT_BYTES))];
        assert_eq!(app.session_text().len(), crate::storage::MAX_TEXT_BYTES);
    }

    // ── per-key accuracy tracking (v0.3.0) ──────────────────────────────────

    fn make_app_with_word(word: &str) -> App {
        let mut app = App::new(None, crate::theme::ThemePalette::default(), Mode::Time(15));
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
        let mut app = App::new(None, palette, Mode::Time(15));
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
        let mut app = App::new(None, palette, Mode::Time(15));
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

    // ── v0.4.0 ──────────────────────────────────────────────────────────────

    fn snippet(name: &str, path: &str) -> Snippet {
        Snippet {
            name: name.into(),
            path: PathBuf::from(path),
            canonical_path: None,
        }
    }

    fn custom_options(menu: &[MenuItem]) -> Vec<(&str, Option<&Path>)> {
        menu.iter()
            .filter(|m| m.action == MenuAction::StartCustom)
            .map(|m| (m.label.as_str(), m.custom_path.as_deref()))
            .collect()
    }

    #[test]
    fn mode_labels_for_v04_modes_are_stable() {
        use crate::words::CodeLang;
        assert_eq!(Mode::Symbols(25).label(), "symbols-25");
        assert_eq!(Mode::Code(CodeLang::Go).label(), "code-go");
        assert_eq!(Mode::Code(CodeLang::Java).label(), "code-java");
        assert_eq!(Mode::Code(CodeLang::Sql).label(), "code-sql");
        assert_eq!(Mode::Code(CodeLang::Shell).label(), "code-shell");
        // Unchanged since v0.1 — saved PBs are keyed on it.
        assert_eq!(Mode::Code(CodeLang::JavaScript).label(), "code-javascript");
    }

    #[test]
    fn menu_offers_new_languages_and_symbols() {
        let menu = default_menu();
        let row = |group: &str| -> Vec<&str> {
            menu.iter()
                .filter(|m| m.group == group)
                .map(|m| m.label.as_str())
                .collect()
        };
        assert_eq!(
            row("code"),
            ["rust", "python", "js", "go", "java", "sql", "shell"]
        );
        assert_eq!(row("symbols"), ["25", "50"]);
        // Rows with a choice of options first, then the single runs, then
        // the actions.
        let mut groups: Vec<&str> = menu.iter().map(|m| m.group).collect();
        groups.dedup();
        assert_eq!(
            groups,
            ["time", "words", "code", "symbols", "custom", "daily", "quote", "zen", "app"]
        );
        assert_eq!(row("daily"), ["daily"]);
        assert_eq!(row("app"), ["stats", "profile", "settings", "quit"]);
    }

    #[test]
    fn best_menu_match_picks_symbols_row() {
        let menu = default_menu();
        assert_eq!(
            menu[best_menu_match(&menu, Mode::Symbols(50))].mode,
            Some(Mode::Symbols(50))
        );
        // A non-standard count falls back to the first symbols option.
        assert_eq!(
            menu[best_menu_match(&menu, Mode::Symbols(7))].mode,
            Some(Mode::Symbols(25))
        );
    }

    #[test]
    fn custom_row_placeholder_when_nothing_to_offer() {
        assert_eq!(custom_options(&default_menu()), [("custom", None)]);
    }

    #[test]
    fn custom_row_lists_last_file_then_snippets() {
        let snippets = [
            snippet("alpha", "/s/alpha.txt"),
            snippet("beta", "/s/beta.txt"),
        ];
        let menu = build_menu(&snippets, Some(Path::new("/home/me/notes.txt")));
        assert_eq!(
            custom_options(&menu),
            [
                ("notes.txt", Some(Path::new("/home/me/notes.txt"))),
                ("alpha", Some(Path::new("/s/alpha.txt"))),
                ("beta", Some(Path::new("/s/beta.txt"))),
            ]
        );
    }

    /// The remembered file is often one of the snippets — don't list it twice.
    #[test]
    fn last_file_that_is_a_snippet_is_not_duplicated() {
        let snippets = [snippet("alpha", "/s/alpha.txt")];
        let menu = build_menu(&snippets, Some(Path::new("/s/alpha.txt")));
        assert_eq!(
            custom_options(&menu),
            [("alpha", Some(Path::new("/s/alpha.txt")))]
        );
    }

    #[test]
    fn symbols_mode_ends_after_its_token_count() {
        let mut app = custom_app(&["()", "=>", "{}"]);
        app.mode = Mode::Symbols(2);
        assert_eq!(app.progress(), Some((0, 2)));
        for ch in "() =>".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.screen, Screen::Typing, "one token left");
        app.handle_char(' ');
        assert_eq!(app.screen, Screen::Results);
        assert_eq!(app.progress(), Some((2, 2)));
    }

    #[test]
    fn start_game_symbols_builds_requested_tokens() {
        let mut app = menu_app();
        app.start_game(Mode::Symbols(25)).unwrap();
        assert_eq!(app.words.len(), 25);
        assert!(app
            .words
            .iter()
            .all(|w| !w.text.contains(char::is_whitespace)));
    }

    #[test]
    fn word_source_settings_apply_but_zen_stays_plain() {
        let mut app = menu_app();
        app.set_word_source(
            WordPool::Extended,
            WordDecor {
                punctuation: true,
                numbers: true,
            },
        );
        app.start_game(Mode::Words(400)).unwrap();
        assert!(app
            .words
            .iter()
            .any(|w| w.text.chars().any(|c| !c.is_ascii_alphabetic())));
        // Zen ignores all three: plain words from the common pool.
        app.start_game(Mode::Zen).unwrap();
        assert!(app
            .words
            .iter()
            .all(|w| crate::words::english::ENGLISH_COMMON.contains(&w.text.as_str())));
    }

    /// A custom file that fails to load leaves the app exactly as it was:
    /// same screen, mode and words — and a restart still uses the file that
    /// last worked, not the one that just failed.
    #[test]
    fn failed_custom_start_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.txt");
        std::fs::write(&good, "one two").unwrap();
        let mut app = menu_app();
        app.start_custom(Some(good.clone())).unwrap();
        app.screen = Screen::Menu;
        let (mode, words) = (app.mode, app.words.len());
        let remembered = app.app_state.clone();

        let err = app
            .start_custom(Some(PathBuf::from("/definitely/not/here.txt")))
            .unwrap_err();
        assert!(format!("{err:#}").contains("here.txt"), "{err:#}");
        assert_eq!(app.screen, Screen::Menu);
        assert_eq!((app.mode, app.words.len()), (mode, words));
        assert_eq!(app.custom_file.as_deref(), Some(good.as_path()));
        assert_eq!(app.app_state, remembered);
    }

    #[test]
    fn custom_without_file_explains_how_to_add_one() {
        let mut app = menu_app();
        let err = app.start_custom(None).unwrap_err().to_string();
        assert!(err.contains("--file"), "{err}");
        assert!(err.contains("snippets"), "{err}");
        assert_eq!(app.screen, Screen::Menu);
    }

    #[test]
    fn empty_custom_file_names_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blank.txt");
        std::fs::write(&path, "  \n\t ").unwrap();
        let mut app = menu_app();
        let err = app.start_custom(Some(path)).unwrap_err().to_string();
        assert!(err.contains("blank.txt") && err.contains("empty"), "{err}");
    }

    /// A successful start saves the file to state.json, offers it in the
    /// menu, and doesn't rewrite the file when the same one starts again.
    #[test]
    fn successful_custom_start_is_remembered_once() {
        let dir = tempfile::tempdir().unwrap();
        let state_file = dir.path().join("state.json");
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "type me").unwrap();
        let mut app = menu_app();
        app.load_custom_sources(Vec::new(), AppState::default(), Some(state_file.clone()));

        app.start_custom(Some(file.clone())).unwrap();
        assert_eq!(app.screen, Screen::Typing);
        assert_eq!(
            state::load_from_path(&state_file)
                .last_custom_file
                .as_deref(),
            file.to_str()
        );
        assert_eq!(
            custom_options(&app.menu),
            [("notes.txt", Some(file.as_path()))]
        );

        // Same file again (e.g. picked from the menu): no rewrite.
        std::fs::remove_file(&state_file).unwrap();
        app.start_custom(Some(file)).unwrap();
        assert!(!state_file.exists());
    }

    /// A relative `--file` path is remembered as absolute, so it still works
    /// when TypeRush is next started from another directory.
    #[test]
    fn relative_paths_are_made_absolute() {
        let stored = absolute(PathBuf::from("relative-notes.txt"));
        assert!(stored.is_absolute());
        assert!(stored.ends_with("relative-notes.txt"));
        let already = std::env::current_dir().unwrap().join("x.txt");
        assert_eq!(absolute(already.clone()), already);
    }

    /// A state.json write that fails isn't treated as saved: the next start
    /// of the same file tries again (and succeeds once the disk is fine).
    #[test]
    fn failed_state_save_is_retried() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "type me").unwrap();
        // A regular file where the state directory should be: the write fails.
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "").unwrap();
        let mut app = menu_app();
        app.load_custom_sources(
            Vec::new(),
            AppState::default(),
            Some(blocker.join("state.json")),
        );
        app.start_custom(Some(file.clone())).unwrap();
        assert!(app.app_state.last_custom_file.is_none());
        // The file is still offered for this run.
        assert_eq!(custom_options(&app.menu)[0].1, Some(file.as_path()));

        let state_file = dir.path().join("state.json");
        app.state_file = Some(state_file.clone());
        app.start_custom(Some(file.clone())).unwrap();
        assert_eq!(
            state::load_from_path(&state_file)
                .last_custom_file
                .as_deref(),
            file.to_str()
        );
    }

    /// No two custom options ever share a label, even when file managers'
    /// copy names ("notes (2)") collide with the numbering.
    #[test]
    fn custom_option_labels_are_unique() {
        let snippets = [
            snippet("notes", "/s/notes.txt"),
            snippet("notes (2)", "/s/notes (2).txt"),
            snippet("notes", "/s/notes.TXT"),
            snippet("notes.txt", "/s/notes.txt.txt"),
        ];
        let menu = build_menu(&snippets, Some(Path::new("/home/me/notes.txt")));
        let labels: Vec<&str> = custom_options(&menu).iter().map(|(l, _)| *l).collect();
        // The remembered file's name is taken by a snippet: its folder tells
        // it apart. "notes (2)" is a real file and keeps its name; the
        // repeated "notes.txt" gets the first number not already in use.
        let remembered = format!("me{}notes.txt", std::path::MAIN_SEPARATOR);
        assert_eq!(
            labels,
            [
                remembered.as_str(),
                "notes.txt",
                "notes (2)",
                "notes.TXT",
                "notes.txt (2)",
            ]
        );
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(unique.len(), labels.len());
    }

    fn snippet_labels(snippets: &[Snippet]) -> Vec<String> {
        custom_labels(snippets, None).0
    }

    /// A relative path in a hand-edited state.json is made absolute, so it
    /// still gets its folder when a snippet uses its name.
    #[test]
    fn relative_remembered_file_gets_its_folder() {
        let mut app = menu_app();
        app.load_custom_sources(
            vec![snippet("notes.txt", "/s/notes.txt.txt")],
            AppState {
                last_custom_file: Some("notes.txt".into()),
                ..Default::default()
            },
            None,
        );
        let cwd = std::env::current_dir().unwrap();
        let folder = cwd.file_name().unwrap().to_string_lossy();
        assert_eq!(
            custom_options(&app.menu)[0].0,
            format!("{folder}{}notes.txt", std::path::MAIN_SEPARATOR)
        );
    }

    /// Two long names that differ only in the middle would look identical
    /// once shortened; the shown labels must still differ.
    #[test]
    fn shortened_labels_stay_distinct() {
        let snippets = [
            snippet("project-alpha-meeting-notes-final", "/s/a.txt"),
            snippet("project-alpha-sprint-notes-final", "/s/b.txt"),
        ];
        let labels = snippet_labels(&snippets);
        assert_ne!(labels[0], labels[1]);
        for label in &labels {
            assert!(unicode_width::UnicodeWidthStr::width(label.as_str()) <= MAX_OPTION_WIDTH);
        }
    }

    /// Unsafe characters in a snippet name are shown as `?` in the menu.
    #[test]
    fn labels_are_terminal_safe() {
        let labels = snippet_labels(&[snippet("evil\u{1b}[2J", "/s/e.txt")]);
        assert_eq!(labels, ["evil?[2J"]);
    }

    /// Which file is remembered never changes a snippet's label.
    #[test]
    fn snippet_labels_do_not_depend_on_the_remembered_file() {
        let snippets = [
            snippet("notes", "/s/notes.txt"),
            snippet("todo", "/s/todo.txt"),
        ];
        let labels = |last: Option<&str>| -> Vec<String> {
            build_menu(&snippets, last.map(Path::new))
                .into_iter()
                .filter(|m| {
                    m.custom_path
                        .as_deref()
                        .is_some_and(|p| p.starts_with("/s"))
                })
                .map(|m| m.label)
                .collect()
        };
        assert_eq!(labels(None), ["notes", "todo"]);
        assert_eq!(labels(Some("/home/me/notes")), ["notes", "todo"]);
        assert_eq!(labels(Some("/home/me/todo")), ["notes", "todo"]);
    }

    /// A single snippet named `custom.txt` still gets the `custom` heading
    /// and opens the file — it isn't mistaken for the placeholder.
    #[test]
    fn snippet_named_custom_is_a_real_option() {
        let menu = build_menu(&[snippet("custom", "/s/custom.txt")], None);
        assert_eq!(
            custom_options(&menu),
            [("custom", Some(Path::new("/s/custom.txt")))]
        );
    }

    /// Decorated time / words runs are saved under their own label, so they
    /// never compete with plain runs for a personal best. Plain labels and
    /// all other modes are unchanged.
    #[test]
    fn session_label_marks_harder_word_settings() {
        let mut app = menu_app();
        app.mode = Mode::Time(30);
        assert_eq!(app.session_label(), "time-30s");
        app.set_word_source(
            WordPool::Extended,
            WordDecor {
                punctuation: true,
                numbers: true,
            },
        );
        assert_eq!(app.session_label(), "time-30s+10k+p+n");
        app.mode = Mode::Words(100);
        assert_eq!(app.session_label(), "words-100+10k+p+n");
        app.set_word_source(
            WordPool::Common,
            WordDecor {
                punctuation: false,
                numbers: true,
            },
        );
        assert_eq!(app.session_label(), "words-100+n");
        // Settings that don't apply to a mode don't change its label.
        for mode in [Mode::Quote, Mode::Symbols(25), Mode::Custom, Mode::Zen] {
            app.mode = mode;
            assert_eq!(app.session_label(), mode.label());
        }
    }

    /// Each custom file has its own label (and so its own personal best),
    /// named after the file without its extension and made safe to draw.
    #[test]
    fn custom_sessions_are_labelled_by_file() {
        let mut app = menu_app();
        app.mode = Mode::Custom;
        assert_eq!(app.session_label(), "custom", "no file yet");
        app.custom_file = Some(PathBuf::from("/home/me/notes.txt"));
        assert_eq!(app.session_label(), "custom-notes");
        app.custom_file = Some(PathBuf::from("/s/long essay.TXT"));
        assert_eq!(app.session_label(), "custom-long essay");
        app.custom_file = Some(PathBuf::from("/s/evil\u{1b}[2J.txt"));
        assert_eq!(app.session_label(), "custom-evil?[2J");
        // Word settings never apply to custom text.
        app.set_word_source(
            WordPool::Extended,
            WordDecor {
                punctuation: true,
                numbers: true,
            },
        );
        assert_eq!(app.session_label(), "custom-evil?[2J");
    }

    /// Rebuilding the menu keeps a highlight on quit on quit — stats and
    /// quit share mode and path (`None`), so the action must be compared too.
    #[test]
    fn rebuild_keeps_quit_highlighted() {
        let mut app = menu_app();
        app.menu_index = app.menu.iter().position(|m| m.label == "quit").unwrap();
        app.load_custom_sources(
            vec![snippet("alpha", "/s/alpha.txt")],
            AppState::default(),
            None,
        );
        assert_eq!(app.menu[app.menu_index].label, "quit");
    }

    /// Startup: the remembered file shows up, and the highlighted default
    /// mode stays highlighted after the menu is rebuilt.
    #[test]
    fn load_custom_sources_offers_remembered_file_and_keeps_selection() {
        let mut app = menu_app(); // highlights time · 30s
        app.load_custom_sources(
            vec![snippet("alpha", "/s/alpha.txt")],
            AppState {
                last_custom_file: Some("/home/me/notes.txt".into()),
                ..Default::default()
            },
            None,
        );
        assert_eq!(app.menu[app.menu_index].label, "30s");
        // Made absolute: on Windows `/home/me/…` has no drive, so it gains one.
        let remembered = absolute(PathBuf::from("/home/me/notes.txt"));
        assert_eq!(
            custom_options(&app.menu),
            [
                ("notes.txt", Some(remembered.as_path())),
                ("alpha", Some(Path::new("/s/alpha.txt"))),
            ]
        );
    }

    /// `--file` wins over the remembered file in the custom row.
    #[test]
    fn cli_file_is_offered_instead_of_remembered_one() {
        let given = absolute(PathBuf::from("/cli/given.txt"));
        let mut app = App::new(
            Some(given.clone()),
            crate::theme::ThemePalette::default(),
            Mode::Time(15),
        );
        app.load_custom_sources(
            Vec::new(),
            AppState {
                last_custom_file: Some("/old/remembered.txt".into()),
                ..Default::default()
            },
            None,
        );
        assert_eq!(
            custom_options(&app.menu),
            [("given.txt", Some(given.as_path()))]
        );
    }
}
