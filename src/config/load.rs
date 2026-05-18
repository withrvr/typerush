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
}

/// Mirror of `crate::words::CodeLang` for config-resolution purposes.
/// Kept here so the config module doesn't depend on the words module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeLangKind {
    Rust,
    Python,
    JavaScript,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            palette: ThemePalette::default(),
            // Picked in agreement with the v0.2.0 spec: "default scheme time 15 sec".
            default_mode: DefaultMode::Time(15),
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
/// `cli_theme_override` lets `--theme` win over whatever the config says.
pub fn load_or_default(cli_theme_override: Option<&str>) -> (ResolvedConfig, Vec<String>) {
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
    resolve(raw_config, cli_theme_override)
}

/// Pure resolver — easier to unit-test than the disk-touching variant.
fn resolve(raw: Config, cli_theme_override: Option<&str>) -> (ResolvedConfig, Vec<String>) {
    let mut warnings = vec![];

    // Theme: CLI wins over config. Unknown name → warn, use dark.
    let theme_choice = cli_theme_override
        .map(str::to_string)
        .or_else(|| raw.theme.clone());
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

    (
        ResolvedConfig {
            palette,
            default_mode,
        },
        warnings,
    )
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
        other => {
            warnings.push(format!(
                "unknown default mode '{}', falling back to 'time'",
                other
            ));
            DefaultMode::Time(defaults.time_seconds.unwrap_or(15))
        }
    }
}

fn parse_code_lang(name: &str, warnings: &mut Vec<String>) -> CodeLangKind {
    match name.to_lowercase().as_str() {
        "rust" | "rs" => CodeLangKind::Rust,
        "python" | "py" => CodeLangKind::Python,
        "js" | "javascript" => CodeLangKind::JavaScript,
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
        assert_eq!(resolved.default_mode, DefaultMode::Code(CodeLangKind::Python));
    }
}
