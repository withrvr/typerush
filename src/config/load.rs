//! Disk I/O for the config file plus the "resolve everything" entry point.
//!
//! `load_or_default_with(CliOverrides)` is the single function the rest of the
//! app calls (`load_from` is the same for any path, for tests). It returns a fully-resolved `ResolvedConfig` and a list of
//! human-readable warnings — never an error. The caller (typically `main.rs`)
//! can choose to surface the first warning via the error modal.

use std::path::{Path, PathBuf};

use super::{Colors, Config};
use crate::app::Mode;
use crate::options::{CursorStyle, KeySound, Pace, StopOnError, TypingOptions};
use crate::storage;
use crate::theme::{self, builtin, ThemePalette};
use crate::words::{CodeLang, WordDecor, WordPool};

/// Final, fully-resolved configuration the rest of the app consumes.
///
/// All `Option`s in the raw `Config` have been flattened: missing values were
/// replaced with hardcoded defaults, missing/invalid theme names fell back to
/// `dark`, and color slot overrides were applied on top of the chosen palette.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedConfig {
    /// Final palette to render with.
    pub palette: ThemePalette,
    /// Pre-selected mode for the menu (and starting `App::mode`).
    pub default_mode: Mode,
    /// Which English word pool to use in Time / Words / Zen modes. Default
    /// is the legacy `Common` pool so a missing config behaves like v0.3.
    pub word_pool: WordPool,
    /// Punctuation / numbers decoration toggles (v0.4.0).
    pub word_decor: WordDecor,
    /// The `[typing]` settings (v0.7.0).
    pub typing: TypingOptions,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            palette: ThemePalette::default(),
            // Picked in agreement with the v0.2.0 spec: "default scheme time 15 sec".
            default_mode: Mode::Time(DEFAULT_SECONDS),
            word_pool: WordPool::Common,
            word_decor: WordDecor::default(),
            typing: TypingOptions::default(),
        }
    }
}

/// Filesystem path of the config file: `~/.typerush/config.toml`.
pub fn config_path() -> PathBuf {
    storage::data_dir().join("config.toml")
}

/// CLI settings that take precedence over the config file. `None` falls
/// through to the config value (or its built-in default); `Some` wins either
/// way, so `--no-punctuation` turns off what the config turned on.
#[derive(Debug, Default, Clone)]
pub struct CliOverrides<'a> {
    /// `--theme <name>`.
    pub theme: Option<&'a str>,
    /// `--big` → `Some(Extended)`, `--no-big` → `Some(Common)`.
    pub word_pool: Option<WordPool>,
    /// `--punctuation` → `Some(true)`, `--no-punctuation` → `Some(false)`.
    pub punctuation: Option<bool>,
    /// `--numbers` → `Some(true)`, `--no-numbers` → `Some(false)`.
    pub numbers: Option<bool>,
    /// `--blind` → `Some(true)`.
    pub blind: Option<bool>,
    /// `--stop-on-error <off|letter|word>`.
    pub stop_on_error: Option<StopOnError>,
    /// `--pace <best|WPM>`.
    pub pace: Option<Pace>,
}

/// Read and resolve `~/.typerush/config.toml`, then apply CLI overrides on
/// top. See [`load_from`].
pub fn load_or_default_with(cli: CliOverrides<'_>) -> (ResolvedConfig, Vec<String>) {
    load_from(&config_path(), cli)
}

/// Read and resolve the config at `path`, then apply CLI overrides on top.
///
/// Always succeeds, and the CLI overrides always apply:
///   - missing file        → defaults, no warnings
///   - unreadable or unparseable file → defaults, one warning
///   - unknown theme name  → fall back to dark, one warning
///   - bad color override  → keep the theme's slot, one warning per bad slot
pub fn load_from(path: &Path, cli: CliOverrides<'_>) -> (ResolvedConfig, Vec<String>) {
    // The path is shown in the error modal: made safe like every other path.
    let shown = || crate::text::printable(&path.display().to_string()).into_owned();
    let (raw_config, file_warning) = match read_config(path) {
        Ok(Some(text)) => match toml::from_str::<Config>(&text) {
            Ok(parsed) => (parsed, None),
            Err(err) => (
                Config::default(),
                Some(format!("could not parse {}: {}", shown(), err)),
            ),
        },
        Ok(None) => (Config::default(), None),
        Err(err) => (
            Config::default(),
            Some(format!("could not read {}: {}", shown(), err)),
        ),
    };
    let (resolved, mut warnings) = resolve_with(raw_config, cli);
    // The file problem comes first: it explains everything after it.
    if let Some(warning) = file_warning {
        warnings.insert(0, warning);
    }
    (resolved, warnings)
}

/// Largest config file read: a real one is a few hundred bytes.
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// The text of the config file at `path`, `None` if there is none. Like a
/// custom file, only a regular file up to 1 MiB is read — a pipe or device
/// put there would otherwise block or fill memory before the menu opens.
pub fn read_config(path: &Path) -> std::io::Result<Option<String>> {
    use std::io::Read;
    let not_a_file = || std::io::Error::other("not a regular file");
    // Checked before opening: opening a named pipe blocks until something
    // writes to it.
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    if !metadata.is_file() {
        return Err(not_a_file());
    }
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err(std::io::Error::other("too big (over 1 MiB)"));
    }
    // Checked again on the opened file, and the read bounded, in case the
    // path changed in between.
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(not_a_file());
    }
    let mut text = String::new();
    file.take(MAX_CONFIG_BYTES).read_to_string(&mut text)?;
    Ok(Some(text))
}

/// Full pure resolver. Applies CLI overrides after the file-derived defaults.
fn resolve_with(raw: Config, cli: CliOverrides<'_>) -> (ResolvedConfig, Vec<String>) {
    let mut warnings = vec![];

    // Theme: CLI wins over config. Unknown name → warn, use dark.
    let theme_choice = cli.theme.map(str::to_string).or_else(|| raw.theme.clone());
    let palette_base = match theme_choice.as_deref() {
        None => builtin::DARK,
        Some(name) => match builtin::by_name(name) {
            Some(palette) => palette,
            None => {
                warnings.push(format!(
                    "unknown theme '{}', falling back to 'dark' (try one of: {})",
                    name,
                    builtin::names().join(", ")
                ));
                builtin::DARK
            }
        },
    };

    // Color slot overrides on top of the chosen palette.
    let palette = if let Some(colors) = raw.colors.as_ref() {
        apply_color_overrides(palette_base, colors, &mut warnings)
    } else {
        palette_base
    };

    let default_mode = resolve_default_mode(raw.defaults.as_ref(), &mut warnings);
    let (mut word_pool, mut word_decor) = resolve_words_section(raw.words.as_ref(), &mut warnings);

    // CLI overrides win.
    if let Some(p) = cli.word_pool {
        word_pool = p;
    }
    if let Some(p) = cli.punctuation {
        word_decor.punctuation = p;
    }
    if let Some(n) = cli.numbers {
        word_decor.numbers = n;
    }
    let mut typing = resolve_typing(raw.typing.as_ref(), &mut warnings);
    if let Some(blind) = cli.blind {
        typing.blind = blind;
    }
    if let Some(stop) = cli.stop_on_error {
        typing.stop_on_error = stop;
    }
    if let Some(pace) = cli.pace {
        typing.pace = pace;
    }

    (
        ResolvedConfig {
            palette,
            default_mode,
            word_pool,
            word_decor,
            typing,
        },
        warnings,
    )
}

/// The `[typing]` section, each unset value its default; an unknown name
/// is a warning and the default.
fn resolve_typing(typing: Option<&super::Typing>, warnings: &mut Vec<String>) -> TypingOptions {
    let defaults = TypingOptions::default();
    let Some(typing) = typing else {
        return defaults;
    };
    let stop_on_error = named(
        "stop_on_error",
        typing.stop_on_error.as_deref(),
        &StopOnError::NAMES,
        StopOnError::from_name,
        defaults.stop_on_error,
        warnings,
    );
    let cursor = named(
        "cursor",
        typing.cursor.as_deref(),
        &CursorStyle::NAMES,
        CursorStyle::from_name,
        defaults.cursor,
        warnings,
    );
    let sound = named(
        "sound",
        typing.sound.as_deref(),
        &KeySound::NAMES,
        KeySound::from_name,
        defaults.sound,
        warnings,
    );
    let pace = match &typing.pace {
        None => defaults.pace,
        Some(value) => Pace::from_name(&value.text()).unwrap_or_else(|| {
            warnings.push(format!(
                "pace '{}' isn't off, best or a WPM from 1 to {}, using 'off'",
                crate::text::printable(&value.text()),
                Pace::MAX_WPM
            ));
            defaults.pace
        }),
    };
    TypingOptions {
        pace,
        live_wpm: typing.live_wpm.unwrap_or(defaults.live_wpm),
        blind: typing.blind.unwrap_or(defaults.blind),
        stop_on_error,
        cursor,
        sound,
    }
}

/// A setting that takes one of `names`: the default when unset, a warning
/// and the default when it isn't one of them.
fn named<T: Copy>(
    setting: &str,
    value: Option<&str>,
    names: &[&str],
    from_name: fn(&str) -> Option<T>,
    default: T,
    warnings: &mut Vec<String>,
) -> T {
    let Some(value) = value else {
        return default;
    };
    from_name(value).unwrap_or_else(|| {
        warnings.push(format!(
            "unknown {setting} '{}', using '{}' (try {})",
            crate::text::printable(value),
            names[0],
            names.join(", ")
        ));
        default
    })
}

/// Translate the optional `[words]` section into a (pool, decor) pair.
fn resolve_words_section(
    words: Option<&super::Words>,
    warnings: &mut Vec<String>,
) -> (WordPool, WordDecor) {
    let Some(words) = words else {
        return (WordPool::Common, WordDecor::default());
    };
    let pool = match words.pool.as_deref() {
        None => WordPool::Common,
        Some(name) => word_pool(name).unwrap_or_else(|| {
            warnings.push(format!(
                "unknown word pool '{}', falling back to 'common'",
                name.to_lowercase()
            ));
            WordPool::Common
        }),
    };
    let decor = WordDecor {
        punctuation: words.punctuation.unwrap_or(false),
        numbers: words.numbers.unwrap_or(false),
    };
    (pool, decor)
}

fn apply_color_overrides(
    mut palette: ThemePalette,
    overrides: &Colors,
    warnings: &mut Vec<String>,
) -> ThemePalette {
    // `extra` is accepted (old configs set it) and still checked, but no
    // longer drawn.
    let mut extra = palette.background;
    let slots = [
        ("extra", &overrides.extra, &mut extra),
        ("accent", &overrides.accent, &mut palette.accent),
        ("secondary", &overrides.secondary, &mut palette.secondary),
        ("correct", &overrides.correct, &mut palette.correct),
        ("incorrect", &overrides.incorrect, &mut palette.incorrect),
        ("pending", &overrides.pending, &mut palette.pending),
        ("mode_tag", &overrides.mode_tag, &mut palette.mode_tag),
        ("error", &overrides.error, &mut palette.error),
        ("neutral", &overrides.neutral, &mut palette.neutral),
        ("background", &overrides.background, &mut palette.background),
    ];
    for (slot, value, color) in slots {
        if let Some(input) = value {
            match theme::color::parse_color(input) {
                Ok(parsed) => *color = parsed,
                Err(err) => warnings.push(format!("bad color for '{}': {}", slot, err)),
            }
        }
    }
    palette
}

/// The `[words] pool` names (case-insensitive). `None` for anything else.
pub fn word_pool(name: &str) -> Option<WordPool> {
    match name.to_lowercase().as_str() {
        "common" | "small" => Some(WordPool::Common),
        "extended" | "big" | "10000" | "10k" => Some(WordPool::Extended),
        _ => None,
    }
}

/// The `[defaults] mode` names, in menu order.
pub const MODE_NAMES: [&str; 7] = ["time", "words", "quote", "code", "zen", "symbols", "daily"];

/// Built-in defaults for the `[defaults]` counts: what a time, words or
/// symbols run starts with when the config doesn't say.
pub const DEFAULT_SECONDS: u64 = 15;
pub const DEFAULT_WORDS: usize = 25;
pub const DEFAULT_SYMBOLS: usize = 25;

/// Longest time run, in seconds: an hour.
pub const MAX_SECONDS: u16 = 3600;
/// Most words or symbol tokens in one run.
pub const MAX_COUNT: u16 = 10_000;

/// `count` if it is from 1 to `max` — the one rule for run lengths, used by
/// the command line and the config alike. A zero count would start a session
/// with nothing to type; a huge one would build billions of words.
pub fn check_count<T: PartialOrd + From<u16>>(count: T, max: u16) -> Result<T, String> {
    if count >= T::from(1) && count <= T::from(max) {
        Ok(count)
    } else {
        Err(format!("must be between 1 and {max}"))
    }
}

/// A config count: `default` when unset, or with a warning when out of range.
fn config_count<T>(
    name: &str,
    value: Option<T>,
    max: u16,
    default: T,
    warnings: &mut Vec<String>,
) -> T
where
    T: PartialOrd + From<u16> + std::fmt::Display + Copy,
{
    let Some(value) = value else {
        return default;
    };
    check_count(value, max).unwrap_or_else(|problem| {
        warnings.push(format!("{name} {problem}, using {default}"));
        default
    })
}

fn resolve_default_mode(defaults: Option<&super::Defaults>, warnings: &mut Vec<String>) -> Mode {
    let Some(defaults) = defaults else {
        return Mode::Time(DEFAULT_SECONDS);
    };
    let mode_name = defaults.mode.as_deref().unwrap_or("time");
    match mode_name.to_lowercase().as_str() {
        "time" => Mode::Time(config_count(
            "time_seconds",
            defaults.time_seconds,
            MAX_SECONDS,
            DEFAULT_SECONDS,
            warnings,
        )),
        "words" => Mode::Words(config_count(
            "word_count",
            defaults.word_count,
            MAX_COUNT,
            DEFAULT_WORDS,
            warnings,
        )),
        "quote" => Mode::Quote,
        "code" => {
            let name = defaults.code_lang.as_deref().unwrap_or("rust");
            Mode::Code(code_lang(name).unwrap_or_else(|| {
                warnings.push(format!(
                    "unknown code_lang '{}', falling back to 'rust'",
                    name.to_lowercase()
                ));
                CodeLang::Rust
            }))
        }
        "zen" => Mode::Zen,
        "daily" => Mode::Daily,
        "symbols" => Mode::Symbols(config_count(
            "symbol_count",
            defaults.symbol_count,
            MAX_COUNT,
            DEFAULT_SYMBOLS,
            warnings,
        )),
        other => {
            warnings.push(format!(
                "unknown default mode '{}', falling back to 'time'",
                other
            ));
            Mode::Time(config_count(
                "time_seconds",
                defaults.time_seconds,
                MAX_SECONDS,
                DEFAULT_SECONDS,
                warnings,
            ))
        }
    }
}

/// The code-language names accepted by both `code_lang` in the config and
/// `--code` on the command line (case-insensitive). `None` for anything else;
/// each caller decides what an unknown name means (a warning vs an error).
pub fn code_lang(name: &str) -> Option<CodeLang> {
    Some(match name.to_lowercase().as_str() {
        "rust" | "rs" => CodeLang::Rust,
        "python" | "py" => CodeLang::Python,
        "js" | "javascript" => CodeLang::JavaScript,
        "go" | "golang" => CodeLang::Go,
        "java" => CodeLang::Java,
        "sql" => CodeLang::Sql,
        "shell" | "sh" | "bash" => CodeLang::Shell,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Colors, Defaults};
    use ratatui::style::Color;

    /// Resolve `raw` with only a `--theme` override (most tests need no more).
    fn resolve(raw: Config, theme: Option<&str>) -> (ResolvedConfig, Vec<String>) {
        resolve_with(
            raw,
            CliOverrides {
                theme,
                ..Default::default()
            },
        )
    }

    /// The config is read like a custom file: only a regular file of at
    /// most 1 MiB. A folder, a pipe or a device where config.toml should be
    /// is a warning, not a hang; so is a huge file.
    #[test]
    fn config_must_be_a_small_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("config.toml");
        std::fs::create_dir(&folder).unwrap();
        let (_, warnings) = load_from(&folder, CliOverrides::default());
        assert!(warnings[0].contains("not a regular file"), "{warnings:?}");

        let huge = dir.path().join("huge.toml");
        let file = std::fs::File::create(&huge).unwrap();
        file.set_len(MAX_CONFIG_BYTES + 1).unwrap();
        let (resolved, warnings) = load_from(&huge, CliOverrides::default());
        assert!(warnings[0].contains("too big"), "{warnings:?}");
        assert_eq!(resolved, ResolvedConfig::default());

        // A named pipe would block `open` forever: it is refused unopened.
        #[cfg(unix)]
        {
            let fifo = dir.path().join("fifo.toml");
            let made = std::process::Command::new("mkfifo").arg(&fifo).status();
            if made.is_ok_and(|status| status.success()) {
                let (_, warnings) = load_from(&fifo, CliOverrides::default());
                assert!(warnings[0].contains("not a regular file"), "{warnings:?}");
            }
        }
    }

    /// Every mode name the config documents resolves without a warning.
    #[test]
    fn every_mode_name_resolves() {
        for name in MODE_NAMES {
            let raw = Config {
                defaults: Some(Defaults {
                    mode: Some(name.into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let (_, warnings) = resolve(raw, None);
            assert!(warnings.is_empty(), "{name}: {warnings:?}");
        }
    }

    /// `stop_on_error` takes off / letter / word; anything else warns and
    /// is off.
    #[test]
    fn stop_on_error_from_the_config() {
        let typing = |name: &str| Config {
            typing: Some(crate::config::Typing {
                stop_on_error: Some(name.into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, warnings) = resolve(typing("Letter"), None);
        assert!(warnings.is_empty());
        assert_eq!(resolved.typing.stop_on_error, StopOnError::Letter);
        let (resolved, warnings) = resolve(typing("strict"), None);
        assert_eq!(resolved.typing.stop_on_error, StopOnError::Off);
        assert!(
            warnings[0].contains("unknown stop_on_error 'strict'"),
            "{warnings:?}"
        );
    }

    /// `pace` is written as a number or as text; out of range warns.
    #[test]
    fn pace_from_the_config() {
        for (text, pace) in [
            ("pace = 60", Pace::Wpm(60)),
            ("pace = \"best\"", Pace::Best),
        ] {
            let raw: Config = toml::from_str(&format!("[typing]\n{text}")).unwrap();
            let (resolved, warnings) = resolve(raw, None);
            assert!(warnings.is_empty(), "{warnings:?}");
            assert_eq!(resolved.typing.pace, pace);
        }
        let raw: Config = toml::from_str("[typing]\npace = 9000").unwrap();
        let (resolved, warnings) = resolve(raw, None);
        assert_eq!(resolved.typing.pace, Pace::Off);
        assert!(warnings[0].contains("pace '9000'"), "{warnings:?}");
    }

    #[test]
    fn daily_default_mode() {
        let raw = Config {
            defaults: Some(Defaults {
                mode: Some("daily".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, warnings) = resolve(raw, None);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(resolved.default_mode, Mode::Daily);
    }

    #[test]
    fn default_when_config_is_empty() {
        let (resolved, warnings) = resolve(Config::default(), None);
        assert!(warnings.is_empty());
        assert_eq!(resolved.palette, builtin::DARK);
        assert_eq!(resolved.default_mode, Mode::Time(15));
    }

    #[test]
    fn theme_name_picks_palette() {
        let raw = Config {
            theme: Some("monokai".into()),
            ..Default::default()
        };
        let (resolved, warnings) = resolve(raw, None);
        assert!(warnings.is_empty());
        assert_eq!(resolved.palette, builtin::MONOKAI);
    }

    #[test]
    fn unknown_theme_warns_and_uses_dark() {
        let raw = Config {
            theme: Some("nonexistent".into()),
            ..Default::default()
        };
        let (resolved, warnings) = resolve(raw, None);
        assert_eq!(resolved.palette, builtin::DARK);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("nonexistent"));
    }

    #[test]
    fn cli_override_wins_over_config() {
        let raw = Config {
            theme: Some("monokai".into()),
            ..Default::default()
        };
        let (resolved, _) = resolve(raw, Some("dracula"));
        assert_eq!(resolved.palette, builtin::DRACULA);
    }

    #[test]
    fn color_override_applies_on_top_of_theme() {
        let raw = Config {
            theme: Some("dark".into()),
            colors: Some(Colors {
                accent: Some("#FF00FF".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, warnings) = resolve(raw, None);
        assert!(warnings.is_empty());
        assert_eq!(resolved.palette.accent, Color::Rgb(255, 0, 255));
        assert_eq!(resolved.palette.correct, builtin::DARK.correct);
    }

    #[test]
    fn bad_color_warns_and_keeps_theme_slot() {
        let raw = Config {
            colors: Some(Colors {
                accent: Some("not-a-color".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, warnings) = resolve(raw, None);
        assert_eq!(resolved.palette.accent, builtin::DARK.accent);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("accent"));
    }

    #[test]
    fn defaults_resolve_to_mode() {
        let raw = Config {
            defaults: Some(Defaults {
                mode: Some("words".into()),
                word_count: Some(100),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, _) = resolve(raw, None);
        assert_eq!(resolved.default_mode, Mode::Words(100));
    }

    #[test]
    fn defaults_unknown_mode_falls_back_with_warning() {
        let raw = Config {
            defaults: Some(Defaults {
                mode: Some("hyperspeed".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, warnings) = resolve(raw, None);
        assert_eq!(resolved.default_mode, Mode::Time(15));
        assert!(!warnings.is_empty());
    }

    #[test]
    fn multiple_bad_colors_accumulate_warnings() {
        let raw = Config {
            colors: Some(Colors {
                accent: Some("not-a-color".into()),
                correct: Some("#GG0000".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (_, warnings) = resolve(raw, None);
        assert_eq!(warnings.len(), 2);
        assert!(warnings.iter().any(|w| w.contains("accent")));
        assert!(warnings.iter().any(|w| w.contains("correct")));
    }

    #[test]
    fn defaults_code_mode_picks_language() {
        let raw = Config {
            defaults: Some(Defaults {
                mode: Some("code".into()),
                code_lang: Some("python".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, _) = resolve(raw, None);
        assert_eq!(resolved.default_mode, Mode::Code(CodeLang::Python));
    }

    // ── v0.4.0 additions ────────────────────────────────────────────────────

    #[test]
    fn new_code_langs_resolve_correctly() {
        use crate::config::Defaults;
        for (input, expected) in [
            ("go", CodeLang::Go),
            ("golang", CodeLang::Go),
            ("java", CodeLang::Java),
            ("sql", CodeLang::Sql),
            ("shell", CodeLang::Shell),
            ("bash", CodeLang::Shell),
            ("sh", CodeLang::Shell),
        ] {
            let raw = Config {
                defaults: Some(Defaults {
                    mode: Some("code".into()),
                    code_lang: Some(input.into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let (resolved, warnings) = resolve(raw, None);
            assert!(
                warnings.is_empty(),
                "warnings for {}: {:?}",
                input,
                warnings
            );
            assert_eq!(resolved.default_mode, Mode::Code(expected));
        }
    }

    #[test]
    fn symbols_mode_resolves() {
        use crate::config::Defaults;
        let raw = Config {
            defaults: Some(Defaults {
                mode: Some("symbols".into()),
                symbol_count: Some(40),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, warnings) = resolve(raw, None);
        assert!(warnings.is_empty());
        assert_eq!(resolved.default_mode, Mode::Symbols(40));
    }

    /// Config counts follow the same 1..=max rule as the command line: out
    /// of range warns and uses the default.
    #[test]
    fn out_of_range_config_counts_warn_and_use_defaults() {
        use crate::config::Defaults;
        for (mode, defaults, expected) in [
            (
                "words",
                Defaults {
                    word_count: Some(0),
                    ..Default::default()
                },
                Mode::Words(25),
            ),
            (
                "time",
                Defaults {
                    time_seconds: Some(99_999_999),
                    ..Default::default()
                },
                Mode::Time(15),
            ),
            (
                "symbols",
                Defaults {
                    symbol_count: Some(10_001),
                    ..Default::default()
                },
                Mode::Symbols(25),
            ),
        ] {
            let raw = Config {
                defaults: Some(Defaults {
                    mode: Some(mode.into()),
                    ..defaults
                }),
                ..Default::default()
            };
            let (resolved, warnings) = resolve(raw, None);
            assert_eq!(resolved.default_mode, expected);
            assert_eq!(warnings.len(), 1, "{warnings:?}");
            assert!(
                warnings[0].contains("must be between 1 and"),
                "{warnings:?}"
            );
        }
    }

    /// `extra` isn't drawn any more, but a bad value is still reported.
    #[test]
    fn bad_extra_color_still_warns() {
        let raw = Config {
            colors: Some(Colors {
                extra: Some("#ZZZ".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, warnings) = resolve(raw, None);
        assert_eq!(resolved.palette, builtin::DARK);
        assert!(
            warnings.iter().any(|w| w.contains("'extra'")),
            "{warnings:?}"
        );
    }

    /// Zero tokens would be a session with nothing to type (the CLI rejects
    /// `--symbols 0` for the same reason): warn and use the default.
    #[test]
    fn symbols_count_zero_warns_and_uses_default() {
        use crate::config::Defaults;
        let raw = Config {
            defaults: Some(Defaults {
                mode: Some("symbols".into()),
                symbol_count: Some(0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, warnings) = resolve(raw, None);
        assert_eq!(resolved.default_mode, Mode::Symbols(25));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("symbol_count"));
    }

    #[test]
    fn code_lang_is_case_insensitive_and_rejects_unknown() {
        assert_eq!(code_lang("GoLang"), Some(CodeLang::Go));
        assert_eq!(code_lang("BASH"), Some(CodeLang::Shell));
        assert_eq!(code_lang("cobol"), None);
        assert_eq!(code_lang(""), None);
    }

    #[test]
    fn symbols_mode_default_count_is_25() {
        use crate::config::Defaults;
        let raw = Config {
            defaults: Some(Defaults {
                mode: Some("symbols".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, _) = resolve(raw, None);
        assert_eq!(resolved.default_mode, Mode::Symbols(25));
    }

    #[test]
    fn words_section_default_is_common_pool_no_decor() {
        let raw = Config::default();
        let (resolved, warnings) = resolve(raw, None);
        assert!(warnings.is_empty());
        assert_eq!(resolved.word_pool, WordPool::Common);
        assert_eq!(resolved.word_decor, WordDecor::default());
    }

    #[test]
    fn words_section_extended_pool() {
        use crate::config::Words;
        let raw = Config {
            words: Some(Words {
                pool: Some("extended".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, warnings) = resolve(raw, None);
        assert!(warnings.is_empty());
        assert_eq!(resolved.word_pool, WordPool::Extended);
    }

    #[test]
    fn words_section_unknown_pool_warns() {
        use crate::config::Words;
        let raw = Config {
            words: Some(Words {
                pool: Some("hyper".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, warnings) = resolve(raw, None);
        assert_eq!(resolved.word_pool, WordPool::Common);
        assert!(!warnings.is_empty());
        assert!(warnings[0].contains("hyper"));
    }

    #[test]
    fn words_section_decor_toggles_apply() {
        use crate::config::Words;
        let raw = Config {
            words: Some(Words {
                punctuation: Some(true),
                numbers: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (resolved, _) = resolve(raw, None);
        assert!(resolved.word_decor.punctuation);
        assert!(resolved.word_decor.numbers);
    }

    #[test]
    fn cli_overrides_beat_config_pool_and_decor() {
        use crate::config::Words;
        let raw = Config {
            words: Some(Words {
                pool: Some("common".into()),
                punctuation: Some(false),
                numbers: Some(false),
            }),
            ..Default::default()
        };
        let (resolved, _) = resolve_with(
            raw,
            CliOverrides {
                word_pool: Some(WordPool::Extended),
                punctuation: Some(true),
                numbers: Some(true),
                ..Default::default()
            },
        );
        assert_eq!(resolved.word_pool, WordPool::Extended);
        assert!(resolved.word_decor.punctuation);
        assert!(resolved.word_decor.numbers);
    }

    #[test]
    fn pool_aliases_all_resolve_to_extended() {
        use crate::config::Words;
        for alias in ["extended", "big", "10000", "10k", "EXTENDED"] {
            let raw = Config {
                words: Some(Words {
                    pool: Some(alias.into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let (resolved, warnings) = resolve(raw, None);
            assert!(
                warnings.is_empty(),
                "warning for alias {}: {:?}",
                alias,
                warnings
            );
            assert_eq!(
                resolved.word_pool,
                WordPool::Extended,
                "alias {} did not resolve to Extended",
                alias
            );
        }
    }

    /// A config.toml with a typo must not swallow the command line: the
    /// warning is shown first, and `--big --punctuation --theme` still apply.
    #[test]
    fn broken_config_still_applies_cli_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "theme = \"light\"\n[words\npool = 1").unwrap();
        let (resolved, warnings) = load_from(
            &path,
            CliOverrides {
                theme: Some("monokai"),
                word_pool: Some(WordPool::Extended),
                punctuation: Some(true),
                numbers: Some(true),
                blind: Some(true),
                stop_on_error: Some(StopOnError::Word),
                pace: Some(Pace::Best),
            },
        );
        assert!(
            resolved.typing.blind,
            "--blind applies on a broken file too"
        );
        assert_eq!(resolved.typing.stop_on_error, StopOnError::Word);
        assert_eq!(resolved.typing.pace, Pace::Best);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].starts_with("could not parse"), "{warnings:?}");
        assert_eq!(resolved.palette, builtin::MONOKAI);
        assert_eq!(resolved.word_pool, WordPool::Extended);
        assert!(resolved.word_decor.punctuation && resolved.word_decor.numbers);
    }

    /// Same for a config that can't be read at all (here: a directory).
    /// The file warning comes before the warnings it causes.
    #[test]
    fn unreadable_config_still_applies_cli_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let (resolved, warnings) = load_from(
            dir.path(),
            CliOverrides {
                theme: Some("nope"),
                numbers: Some(true),
                ..Default::default()
            },
        );
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].starts_with("could not read"), "{warnings:?}");
        assert!(warnings[1].contains("nope"), "{warnings:?}");
        assert!(resolved.word_decor.numbers);
    }

    #[test]
    fn missing_config_is_silent_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let (resolved, warnings) =
            load_from(&dir.path().join("config.toml"), CliOverrides::default());
        assert!(warnings.is_empty());
        assert_eq!(resolved, ResolvedConfig::default());
    }

    /// `--no-big`, `--no-punctuation`, `--no-numbers` turn off what the
    /// config turned on, so a plain run never needs a config edit.
    #[test]
    fn cli_off_switches_beat_config() {
        use crate::config::Words;
        let raw = Config {
            words: Some(Words {
                pool: Some("extended".into()),
                punctuation: Some(true),
                numbers: Some(true),
            }),
            ..Default::default()
        };
        let (resolved, _) = resolve_with(
            raw,
            CliOverrides {
                word_pool: Some(WordPool::Common),
                punctuation: Some(false),
                numbers: Some(false),
                ..Default::default()
            },
        );
        assert_eq!(resolved.word_pool, WordPool::Common);
        assert_eq!(resolved.word_decor, WordDecor::default());
    }
}
