//! Changing `~/.typerush/config.toml` without opening it: the
//! `typerush config` subcommands and the Settings screen both come here.
//!
//! Values are checked with the same rules the loader applies when TypeRush
//! starts, so anything `set` accepts loads without a warning. The file is
//! edited in place — comments, order and layout are kept — and written
//! atomically, like every file TypeRush writes.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use toml_edit::{DocumentMut, Item, Table, Value};

use super::load::{
    check_count, code_lang, read_config, word_pool, DEFAULT_SECONDS, DEFAULT_SYMBOLS,
    DEFAULT_WORDS, MAX_COUNT, MAX_SECONDS, MODE_NAMES,
};
use super::Config;
use crate::app::Mode;
use crate::options::{CursorStyle, KeySound, StopOnError};
use crate::storage::write_atomic;
use crate::text::printable;
use crate::theme::{builtin, color, ThemePalette};
use crate::words::WordPool;

/// What a setting holds, which decides how a value is checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A built-in theme name.
    Theme,
    /// One of [`MODE_NAMES`].
    Mode,
    /// A time run's length: 1 to [`MAX_SECONDS`].
    Seconds,
    /// A words or symbols count: 1 to [`MAX_COUNT`].
    Count,
    /// A code language name.
    CodeLang,
    /// A `[words] pool` name.
    Pool,
    /// On or off.
    Switch,
    /// A color: `#RRGGBB`, an ANSI name or `reset`.
    Color,
    /// One of a fixed set of names.
    Choice(&'static [&'static str]),
}

/// One setting of `config.toml`, by its dotted name (`defaults.mode`).
#[derive(Debug, PartialEq, Eq)]
pub struct Key {
    pub name: &'static str,
    pub kind: Kind,
}

/// Every setting, in the order `config show` lists them. (`colors.extra`
/// is left out: it is still read, so old files load, but draws nothing.)
pub const KEYS: &[Key] = &[
    Key {
        name: "theme",
        kind: Kind::Theme,
    },
    Key {
        name: "defaults.mode",
        kind: Kind::Mode,
    },
    Key {
        name: "defaults.time_seconds",
        kind: Kind::Seconds,
    },
    Key {
        name: "defaults.word_count",
        kind: Kind::Count,
    },
    Key {
        name: "defaults.code_lang",
        kind: Kind::CodeLang,
    },
    Key {
        name: "defaults.symbol_count",
        kind: Kind::Count,
    },
    Key {
        name: "words.pool",
        kind: Kind::Pool,
    },
    Key {
        name: "words.punctuation",
        kind: Kind::Switch,
    },
    Key {
        name: "words.numbers",
        kind: Kind::Switch,
    },
    Key {
        name: "typing.live_wpm",
        kind: Kind::Switch,
    },
    Key {
        name: "typing.blind",
        kind: Kind::Switch,
    },
    Key {
        name: "typing.stop_on_error",
        kind: Kind::Choice(&StopOnError::NAMES),
    },
    Key {
        name: "typing.cursor",
        kind: Kind::Choice(&CursorStyle::NAMES),
    },
    Key {
        name: "typing.sound",
        kind: Kind::Choice(&KeySound::NAMES),
    },
    Key {
        name: "colors.accent",
        kind: Kind::Color,
    },
    Key {
        name: "colors.secondary",
        kind: Kind::Color,
    },
    Key {
        name: "colors.correct",
        kind: Kind::Color,
    },
    Key {
        name: "colors.incorrect",
        kind: Kind::Color,
    },
    Key {
        name: "colors.pending",
        kind: Kind::Color,
    },
    Key {
        name: "colors.mode_tag",
        kind: Kind::Color,
    },
    Key {
        name: "colors.error",
        kind: Kind::Color,
    },
    Key {
        name: "colors.neutral",
        kind: Kind::Color,
    },
    Key {
        name: "colors.background",
        kind: Kind::Color,
    },
];

/// The setting called `name`, or an error that says where to find them.
pub fn key(name: &str) -> Result<&'static Key> {
    KEYS.iter().find(|key| key.name == name).ok_or_else(|| {
        anyhow!(
            "unknown setting '{}' (`typerush config show` lists them all)",
            printable(name)
        )
    })
}

/// A checked value, in the form it is written to the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Setting {
    Text(String),
    Number(i64),
    Switch(bool),
}

impl Setting {
    /// As `config get` prints it: `monokai`, `30`, `true`.
    pub fn plain(&self) -> String {
        match self {
            Setting::Text(text) => text.clone(),
            Setting::Number(number) => number.to_string(),
            Setting::Switch(on) => on.to_string(),
        }
    }

    /// As TOML: `"monokai"`, `30`, `true`. Checked text never holds a quote
    /// or a backslash (names, `#RRGGBB`), so quoting it is enough.
    pub fn toml(&self) -> String {
        match self {
            Setting::Text(text) => format!("\"{text}\""),
            other => other.plain(),
        }
    }

    fn value(&self) -> Value {
        match self {
            Setting::Text(text) => Value::from(text.as_str()),
            Setting::Number(number) => Value::from(*number),
            Setting::Switch(on) => Value::from(*on),
        }
    }
}

/// Check `raw` for `key` with the loader's rules, and put it in the form
/// written to the file: theme, mode, language and pool names in lower case
/// (a language or pool by its main name: `js` is `javascript`), switches as
/// `true` / `false` (`on` / `off` are accepted too), colors as `#RRGGBB` or
/// a name like `dark_gray`. The error says what would be accepted.
pub fn check(key: &Key, raw: &str) -> Result<Setting, String> {
    let raw = raw.trim();
    let lower = raw.to_lowercase();
    let text = |text: &str| Ok(Setting::Text(text.to_string()));
    match key.kind {
        Kind::Theme => match builtin::by_name(&lower) {
            Some(_) => text(&lower),
            None => Err(format!("not a theme (try {})", builtin::names().join(", "))),
        },
        Kind::Mode => match MODE_NAMES.contains(&lower.as_str()) {
            true => text(&lower),
            false => Err(format!("not a mode (try {})", MODE_NAMES.join(", "))),
        },
        Kind::Seconds | Kind::Count => {
            let max = if key.kind == Kind::Seconds {
                MAX_SECONDS
            } else {
                MAX_COUNT
            };
            let number: i64 = raw.parse().map_err(|_| "not a whole number".to_string())?;
            check_count(number, max).map(Setting::Number)
        }
        Kind::CodeLang => match code_lang(&lower) {
            Some(lang) => text(&Mode::Code(lang).label()["code-".len()..]),
            None => Err("not a code language (try rust, python, js, go, java, sql, shell)".into()),
        },
        Kind::Pool => match word_pool(&lower) {
            Some(WordPool::Common) => text("common"),
            Some(WordPool::Extended) => text("extended"),
            None => Err("not a word pool (try common or extended)".into()),
        },
        Kind::Switch => match lower.as_str() {
            "true" | "on" => Ok(Setting::Switch(true)),
            "false" | "off" => Ok(Setting::Switch(false)),
            _ => Err("not a switch (try true or false)".into()),
        },
        // Written back the one way the parser reads it (`#A6E22E`,
        // `dark_gray`), so a printed value is always one of those.
        Kind::Choice(names) => match names.contains(&lower.as_str()) {
            true => text(&lower),
            false => Err(format!("not one of {}", names.join(", "))),
        },
        Kind::Color => match color::parse_color(raw) {
            Ok(parsed) => text(&color::color_name(parsed)),
            Err(error) => Err(error.to_string()),
        },
    }
}

/// Where a setting's value comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// Set in the file.
    File,
    /// Not set: the built-in default.
    Default,
    /// Set in the file to something the loader rejects (the file's value,
    /// made printable, and why), so the default is used.
    Invalid { found: String, problem: String },
}

/// A setting's value as TypeRush will use it, and where that comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lookup {
    pub key: &'static Key,
    pub value: Setting,
    pub source: Source,
}

/// Every setting as TypeRush will use it, for a parsed `config`. Colors
/// that aren't set take the theme's own.
pub fn effective(config: &Config) -> Vec<Lookup> {
    let theme = file_value(config, "theme")
        .and_then(|name| builtin::by_name(&name))
        .unwrap_or(builtin::DARK);
    KEYS.iter()
        .map(|key| {
            let default = default_value(key, &theme);
            let (value, source) = match file_value(config, key.name) {
                None => (default, Source::Default),
                Some(found) => match check(key, &found) {
                    Ok(value) => (value, Source::File),
                    Err(problem) => {
                        let found = printable(&found).into_owned();
                        (default, Source::Invalid { found, problem })
                    }
                },
            };
            Lookup { key, value, source }
        })
        .collect()
}

/// The setting `name` as it is in the file, as text; `None` when unset.
fn file_value(config: &Config, name: &str) -> Option<String> {
    let defaults = config.defaults.as_ref();
    let words = config.words.as_ref();
    let colors = config.colors.as_ref();
    let number = |n: Option<u64>| n.map(|n| n.to_string());
    let switch = |on: Option<bool>| on.map(|on| on.to_string());
    match name {
        "theme" => config.theme.clone(),
        "defaults.mode" => defaults?.mode.clone(),
        "defaults.time_seconds" => number(defaults?.time_seconds),
        "defaults.word_count" => number(defaults?.word_count.map(|n| n as u64)),
        "defaults.code_lang" => defaults?.code_lang.clone(),
        "defaults.symbol_count" => number(defaults?.symbol_count.map(|n| n as u64)),
        "words.pool" => words?.pool.clone(),
        "words.punctuation" => switch(words?.punctuation),
        "words.numbers" => switch(words?.numbers),
        "typing.live_wpm" => switch(config.typing.as_ref()?.live_wpm),
        "typing.blind" => switch(config.typing.as_ref()?.blind),
        "typing.stop_on_error" => config.typing.as_ref()?.stop_on_error.clone(),
        "typing.cursor" => config.typing.as_ref()?.cursor.clone(),
        "typing.sound" => config.typing.as_ref()?.sound.clone(),
        "colors.accent" => colors?.accent.clone(),
        "colors.secondary" => colors?.secondary.clone(),
        "colors.correct" => colors?.correct.clone(),
        "colors.incorrect" => colors?.incorrect.clone(),
        "colors.pending" => colors?.pending.clone(),
        "colors.mode_tag" => colors?.mode_tag.clone(),
        "colors.error" => colors?.error.clone(),
        "colors.neutral" => colors?.neutral.clone(),
        "colors.background" => colors?.background.clone(),
        _ => None,
    }
}

/// The built-in default for `key`; a color is `theme`'s own.
fn default_value(key: &Key, theme: &ThemePalette) -> Setting {
    let text = |text: &str| Setting::Text(text.to_string());
    let color = |color| Setting::Text(color::color_name(color));
    match key.name {
        "theme" => text("dark"),
        "defaults.mode" => text("time"),
        "defaults.time_seconds" => Setting::Number(DEFAULT_SECONDS as i64),
        "defaults.word_count" => Setting::Number(DEFAULT_WORDS as i64),
        "defaults.code_lang" => text("rust"),
        "defaults.symbol_count" => Setting::Number(DEFAULT_SYMBOLS as i64),
        "words.pool" => text("common"),
        "words.punctuation" | "words.numbers" => Setting::Switch(false),
        "typing.live_wpm" => Setting::Switch(true),
        "typing.blind" => Setting::Switch(false),
        "typing.stop_on_error" => text("off"),
        "typing.cursor" => text("underline"),
        "typing.sound" => text("off"),
        "colors.accent" => color(theme.accent),
        "colors.secondary" => color(theme.secondary),
        "colors.correct" => color(theme.correct),
        "colors.incorrect" => color(theme.incorrect),
        "colors.pending" => color(theme.pending),
        "colors.mode_tag" => color(theme.mode_tag),
        "colors.error" => color(theme.error),
        "colors.neutral" => color(theme.neutral),
        "colors.background" => color(theme.background),
        _ => text(""),
    }
}

/// The config at `path` as TypeRush reads it: `Ok(None)` when there is no
/// file; an error, saying why, when it can't be read or doesn't parse (the
/// app then uses every default).
pub fn read(path: &Path) -> Result<Option<Config>> {
    let shown = shown(path);
    let Some(text) = read_config(path).with_context(|| format!("can't read {shown}"))? else {
        return Ok(None);
    };
    let config = toml::from_str(&text).with_context(|| format!("can't parse {shown}"))?;
    Ok(Some(config))
}

/// Set `name` to `raw` in the config at `path` (created, with its folder,
/// if missing), keeping everything else in the file as it was — including
/// a comment after the old value. Returns the value as written.
pub fn set(path: &Path, name: &str, raw: &str) -> Result<Setting> {
    let key = key(name)?;
    let setting = check(key, raw)
        .map_err(|problem| anyhow!("can't set {name} to '{}': {problem}", printable(raw.trim())))?;
    edit(path, |doc| {
        let (table, field) = split(key.name);
        let target = match table {
            None => doc.as_table_mut() as &mut dyn toml_edit::TableLike,
            Some(table) => {
                if !doc.contains_key(table) {
                    // A new section goes at the very end. Comments after the
                    // last setting are the document's trailing text, which
                    // would print after it: move them above it instead.
                    let mut section = Table::new();
                    let trailing = doc.trailing().as_str().unwrap_or("").to_string();
                    if !trailing.trim().is_empty() {
                        section
                            .decor_mut()
                            .set_prefix(format!("{}\n\n", trailing.trim_end()));
                        doc.set_trailing("");
                    }
                    doc.insert(table, Item::Table(section));
                }
                doc[table]
                    .as_table_like_mut()
                    .ok_or_else(|| anyhow!("{table} in the file is not a section"))?
            }
        };
        let mut value = setting.value();
        // Changed in place, so the key keeps the comments above it and the
        // value the comment after it.
        match target.get_mut(field) {
            Some(Item::Value(old)) => {
                *value.decor_mut() = old.decor().clone();
                *old = value;
            }
            Some(item) => *item = Item::Value(value),
            None => {
                target.insert(field, Item::Value(value));
            }
        }
        Ok(())
    })?;
    Ok(setting)
}

/// Remove `name` from the config at `path`, so its default applies. Its
/// section stays, even if empty: comments above it may belong to it.
/// Returns whether it was set.
pub fn reset(path: &Path, name: &str) -> Result<bool> {
    let key = key(name)?;
    if read_config(path)
        .with_context(|| format!("can't read {}", shown(path)))?
        .is_none()
    {
        return Ok(false);
    }
    let mut removed = false;
    edit(path, |doc| {
        let (table, field) = split(key.name);
        let target = match table {
            None => Some(doc.as_table_mut() as &mut dyn toml_edit::TableLike),
            Some(table) => doc.get_mut(table).and_then(Item::as_table_like_mut),
        };
        removed = target.and_then(|target| target.remove(field)).is_some();
        Ok(())
    })?;
    Ok(removed)
}

/// Put every setting back to its default by moving the config at `path`
/// aside, next to it, with the time in its name — nothing is lost. Returns
/// where it went, or `None` when there was no file.
pub fn reset_all(path: &Path) -> Result<Option<PathBuf>> {
    if std::fs::symlink_metadata(path).is_err() {
        return Ok(None);
    }
    let mut backup = path.as_os_str().to_owned();
    backup.push(
        chrono::Local::now()
            .format(".bak-%Y%m%d-%H%M%S%.3f")
            .to_string(),
    );
    let backup = PathBuf::from(backup);
    std::fs::rename(path, &backup).with_context(|| format!("can't move {} aside", shown(path)))?;
    Ok(Some(backup))
}

/// The starter config: every setting at its built-in default, each one
/// explained, most of them commented out. It changes nothing until edited.
pub const STARTER: &str = include_str!("../../config.example.toml");

/// Write [`STARTER`] to `path`, creating its folder. Refuses to replace a
/// config that is already there.
pub fn init(path: &Path) -> Result<()> {
    if std::fs::symlink_metadata(path).is_ok() {
        bail!(
            "{} already exists (`typerush config reset` moves it aside first)",
            shown(path)
        );
    }
    write(path, STARTER)
}

/// Read the config at `path` (empty if missing), apply `change` to it as a
/// TOML document, and write it back.
fn edit(path: &Path, change: impl FnOnce(&mut DocumentMut) -> Result<()>) -> Result<()> {
    let shown = shown(path);
    let text = read_config(path)
        .with_context(|| format!("can't read {shown}"))?
        .unwrap_or_default();
    let mut doc: DocumentMut = text.parse().map_err(|error| {
        anyhow!("can't change {shown}: it isn't valid TOML (fix it, or `typerush config reset`)\n{error}")
    })?;
    change(&mut doc)?;
    write(path, &doc.to_string())
}

/// Write `text` to `path` atomically, creating its folder.
fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("can't create {}", shown(dir)))?;
    }
    write_atomic(path, text).with_context(|| format!("can't write {}", shown(path)))
}

/// `defaults.mode` → (`Some("defaults")`, `"mode"`); `theme` → (`None`, `"theme"`).
fn split(name: &str) -> (Option<&str>, &str) {
    match name.split_once('.') {
        Some((table, field)) => (Some(table), field),
        None => (None, name),
    }
}

/// `path` as shown in a message: made safe, like every path.
fn shown(path: &Path) -> String {
    printable(&path.display().to_string()).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load::{load_from, CliOverrides};
    use crate::config::{Colors, Defaults, Typing, Words};

    fn temp_config() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".typerush").join("config.toml");
        (dir, path)
    }

    /// `KEYS` covers every field of the config (but the unused `extra`):
    /// destructured without `..`, so a new field fails to compile here until
    /// it is added there too.
    #[test]
    fn keys_cover_every_config_field() {
        let Config {
            theme: _,
            defaults: _,
            colors: _,
            words: _,
            typing: _,
        } = Config::default();
        let Defaults {
            mode: _,
            time_seconds: _,
            word_count: _,
            code_lang: _,
            symbol_count: _,
        } = Defaults::default();
        let Words {
            pool: _,
            punctuation: _,
            numbers: _,
        } = Words::default();
        let Typing {
            live_wpm: _,
            blind: _,
            stop_on_error: _,
            cursor: _,
            sound: _,
        } = Typing::default();
        let Colors {
            accent: _,
            secondary: _,
            correct: _,
            incorrect: _,
            pending: _,
            extra: _,
            mode_tag: _,
            error: _,
            neutral: _,
            background: _,
        } = Colors::default();
        // 1 theme + 5 defaults + 3 words + 5 typing + 9 colors (10 minus
        // `extra`).
        assert_eq!(KEYS.len(), 1 + 5 + 3 + 5 + 9);
    }

    /// One valid value for each kind of setting, as typed by a person.
    fn sample(kind: Kind) -> &'static str {
        match kind {
            Kind::Theme => "Dracula",
            Kind::Mode => "words",
            Kind::Seconds => "60",
            Kind::Count => "50",
            Kind::CodeLang => "JS",
            Kind::Pool => "10k",
            Kind::Switch => "on",
            Kind::Color => "#A6E22E",
            Kind::Choice(names) => names[names.len() - 1],
        }
    }

    /// Whatever `set` accepts, the loader takes without a warning — for
    /// every setting — and `effective` reads it back from the file.
    #[test]
    fn every_setting_round_trips_through_the_loader() {
        let (_dir, path) = temp_config();
        for key in KEYS {
            let written = set(&path, key.name, sample(key.kind)).unwrap();
            let (_, warnings) = load_from(&path, CliOverrides::default());
            assert!(warnings.is_empty(), "{}: {warnings:?}", key.name);
            let config = read(&path).unwrap().unwrap();
            let lookup = effective(&config)
                .into_iter()
                .find(|lookup| lookup.key == key)
                .unwrap();
            assert_eq!((lookup.value, lookup.source), (written, Source::File));
        }
        let (resolved, _) = load_from(&path, CliOverrides::default());
        assert_eq!(resolved.default_mode, Mode::Words(50));
        assert_eq!(resolved.word_pool, WordPool::Extended);
        assert!(resolved.word_decor.punctuation && resolved.word_decor.numbers);
    }

    /// Values are written the canonical way: names lower case, a language
    /// or pool by its main name, numbers and switches unquoted.
    #[test]
    fn values_are_written_canonically() {
        let check = |name: &str, raw: &str| check(key(name).unwrap(), raw).map(|s| s.toml());
        assert_eq!(check("theme", " MonoKai "), Ok("\"monokai\"".into()));
        assert_eq!(
            check("defaults.code_lang", "js"),
            Ok("\"javascript\"".into())
        );
        assert_eq!(check("defaults.code_lang", "bash"), Ok("\"shell\"".into()));
        assert_eq!(check("words.pool", "BIG"), Ok("\"extended\"".into()));
        assert_eq!(check("defaults.time_seconds", "30"), Ok("30".into()));
        assert_eq!(check("words.numbers", "OFF"), Ok("false".into()));
        assert_eq!(
            check("colors.accent", "dark_gray"),
            Ok("\"dark_gray\"".into())
        );
    }

    /// Bad values are refused with the reason, and the file isn't touched;
    /// so is an unknown setting.
    #[test]
    fn bad_values_are_refused() {
        let (_dir, path) = temp_config();
        for (name, raw, reason) in [
            ("theme", "solarized", "not a theme"),
            ("defaults.mode", "race", "not a mode"),
            ("defaults.time_seconds", "0", "between 1 and 3600"),
            ("defaults.word_count", "10001", "between 1 and 10000"),
            ("defaults.symbol_count", "lots", "not a whole number"),
            ("defaults.code_lang", "cobol", "not a code language"),
            ("words.pool", "huge", "not a word pool"),
            ("words.punctuation", "maybe", "not a switch"),
            ("colors.accent", "#aé€", "hex color"),
        ] {
            let error = set(&path, name, raw).unwrap_err().to_string();
            assert!(error.contains(reason), "{name}: {error}");
        }
        assert!(!path.exists(), "a refused value wrote the file");
        let error = set(&path, "colors.extra", "red").unwrap_err().to_string();
        assert!(error.contains("unknown setting"), "{error}");
    }

    /// `set` changes only its value: comments (on their own line and after
    /// the old value), other settings and their order stay as they were.
    #[test]
    fn set_keeps_comments_and_layout() {
        let (_dir, path) = temp_config();
        let original = "# my settings\ntheme = \"light\" # easy on the eyes\n\n[defaults]\n# how I start\nmode = \"words\"\nword_count = 50\n";
        write(&path, original).unwrap();
        set(&path, "theme", "dracula").unwrap();
        set(&path, "defaults.time_seconds", "30").unwrap();
        set(&path, "words.numbers", "true").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "# my settings\ntheme = \"dracula\" # easy on the eyes\n\n[defaults]\n# how I start\nmode = \"words\"\nword_count = 50\ntime_seconds = 30\n\n[words]\nnumbers = true\n"
        );
    }

    /// A new section goes after everything already in the file, comments at
    /// its end included (the starter config ends in a commented example).
    #[test]
    fn new_sections_go_after_trailing_comments() {
        let (_dir, path) = temp_config();
        write(
            &path,
            "theme = \"dark\"\n\n# [colors]\n# accent = \"red\"\n",
        )
        .unwrap();
        set(&path, "colors.accent", "blue").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "theme = \"dark\"\n\n# [colors]\n# accent = \"red\"\n\n[colors]\naccent = \"blue\"\n"
        );
    }

    /// `reset NAME` removes just that setting (its section stays); `reset`
    /// of everything moves the file aside, so nothing is lost.
    #[test]
    fn reset_one_or_all() {
        let (_dir, path) = temp_config();
        assert!(!reset(&path, "theme").unwrap(), "nothing to reset");
        assert_eq!(reset_all(&path).unwrap(), None);
        set(&path, "theme", "monokai").unwrap();
        set(&path, "colors.accent", "red").unwrap();
        assert!(reset(&path, "colors.accent").unwrap());
        assert!(!reset(&path, "colors.accent").unwrap());
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, "theme = \"monokai\"\n\n[colors]\n");

        let backup = reset_all(&path).unwrap().unwrap();
        assert!(!path.exists());
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), text);
        let name = backup.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("config.toml.bak-"), "{name}");
    }

    /// A file that isn't valid TOML is never overwritten by `set`: the error
    /// says how to get out of it.
    #[test]
    fn broken_file_is_not_overwritten() {
        let (_dir, path) = temp_config();
        write(&path, "theme = = \"dark\"").unwrap();
        let error = format!("{:#}", set(&path, "theme", "light").unwrap_err());
        assert!(error.contains("isn't valid TOML"), "{error}");
        assert!(error.contains("config reset"), "{error}");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "theme = = \"dark\""
        );
    }

    /// `effective` gives the defaults for an empty config (colors from the
    /// theme), the file's values where valid, and the default with the
    /// reason where not.
    #[test]
    fn effective_values_and_sources() {
        let lookups = effective(&Config::default());
        assert!(lookups.iter().all(|l| l.source == Source::Default));
        let accent = |lookups: &[Lookup]| {
            lookups
                .iter()
                .find(|l| l.key.name == "colors.accent")
                .unwrap()
                .value
                .clone()
        };
        assert_eq!(accent(&lookups), Setting::Text("cyan".into()));

        let config: Config =
            toml::from_str("theme = \"monokai\"\n[defaults]\ntime_seconds = 99999\n").unwrap();
        let lookups = effective(&config);
        assert_eq!(accent(&lookups), Setting::Text("#66D9EF".into()));
        let seconds = lookups
            .iter()
            .find(|l| l.key.name == "defaults.time_seconds")
            .unwrap();
        assert_eq!(seconds.value, Setting::Number(15));
        assert!(
            matches!(&seconds.source, Source::Invalid { found, problem }
                if found == "99999" && problem.contains("between 1 and 3600")),
            "{seconds:?}"
        );
    }

    /// The starter config changes nothing: it loads without a warning to
    /// exactly the built-in defaults. `init` writes it, and refuses to
    /// replace a config that is already there.
    #[test]
    fn starter_config_is_the_defaults() {
        let (_dir, path) = temp_config();
        init(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), STARTER);
        let (resolved, warnings) = load_from(&path, CliOverrides::default());
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(resolved, crate::config::load::ResolvedConfig::default());
        let error = init(&path).unwrap_err().to_string();
        assert!(error.contains("already exists"), "{error}");
    }
}
