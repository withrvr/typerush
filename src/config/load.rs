//! Disk I/O for the config file plus the "resolve everything" entry point.
//!
//! `load_or_default()` is the single function the rest of the app calls. It
//! returns a fully-resolved `ResolvedConfig` and a list of human-readable
//! warnings — never an error. The caller (typically `main.rs`) can choose to
//! surface the first warning via the error modal.

use std::path::PathBuf;

use ratatui::style::Color;

use super::{Colors, Config};
use crate::storage;
use crate::theme::{self, builtin, ThemePalette};
use crate::words::{WordDecor, WordPool};

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
    pub default_mode: DefaultMode,
    /// Which English word pool to use in Time / Words / Zen modes. Default
    /// is the legacy `Common` pool so a missing config behaves like v0.3.
    pub word_pool: WordPool,
    /// Punctuation / numbers decoration toggles (v0.4.0).
    pub word_decor: WordDecor,
}

/// Coarse picker for which menu row to highlight on launch. Distinct from
/// `app::Mode` because we don't want config-loading to depend on the runtime
/// `Mode` enum's variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultMode {
    /// `time-Ns` — value is N (seconds).
    Time(u64),
    /// `words-N` — value is N (words).
    Words(usize),
    /// Single quote round.
    Quote,
    /// Language identifier matching `app::Mode::Code`.
    Code(CodeLangKind),
    /// Zen mode.
    Zen,
    /// `symbols-N` — value is N (symbol tokens). Added in v0.4.0.
    Symbols(usize),
}

/// Mirror of `crate::words::CodeLang` for config-resolution purposes.
/// Kept here so the config module doesn't depend on the words module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeLangKind {
    Rust,
    Python,
    JavaScript,
    Go,
    Java,
    Sql,
    Shell,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            palette: ThemePalette::default(),
            // Picked in agreement with the v0.2.0 spec: "default scheme time 15 sec".
            default_mode: DefaultMode::Time(15),
            word_pool: WordPool::Common,
            word_decor: WordDecor::default(),
        }
    }
}

/// Filesystem path of the config file: `~/.typerush/config.toml`.
pub fn config_path() -> PathBuf {
    storage::data_dir().join("config.toml")
}

/// Read and resolve the config from disk. Always succeeds:
///
///   - missing file        → defaults, no warnings
///   - unparseable file    → defaults, one warning
///   - unknown theme name  → fall back to dark, one warning
///   - bad color override  → keep the theme's slot, one warning per bad slot
///
/// CLI overrides that should take precedence over whatever the config file
/// says. Anything set to `None` falls through to the config value (or its
/// hardcoded default).
#[derive(Debug, Default, Clone)]
pub struct CliOverrides<'a> {
    /// `--theme <name>`.
    pub theme: Option<&'a str>,
    /// `--big` → `Some(WordPool::Extended)`.
    pub word_pool: Option<WordPool>,
    /// `--punctuation` → `Some(true)`; `--no-punctuation` → `Some(false)`.
    pub punctuation: Option<bool>,
    /// `--numbers` → `Some(true)`; `--no-numbers` → `Some(false)`.
    pub numbers: Option<bool>,
}

/// Read and resolve the config from disk, then apply CLI overrides on top.
///
/// Always succeeds:
///   - missing file        → defaults, no warnings
///   - unparseable file    → defaults, one warning
///   - unknown theme name  → fall back to dark, one warning
///   - bad color override  → keep the theme's slot, one warning per bad slot
pub fn load_or_default_with(cli: CliOverrides<'_>) -> (ResolvedConfig, Vec<String>) {
    let path = config_path();
    let raw_config = if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Config>(&text) {
                Ok(parsed) => parsed,
                Err(err) => {
                    let warning = format!("could not parse {}: {}", path.display(), err);
                    return (ResolvedConfig::default(), vec![warning]);
                }
            },
            Err(err) => {
                let warning = format!("could not read {}: {}", path.display(), err);
                return (ResolvedConfig::default(), vec![warning]);
            }
        }
    } else {
        Config::default()
    };
    resolve_with(raw_config, cli)
}

/// Pure resolver — easier to unit-test than the disk-touching variant.
/// Compat shim: takes a single `--theme` override only.
#[cfg(test)]
fn resolve(raw: Config, cli_theme_override: Option<&str>) -> (ResolvedConfig, Vec<String>) {
    resolve_with(
        raw,
        CliOverrides {
            theme: cli_theme_override,
            ..Default::default()
        },
    )
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

    (
        ResolvedConfig {
            palette,
            default_mode,
            word_pool,
            word_decor,
        },
        warnings,
    )
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
        Some(name) => match name.to_lowercase().as_str() {
            "common" | "small" | "1000" | "1k" => WordPool::Common,
            "extended" | "big" | "10000" | "10k" => WordPool::Extended,
            other => {
                warnings.push(format!(
                    "unknown word pool '{}', falling back to 'common'",
                    other
                ));
                WordPool::Common
            }
        },
    };
    let decor = WordDecor {
        punctuation: words.punctuation.unwrap_or(false),
        numbers: words.numbers.unwrap_or(false),
    };
    (pool, decor)
}

fn apply_color_overrides(
    base: ThemePalette,
    overrides: &Colors,
    warnings: &mut Vec<String>,
) -> ThemePalette {
    let mut parsed: Vec<(&str, Color)> = vec![];
    let mut try_push = |slot: &'static str, value: &Option<String>, warnings: &mut Vec<String>| {
        if let Some(input) = value {
            match theme::color::parse_color(input) {
                Ok(color) => parsed.push((slot, color)),
                Err(err) => warnings.push(format!("bad color for '{}': {}", slot, err)),
            }
        }
    };
    try_push("accent", &overrides.accent, warnings);
    try_push("secondary", &overrides.secondary, warnings);
    try_push("correct", &overrides.correct, warnings);
    try_push("incorrect", &overrides.incorrect, warnings);
    try_push("pending", &overrides.pending, warnings);
    try_push("extra", &overrides.extra, warnings);
    try_push("mode_tag", &overrides.mode_tag, warnings);
    try_push("error", &overrides.error, warnings);
    try_push("neutral", &overrides.neutral, warnings);
    try_push("background", &overrides.background, warnings);
    base.with_overrides(&parsed)
}

fn resolve_default_mode(
    defaults: Option<&super::Defaults>,
    warnings: &mut Vec<String>,
) -> DefaultMode {
    let Some(defaults) = defaults else {
        return DefaultMode::Time(15);
    };
    let mode_name = defaults.mode.as_deref().unwrap_or("time");
    match mode_name.to_lowercase().as_str() {
        "time" => DefaultMode::Time(defaults.time_seconds.unwrap_or(15)),
        "words" => DefaultMode::Words(defaults.word_count.unwrap_or(25)),
        "quote" => DefaultMode::Quote,
        "code" => DefaultMode::Code(parse_code_lang(
            defaults.code_lang.as_deref().unwrap_or("rust"),
            warnings,
        )),
        "zen" => DefaultMode::Zen,
        "symbols" => DefaultMode::Symbols(defaults.symbol_count.unwrap_or(25)),
        other => {
            warnings.push(format!(
                "unknown default mode '{}', falling back to 'time'",
                other
            ));
            DefaultMode::Time(defaults.time_seconds.unwrap_or(15))
        }
    }
}

/// Parse a code-lang config value into the canonical [`CodeLangKind`].
/// Public so the CLI parser in `main.rs` can use the same accepted-alias
/// table and warning style.
pub fn parse_code_lang(name: &str, warnings: &mut Vec<String>) -> CodeLangKind {
    match name.to_lowercase().as_str() {
        "rust" | "rs" => CodeLangKind::Rust,
        "python" | "py" => CodeLangKind::Python,
        "js" | "javascript" => CodeLangKind::JavaScript,
        "go" | "golang" => CodeLangKind::Go,
        "java" => CodeLangKind::Java,
        "sql" => CodeLangKind::Sql,
        "shell" | "sh" | "bash" => CodeLangKind::Shell,
        other => {
            warnings.push(format!(
                "unknown code_lang '{}', falling back to 'rust'",
                other
            ));
            CodeLangKind::Rust
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Colors, Defaults};

    #[test]
    fn default_when_config_is_empty() {
        let (resolved, warnings) = resolve(Config::default(), None);
        assert!(warnings.is_empty());
        assert_eq!(resolved.palette, builtin::DARK);
        assert_eq!(resolved.default_mode, DefaultMode::Time(15));
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
        assert_eq!(resolved.default_mode, DefaultMode::Words(100));
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
        assert_eq!(resolved.default_mode, DefaultMode::Time(15));
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
        assert_eq!(
            resolved.default_mode,
            DefaultMode::Code(CodeLangKind::Python)
        );
    }

    // ── v0.4.0 additions ────────────────────────────────────────────────────

    #[test]
    fn new_code_langs_resolve_correctly() {
        use crate::config::Defaults;
        for (input, expected) in [
            ("go", CodeLangKind::Go),
            ("golang", CodeLangKind::Go),
            ("java", CodeLangKind::Java),
            ("sql", CodeLangKind::Sql),
            ("shell", CodeLangKind::Shell),
            ("bash", CodeLangKind::Shell),
            ("sh", CodeLangKind::Shell),
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
            assert_eq!(resolved.default_mode, DefaultMode::Code(expected));
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
        assert_eq!(resolved.default_mode, DefaultMode::Symbols(40));
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
        assert_eq!(resolved.default_mode, DefaultMode::Symbols(25));
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
}
