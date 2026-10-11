//! The Settings screen's state and actions: what `config.toml` says, and
//! changing it. Every change is saved at once (through `config::edit`, so
//! the file keeps its comments) and used at once — a new theme shows as you
//! pick it.

use std::path::Path;

use crate::app::{best_menu_match, App, MenuAction, Mode};
use crate::config::edit;
use crate::config::load::{load_from, CliOverrides, ResolvedConfig};
use crate::theme::builtin;
use crate::words::{WordDecor, WordPool};

/// One row of the Settings screen, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Theme,
    Start,
    Punctuation,
    Numbers,
    Pool,
    LiveWpm,
    Blind,
    StopOnError,
    Pace,
    Cursor,
    Sound,
    Reset,
}

/// Every row, in order.
pub const ROWS: [Row; 12] = [
    Row::Theme,
    Row::Start,
    Row::Punctuation,
    Row::Numbers,
    Row::Pool,
    Row::LiveWpm,
    Row::Blind,
    Row::StopOnError,
    Row::Pace,
    Row::Cursor,
    Row::Sound,
    Row::Reset,
];

/// What the Settings screen shows: the saved settings, read from the
/// config file when the screen opens and after each change (never per
/// frame), and where the selection is.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsView {
    /// Index into [`ROWS`].
    pub selected: usize,
    /// Enter was pressed once on "reset to defaults": the next Enter resets.
    pub confirm_reset: bool,
    /// The saved theme's name.
    pub theme: String,
    /// The mode the menu starts on.
    pub start: Mode,
    pub pool: WordPool,
    pub decor: WordDecor,
    /// The `[typing]` settings.
    pub typing: crate::options::TypingOptions,
    /// The first problem the loader has with the file (it doesn't parse, a
    /// value is out of range…), shown on the screen.
    pub problem: Option<String>,
}

impl SettingsView {
    /// The settings saved at `path` (`None`: nowhere to save, so the
    /// built-in defaults), with the selection on row `selected`.
    pub fn load(path: Option<&Path>, selected: usize) -> Self {
        let (resolved, warnings) = match path {
            Some(path) => load_from(path, CliOverrides::default()),
            None => (ResolvedConfig::default(), Vec::new()),
        };
        let theme = path
            .and_then(|path| edit::read(path).ok().flatten())
            .and_then(|config| {
                edit::effective(&config)
                    .into_iter()
                    .find(|lookup| lookup.key.name == "theme")
            })
            .map_or_else(|| "dark".to_string(), |lookup| lookup.value.plain());
        Self {
            selected: selected.min(ROWS.len() - 1),
            confirm_reset: false,
            theme,
            start: resolved.default_mode,
            pool: resolved.word_pool,
            decor: resolved.word_decor,
            typing: resolved.typing,
            problem: warnings.into_iter().next(),
        }
    }

    /// The row the selection is on.
    pub fn row(&self) -> Row {
        ROWS[self.selected]
    }
}

/// `time 30s`, `code javascript`, `daily`: a start mode as the screen shows it.
pub fn mode_name(mode: Mode) -> String {
    mode.label().replacen('-', " ", 1)
}

impl App {
    /// The Settings view, loaded from the config file if the screen hasn't
    /// been entered yet.
    fn settings_view(&mut self) -> &mut SettingsView {
        let path = self.config_file.clone();
        self.settings
            .get_or_insert_with(|| SettingsView::load(path.as_deref(), 0))
    }

    /// ↑/↓: move the selection by `rows`, stopping at either end. Moving
    /// away from "reset to defaults" cancels a pending reset.
    pub fn settings_move(&mut self, rows: isize) {
        let view = self.settings_view();
        view.selected = view
            .selected
            .saturating_add_signed(rows)
            .min(ROWS.len() - 1);
        view.confirm_reset = false;
    }

    /// Select row `row` (clicked).
    pub fn settings_select(&mut self, row: usize) {
        let view = self.settings_view();
        if row < ROWS.len() && row != view.selected {
            view.selected = row;
            view.confirm_reset = false;
        }
    }

    /// ←/→ on the selected row: the previous / next theme or start mode, a
    /// switch flipped. Does nothing on "reset to defaults".
    pub fn settings_change(&mut self, forward: bool) {
        let view = self.settings_view().clone();
        match view.row() {
            Row::Theme => {
                let names = builtin::names();
                let at = names.iter().position(|name| *name == view.theme);
                let name = names[cycle(at, names.len(), forward)];
                if let Some(resolved) = self.save_settings(&[("theme", name.to_string())]) {
                    // Saved theme plus the file's color overrides.
                    self.theme = resolved.palette;
                }
            }
            Row::Start => {
                let modes = self.start_modes();
                let at = modes.iter().position(|mode| *mode == view.start);
                let mode = modes[cycle(at, modes.len(), forward)];
                if self.save_settings(&start_settings(mode)).is_some() {
                    self.menu_index = best_menu_match(&self.menu, mode);
                }
            }
            Row::Punctuation => {
                let on = !view.decor.punctuation;
                if self
                    .save_settings(&[("words.punctuation", on.to_string())])
                    .is_some()
                {
                    self.word_decor.punctuation = on;
                }
            }
            Row::Numbers => {
                let on = !view.decor.numbers;
                if self
                    .save_settings(&[("words.numbers", on.to_string())])
                    .is_some()
                {
                    self.word_decor.numbers = on;
                }
            }
            Row::Pool => {
                let (pool, name) = match view.pool {
                    WordPool::Common => (WordPool::Extended, "extended"),
                    WordPool::Extended => (WordPool::Common, "common"),
                };
                if self
                    .save_settings(&[("words.pool", name.to_string())])
                    .is_some()
                {
                    self.word_pool = pool;
                }
            }
            Row::LiveWpm => {
                let on = !view.typing.live_wpm;
                if self
                    .save_settings(&[("typing.live_wpm", on.to_string())])
                    .is_some()
                {
                    self.options.live_wpm = on;
                }
            }
            Row::Blind => {
                let on = !view.typing.blind;
                if self
                    .save_settings(&[("typing.blind", on.to_string())])
                    .is_some()
                {
                    self.options.blind = on;
                }
            }
            Row::StopOnError => {
                use crate::options::StopOnError;
                let names = StopOnError::NAMES;
                let at = names
                    .iter()
                    .position(|n| *n == view.typing.stop_on_error.name());
                let name = names[cycle(at, names.len(), forward)];
                if self
                    .save_settings(&[("typing.stop_on_error", name.to_string())])
                    .is_some()
                {
                    self.options.stop_on_error = StopOnError::from_name(name).unwrap_or_default();
                }
            }
            Row::Pace => {
                use crate::options::Pace;
                let at = Pace::CHOICES.iter().position(|p| *p == view.typing.pace);
                let pace = Pace::CHOICES[cycle(at, Pace::CHOICES.len(), forward)];
                if self
                    .save_settings(&[("typing.pace", pace.name())])
                    .is_some()
                {
                    self.options.pace = pace;
                }
            }
            Row::Cursor => {
                use crate::options::CursorStyle;
                let names = CursorStyle::NAMES;
                let at = names.iter().position(|n| *n == view.typing.cursor.name());
                let name = names[cycle(at, names.len(), forward)];
                if self
                    .save_settings(&[("typing.cursor", name.to_string())])
                    .is_some()
                {
                    self.options.cursor = CursorStyle::from_name(name).unwrap_or_default();
                }
            }
            Row::Sound => {
                use crate::options::KeySound;
                let names = KeySound::NAMES;
                let at = names.iter().position(|n| *n == view.typing.sound.name());
                let name = names[cycle(at, names.len(), forward)];
                if self
                    .save_settings(&[("typing.sound", name.to_string())])
                    .is_some()
                {
                    self.options.sound = KeySound::from_name(name).unwrap_or_default();
                }
            }
            Row::Reset => {}
        }
    }

    /// Enter / Space on the selected row: the next value, or on "reset to
    /// defaults" ask first, and reset on the second press.
    pub fn settings_activate(&mut self) {
        let view = self.settings_view();
        if view.row() != Row::Reset {
            return self.settings_change(true);
        }
        if !view.confirm_reset {
            view.confirm_reset = true;
            return;
        }
        self.reset_settings();
    }

    /// Esc on the Settings screen: cancel a pending reset (`true`), or
    /// `false` when there is none (leave the screen).
    pub fn settings_cancel(&mut self) -> bool {
        std::mem::take(&mut self.settings_view().confirm_reset)
    }

    /// Every start mode the menu offers, in menu order (custom files left
    /// out: a start mode is a kind of run, not a file).
    fn start_modes(&self) -> Vec<Mode> {
        let mut modes: Vec<Mode> = Vec::new();
        for item in &self.menu {
            if let (MenuAction::Start, Some(mode)) = (item.action, item.mode) {
                if !modes.contains(&mode) {
                    modes.push(mode);
                }
            }
        }
        modes
    }

    /// Save `settings` (name, value) to the config file and reload the
    /// view; the config as it now resolves. On failure show the error and
    /// change nothing else (`None`); with no file to save to, `None` too.
    fn save_settings(&mut self, settings: &[(&str, String)]) -> Option<ResolvedConfig> {
        let selected = self.settings_view().selected;
        let path = self.config_file.clone()?;
        for (name, value) in settings {
            if let Err(error) = edit::set(&path, name, value) {
                self.show_error(&error);
                return None;
            }
        }
        self.settings = Some(SettingsView::load(Some(&path), selected));
        Some(load_from(&path, CliOverrides::default()).0)
    }

    /// "reset to defaults": move the config file aside (it is kept, with
    /// the time in its name) and use the built-in defaults from now on.
    fn reset_settings(&mut self) {
        let selected = self.settings_view().selected;
        let Some(path) = self.config_file.clone() else {
            return;
        };
        match edit::reset_all(&path) {
            Ok(Some(backup)) => {
                self.notice = Some(format!(
                    "every setting is back to its default; your old file is at {}",
                    crate::text::screen_path(&backup)
                ));
            }
            Ok(None) => {}
            Err(error) => return self.show_error(&error),
        }
        let defaults = ResolvedConfig::default();
        self.theme = defaults.palette;
        self.word_pool = defaults.word_pool;
        self.word_decor = defaults.word_decor;
        self.options = defaults.typing;
        self.menu_index = best_menu_match(&self.menu, defaults.default_mode);
        self.settings = Some(SettingsView::load(Some(&path), selected));
    }
}

/// The settings that make `mode` the start mode: its name, and its count
/// or language where it has one.
fn start_settings(mode: Mode) -> Vec<(&'static str, String)> {
    let name = match mode {
        Mode::Time(_) => "time",
        Mode::Words(_) => "words",
        Mode::Quote => "quote",
        Mode::Code(_) => "code",
        Mode::Zen => "zen",
        Mode::Symbols(_) => "symbols",
        Mode::Daily => "daily",
        // Never offered (see `start_modes`).
        Mode::Custom => "time",
    };
    let mut settings = vec![("defaults.mode", name.to_string())];
    match mode {
        Mode::Time(seconds) => settings.push(("defaults.time_seconds", seconds.to_string())),
        Mode::Words(count) => settings.push(("defaults.word_count", count.to_string())),
        Mode::Symbols(count) => settings.push(("defaults.symbol_count", count.to_string())),
        Mode::Code(_) => {
            settings.push(("defaults.code_lang", mode.label()["code-".len()..].into()))
        }
        _ => {}
    }
    settings
}

/// Index after (or before) `at` in a list of `len`, wrapping; the first
/// (or last) when `at` isn't in the list.
fn cycle(at: Option<usize>, len: usize, forward: bool) -> usize {
    match (at, forward) {
        (Some(at), true) => (at + 1) % len,
        (Some(at), false) => (at + len - 1) % len,
        (None, true) => 0,
        (None, false) => len - 1,
    }
}
