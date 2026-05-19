//! Reading and resolving `~/.typerush/config.toml`.
//!
//! The config is **entirely optional**: a missing file means defaults. A
//! malformed file means defaults plus a warning surfaced once via the error
//! modal. We never crash the app over a config problem.
//!
//! ## Schema
//!
//! ```toml
//! theme = "monokai"            # built-in name: dark | light | monokai | dracula
//!
//! [defaults]
//! mode = "time"                 # time | words | quote | code | zen
//! time_seconds = 15             # default duration for time mode
//! word_count = 25               # default count for words mode
//! code_lang = "rust"            # rust | python | js
//!
//! [colors]                      # optional — overrides slots of the chosen theme
//! accent = "#FF00FF"
//! correct = "green"
//! ```

pub mod load;

use serde::Deserialize;

/// Top-level config shape.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Built-in theme name, e.g. `"monokai"`. Unknown names trigger a warning
    /// and fall back to `dark`.
    pub theme: Option<String>,
    /// Pre-selected mode and per-mode defaults.
    pub defaults: Option<Defaults>,
    /// Optional per-slot color overrides applied on top of the chosen theme.
    pub colors: Option<Colors>,
    /// Word source / decoration toggles (v0.4.0).
    pub words: Option<Words>,
}

/// Default mode + per-mode defaults. Pre-selects the matching row in the menu
/// at startup.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    /// Which menu row to pre-select: `time` | `words` | `quote` | `code` |
    /// `zen` | `symbols`.
    pub mode: Option<String>,
    /// Seconds for `Mode::Time` (and for the time-mode menu row pre-selection).
    pub time_seconds: Option<u64>,
    /// Word count for `Mode::Words`.
    pub word_count: Option<usize>,
    /// Language for `Mode::Code`: `rust` | `python` | `js` | `go` | `java`
    /// | `sql` | `shell`.
    pub code_lang: Option<String>,
    /// Token count for `Mode::Symbols` (v0.4.0).
    pub symbol_count: Option<usize>,
}

/// `[words]` section: which pool to draw English words from and whether to
/// decorate words with punctuation or numbers. Added in v0.4.0.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Words {
    /// Pool name: `common` (default, ≈1k words) | `extended` (10k words).
    pub pool: Option<String>,
    /// When true, randomly attach punctuation marks (`,.;:?!"'` etc.) to
    /// roughly a quarter of words and occasionally wrap a word in paired
    /// brackets / quotes.
    pub punctuation: Option<bool>,
    /// When true, replace ~12% of slots with a random 1–4 digit number.
    pub numbers: Option<bool>,
}

/// Per-slot color overrides. Any field that's `Some` overrides the corresponding
/// slot of the chosen theme. Invalid color strings are ignored with a warning.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Colors {
    pub accent: Option<String>,
    pub secondary: Option<String>,
    pub correct: Option<String>,
    pub incorrect: Option<String>,
    pub pending: Option<String>,
    pub extra: Option<String>,
    pub mode_tag: Option<String>,
    pub error: Option<String>,
    pub neutral: Option<String>,
    pub background: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_config() {
        let raw = r#"theme = "monokai""#;
        let cfg: Config = toml::from_str(raw).unwrap();
        assert_eq!(cfg.theme.as_deref(), Some("monokai"));
        assert!(cfg.defaults.is_none());
        assert!(cfg.colors.is_none());
    }

    #[test]
    fn parses_full_config() {
        let raw = r##"
theme = "dracula"

[defaults]
mode = "time"
time_seconds = 30
word_count = 50
code_lang = "rust"

[colors]
accent = "#FF00FF"
correct = "green"
        "##;
        let cfg: Config = toml::from_str(raw).unwrap();
        assert_eq!(cfg.theme.as_deref(), Some("dracula"));
        let defaults = cfg.defaults.expect("defaults section parsed");
        assert_eq!(defaults.mode.as_deref(), Some("time"));
        assert_eq!(defaults.time_seconds, Some(30));
        assert_eq!(defaults.word_count, Some(50));
        assert_eq!(defaults.code_lang.as_deref(), Some("rust"));
        let colors = cfg.colors.expect("colors section parsed");
        assert_eq!(colors.accent.as_deref(), Some("#FF00FF"));
        assert_eq!(colors.correct.as_deref(), Some("green"));
        assert!(colors.incorrect.is_none());
    }

    #[test]
    fn empty_string_is_a_valid_config() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn unknown_field_rejected() {
        let raw = r#"
theme = "dark"
nonsense_field = 42
        "#;
        assert!(toml::from_str::<Config>(raw).is_err());
    }

    #[test]
    fn unknown_color_slot_rejected() {
        let raw = r#"
[colors]
not_a_real_slot = "red"
        "#;
        assert!(toml::from_str::<Config>(raw).is_err());
    }

    /// Regression guard: the example config we ship at the repo root must
    /// parse cleanly through the same deserializer the runtime uses. If
    /// someone adds a slot or renames a field, this fails before users see it.
    #[test]
    fn shipped_example_config_round_trips() {
        let example = include_str!("../../config.example.toml");
        let cfg: Config = toml::from_str(example).expect("config.example.toml must parse");
        assert_eq!(cfg.theme.as_deref(), Some("monokai"));
        let defaults = cfg.defaults.expect("defaults section");
        assert_eq!(defaults.mode.as_deref(), Some("time"));
        assert_eq!(defaults.time_seconds, Some(15));
    }
}
