//! Programmatic editing of `~/.typerush/config.toml` (v0.5.0).
//!
//! Powers two front doors:
//!   - the `typerush config get/set/show/reset/path` CLI subcommands
//!   - the in-app Settings screen (theme / default-mode pickers)
//!
//! Design rules:
//!   - **Comment-preserving.** Edits go through `toml_edit`, so a hand-written
//!     config keeps its comments and layout after a `config set`. (`toml_edit`
//!     already sits in the dependency tree — the `toml` crate is built on it.)
//!   - **Whitelisted keys, validated values.** Only keys the runtime actually
//!     reads can be set, and every value is validated with the same rules the
//!     loader applies, so a `config set` can never produce a config that warns
//!     on the next launch.
//!   - **Path-parameterized.** Every function takes the file path explicitly
//!     so tests run against temp dirs; `config_path()` supplies the real one.

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::theme::{builtin, color};

/// Starter config written by `typerush --init-config` — the same annotated
/// example that ships at the repository root.
pub const STARTER_CONFIG: &str = include_str!("../../config.example.toml");

/// The value type expected behind a config key. Drives both validation and
/// how the value is written into the TOML document (string vs integer vs bool).
enum ValueKind {
    /// A built-in theme name (`dark`, `light`, `monokai`, `dracula`).
    Theme,
    /// A default-mode name (`time`, `words`, `quote`, `code`, `zen`, `symbols`).
    ModeName,
    /// A non-negative integer (seconds / counts).
    Integer,
    /// A code-language name or alias (`rust`, `py`, `golang`, …).
    CodeLang,
    /// A word-pool name or alias (`common`, `extended`, `10k`, …).
    Pool,
    /// `true` / `false`.
    Bool,
    /// A color: `#RRGGBB`, an ANSI name, or `reset`.
    Color,
}

/// Every key `config get` / `config set` accepts, with the built-in default
/// the runtime falls back to when the key is unset. Color slots default to
/// the active theme's value, which isn't a constant — represented as `None`.
const KEYS: &[(&str, Option<&str>)] = &[
    ("theme", Some("dark")),
    ("defaults.mode", Some("time")),
    ("defaults.time_seconds", Some("15")),
    ("defaults.word_count", Some("25")),
    ("defaults.code_lang", Some("rust")),
    ("defaults.symbol_count", Some("25")),
    ("words.pool", Some("common")),
    ("words.punctuation", Some("false")),
    ("words.numbers", Some("false")),
    ("colors.accent", None),
    ("colors.secondary", None),
    ("colors.correct", None),
    ("colors.incorrect", None),
    ("colors.pending", None),
    ("colors.extra", None),
    ("colors.mode_tag", None),
    ("colors.error", None),
    ("colors.neutral", None),
    ("colors.background", None),
];

/// Human-readable list of accepted keys, for error messages and `--help`.
pub fn known_keys() -> Vec<&'static str> {
    KEYS.iter().map(|(k, _)| *k).collect()
}

/// The built-in default for `key`, when it has a static one.
fn default_for(key: &str) -> Option<&'static str> {
    KEYS.iter().find(|(k, _)| *k == key).and_then(|(_, d)| *d)
}

/// Map a whitelisted key to its expected value type. `None` = unknown key.
fn kind_for(key: &str) -> Option<ValueKind> {
    match key {
        "theme" => Some(ValueKind::Theme),
        "defaults.mode" => Some(ValueKind::ModeName),
        "defaults.time_seconds" | "defaults.word_count" | "defaults.symbol_count" => {
            Some(ValueKind::Integer)
        }
        "defaults.code_lang" => Some(ValueKind::CodeLang),
        "words.pool" => Some(ValueKind::Pool),
        "words.punctuation" | "words.numbers" => Some(ValueKind::Bool),
        _ => match key.strip_prefix("colors.") {
            Some(slot) if default_for(key).is_none() && is_color_slot(slot) => {
                Some(ValueKind::Color)
            }
            _ => None,
        },
    }
}

/// The ten themable slots — must match `config::Colors` field names.
fn is_color_slot(slot: &str) -> bool {
    matches!(
        slot,
        "accent"
            | "secondary"
            | "correct"
            | "incorrect"
            | "pending"
            | "extra"
            | "mode_tag"
            | "error"
            | "neutral"
            | "background"
    )
}

/// Validate `value` for `key` using the same acceptance rules the config
/// loader applies at startup. Returns a human-readable error on rejection.
fn validate(key: &str, value: &str) -> Result<ValueKind, String> {
    let Some(kind) = kind_for(key) else {
        return Err(format!(
            "unknown key '{}'. Valid keys: {}",
            key,
            known_keys().join(", ")
        ));
    };
    match kind {
        ValueKind::Theme => {
            if builtin::by_name(value).is_none() {
                return Err(format!(
                    "unknown theme '{}' (try one of: {})",
                    value,
                    builtin::names().join(", ")
                ));
            }
        }
        ValueKind::ModeName => {
            let accepted = ["time", "words", "quote", "code", "zen", "symbols"];
            if !accepted.contains(&value.to_lowercase().as_str()) {
                return Err(format!(
                    "unknown mode '{}' (try one of: {})",
                    value,
                    accepted.join(", ")
                ));
            }
        }
        ValueKind::Integer => {
            // Parse as i64 — the widest integer TOML can represent — so the
            // later `as i64` write in set_value can never overflow.
            let parsed: i64 = value
                .parse()
                .map_err(|_| format!("'{}' is not a whole number", value))?;
            if parsed <= 0 {
                return Err(format!("{} must be greater than zero", key));
            }
        }
        ValueKind::CodeLang => {
            let accepted = [
                "rust",
                "rs",
                "python",
                "py",
                "js",
                "javascript",
                "go",
                "golang",
                "java",
                "sql",
                "shell",
                "sh",
                "bash",
            ];
            if !accepted.contains(&value.to_lowercase().as_str()) {
                return Err(format!(
                    "unknown code_lang '{}' (try one of: rust, python, js, go, java, sql, shell)",
                    value
                ));
            }
        }
        ValueKind::Pool => {
            let accepted = [
                "common", "small", "1000", "1k", "extended", "big", "10000", "10k",
            ];
            if !accepted.contains(&value.to_lowercase().as_str()) {
                return Err(format!(
                    "unknown pool '{}' (try 'common' or 'extended')",
                    value
                ));
            }
        }
        ValueKind::Bool => {
            if value != "true" && value != "false" {
                return Err(format!("'{}' is not a bool (use true or false)", value));
            }
        }
        ValueKind::Color => {
            color::parse_color(value).map_err(|e| e.to_string())?;
        }
    }
    Ok(kind)
}

/// Read the value of `key` from the config file at `path`.
///
/// Returns `Ok(Some(value))` when the key is set, `Ok(None)` when the file or
/// key is absent (caller decides how to present the default), and `Err` for
/// an unknown key or an unreadable/unparseable file.
pub fn get_value(path: &Path, key: &str) -> Result<Option<String>, String> {
    if kind_for(key).is_none() {
        return Err(format!(
            "unknown key '{}'. Valid keys: {}",
            key,
            known_keys().join(", ")
        ));
    }
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("could not read {}: {}", path.display(), e))?;
    let doc: toml_edit::DocumentMut = raw
        .parse()
        .map_err(|e| format!("could not parse {}: {}", path.display(), e))?;
    let item = match key.split_once('.') {
        None => doc.get(key),
        Some((section, field)) => doc.get(section).and_then(|s| s.get(field)),
    };
    Ok(item.and_then(display_value))
}

/// Render a TOML value the way a user would type it (`monokai`, `30`, `true`),
/// without the quotes `toml_edit` would add around strings.
fn display_value(item: &toml_edit::Item) -> Option<String> {
    let value = item.as_value()?;
    Some(match value {
        toml_edit::Value::String(s) => s.value().clone(),
        other => other.to_string().trim().to_string(),
    })
}

/// The built-in default shown by `config get` when `key` is unset. Color
/// slots have no static default (they fall back to the active theme's slot).
pub fn describe_default(key: &str) -> String {
    match default_for(key) {
        Some(default) => format!("{} (default)", default),
        None => "unset (falls back to the theme's slot)".to_string(),
    }
}

/// Set `key = value` in the config file at `path`, preserving every comment
/// and all formatting of the existing file. Creates the file (and parent
/// directory) when missing. The key must be whitelisted and the value valid.
pub fn set_value(path: &Path, key: &str, value: &str) -> Result<(), String> {
    let kind = validate(key, value)?;

    let raw = if path.exists() {
        fs::read_to_string(path).map_err(|e| format!("could not read {}: {}", path.display(), e))?
    } else {
        String::new()
    };
    let mut doc: toml_edit::DocumentMut = raw
        .parse()
        .map_err(|e| format!("could not parse {}: {}", path.display(), e))?;

    let toml_value = match kind {
        ValueKind::Integer => toml_edit::value(value.parse::<i64>().expect("validated as integer")),
        ValueKind::Bool => toml_edit::value(value == "true"),
        _ => toml_edit::value(value),
    };
    match key.split_once('.') {
        None => {
            doc[key] = toml_value;
        }
        Some((section, field)) => {
            // Auto-create the section as a proper `[section]` table when the
            // file doesn't have one yet.
            if doc.get(section).is_none() {
                doc[section] = toml_edit::Item::Table(toml_edit::Table::new());
            }
            doc[section][field] = toml_value;
        }
    }

    // Safety net: the edited document must round-trip through the same
    // deserializer the app boots with. This can only fail if the whitelist
    // above drifts from the `Config` struct — better a hard error here than
    // a warning modal for the user at next launch.
    let rendered = doc.to_string();
    toml::from_str::<Config>(&rendered)
        .map_err(|e| format!("internal error: edit produced an invalid config: {}", e))?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {}", parent.display(), e))?;
    }
    fs::write(path, rendered).map_err(|e| format!("could not write {}: {}", path.display(), e))
}

/// Remove the config file, returning to built-in defaults. The old file is
/// kept as `config.toml.bak` next to the original so a reset is reversible.
///
/// Returns `Ok(Some(backup_path))` when a file was reset, `Ok(None)` when
/// there was nothing to reset (already at defaults).
pub fn reset(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let backup = path.with_extension("toml.bak");
    fs::copy(path, &backup)
        .map_err(|e| format!("could not back up to {}: {}", backup.display(), e))?;
    fs::remove_file(path).map_err(|e| format!("could not remove {}: {}", path.display(), e))?;
    Ok(Some(backup))
}

/// Write the starter config to `path` (`--init-config`). Refuses to touch an
/// existing file — `config set` edits values in place and `config reset`
/// clears it, so overwriting silently would only lose user edits.
pub fn init_config(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "{} already exists — edit it with `typerush config set <key> <value>`, or run `typerush config reset` first",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {}", parent.display(), e))?;
    }
    fs::write(path, STARTER_CONFIG)
        .map_err(|e| format!("could not write {}: {}", path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        (dir, path)
    }

    // ── set / get round-trips ────────────────────────────────────────────────

    #[test]
    fn set_then_get_round_trips_on_fresh_file() {
        let (_dir, path) = temp_config();
        set_value(&path, "theme", "monokai").unwrap();
        assert_eq!(
            get_value(&path, "theme").unwrap().as_deref(),
            Some("monokai")
        );
    }

    #[test]
    fn set_nested_key_creates_section() {
        let (_dir, path) = temp_config();
        set_value(&path, "defaults.time_seconds", "60").unwrap();
        assert_eq!(
            get_value(&path, "defaults.time_seconds")
                .unwrap()
                .as_deref(),
            Some("60")
        );
        // Written as a real integer, not a string.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("time_seconds = 60"), "raw was: {raw}");
    }

    #[test]
    fn set_bool_writes_toml_bool() {
        let (_dir, path) = temp_config();
        set_value(&path, "words.punctuation", "true").unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("punctuation = true"), "raw was: {raw}");
    }

    #[test]
    fn set_color_slot_accepts_hex_and_names() {
        let (_dir, path) = temp_config();
        set_value(&path, "colors.accent", "#FF00FF").unwrap();
        set_value(&path, "colors.background", "reset").unwrap();
        assert_eq!(
            get_value(&path, "colors.accent").unwrap().as_deref(),
            Some("#FF00FF")
        );
    }

    // ── comment preservation ─────────────────────────────────────────────────

    #[test]
    fn set_preserves_comments_and_other_keys() {
        let (_dir, path) = temp_config();
        std::fs::write(
            &path,
            "# my precious comment\ntheme = \"dark\"\n\n[defaults]\n# inline note\nmode = \"words\"\n",
        )
        .unwrap();
        set_value(&path, "theme", "dracula").unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("# my precious comment"));
        assert!(raw.contains("# inline note"));
        assert!(raw.contains("theme = \"dracula\""));
        assert!(raw.contains("mode = \"words\""));
    }

    // ── validation ───────────────────────────────────────────────────────────

    #[test]
    fn unknown_key_is_rejected_by_set_and_get() {
        let (_dir, path) = temp_config();
        let err = set_value(&path, "nonsense.key", "1").unwrap_err();
        assert!(err.contains("unknown key"));
        let err = get_value(&path, "nonsense.key").unwrap_err();
        assert!(err.contains("unknown key"));
    }

    #[test]
    fn invalid_values_are_rejected() {
        let (_dir, path) = temp_config();
        assert!(set_value(&path, "theme", "hotdog-stand").is_err());
        assert!(set_value(&path, "defaults.mode", "hyperspeed").is_err());
        assert!(set_value(&path, "defaults.time_seconds", "abc").is_err());
        assert!(set_value(&path, "defaults.time_seconds", "0").is_err());
        assert!(set_value(&path, "defaults.time_seconds", "-30").is_err());
        // u64-but-not-i64 values must be a clean error, not an overflow
        // panic in the TOML write.
        assert!(set_value(&path, "defaults.time_seconds", "18446744073709551615").is_err());
        assert!(set_value(&path, "words.pool", "gigantic").is_err());
        assert!(set_value(&path, "words.numbers", "yes").is_err());
        assert!(set_value(&path, "colors.accent", "not-a-color").is_err());
        // Nothing was written by any failed set.
        assert!(!path.exists());
    }

    #[test]
    fn code_lang_aliases_accepted() {
        let (_dir, path) = temp_config();
        for alias in ["rust", "py", "javascript", "golang", "bash"] {
            set_value(&path, "defaults.code_lang", alias).unwrap();
        }
    }

    /// The whitelist and the `Config` struct must not drift apart: every
    /// settable key round-trips through the real deserializer (exercised by
    /// the sanity re-parse inside `set_value`).
    #[test]
    fn every_known_key_can_be_set() {
        let (_dir, path) = temp_config();
        for key in known_keys() {
            let value = match kind_for(key).unwrap() {
                ValueKind::Theme => "monokai",
                ValueKind::ModeName => "words",
                ValueKind::Integer => "30",
                ValueKind::CodeLang => "python",
                ValueKind::Pool => "extended",
                ValueKind::Bool => "true",
                ValueKind::Color => "#123456",
            };
            set_value(&path, key, value).unwrap_or_else(|e| panic!("could not set {key}: {e}"));
        }
    }

    // ── get on missing file / key ────────────────────────────────────────────

    #[test]
    fn get_missing_file_or_key_returns_none() {
        let (_dir, path) = temp_config();
        assert_eq!(get_value(&path, "theme").unwrap(), None);
        std::fs::write(&path, "theme = \"dark\"\n").unwrap();
        assert_eq!(get_value(&path, "words.pool").unwrap(), None);
    }

    #[test]
    fn describe_default_covers_static_and_theme_slots() {
        assert_eq!(describe_default("theme"), "dark (default)");
        assert_eq!(describe_default("defaults.time_seconds"), "15 (default)");
        assert!(describe_default("colors.accent").contains("theme"));
    }

    // ── reset ────────────────────────────────────────────────────────────────

    #[test]
    fn reset_backs_up_then_removes() {
        let (_dir, path) = temp_config();
        std::fs::write(&path, "theme = \"monokai\"\n").unwrap();
        let backup = reset(&path).unwrap().expect("backup path");
        assert!(!path.exists());
        assert!(backup.exists());
        let saved = std::fs::read_to_string(backup).unwrap();
        assert!(saved.contains("monokai"));
    }

    #[test]
    fn reset_with_no_file_is_noop() {
        let (_dir, path) = temp_config();
        assert_eq!(reset(&path).unwrap(), None);
    }

    // ── init-config ──────────────────────────────────────────────────────────

    #[test]
    fn init_config_writes_starter_and_refuses_overwrite() {
        let (_dir, path) = temp_config();
        init_config(&path).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert_eq!(raw, STARTER_CONFIG);
        // Second run must refuse rather than clobber.
        std::fs::write(&path, "theme = \"dracula\"\n").unwrap();
        let err = init_config(&path).unwrap_err();
        assert!(err.contains("already exists"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "theme = \"dracula\"\n"
        );
    }

    /// The starter config must itself parse — guards against the example
    /// file drifting out of sync with the schema.
    #[test]
    fn starter_config_parses_through_runtime_loader() {
        let cfg: Config = toml::from_str(STARTER_CONFIG).unwrap();
        assert_eq!(cfg.theme.as_deref(), Some("monokai"));
    }
}
