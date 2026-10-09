//! Small key-value state persisted alongside the stats file.
//!
//! `~/.typerush/state.json` remembers UI choices between launches — currently
//! just the last custom file the user typed (a `--file` path or a snippet),
//! so the menu's "custom" row can offer it again without the CLI flag.
//!
//! Like stats persistence, everything here is best-effort: a missing, corrupt
//! or unreadable file is treated as "no state", never as an error. Callers
//! pass the file path in (normally [`state_path`]) so tests never touch the
//! user's real home directory.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::storage;

/// On-disk shape of `state.json`.
///
/// `#[serde(default)]` on the struct lets future versions add fields without
/// breaking old files, and unknown fields from newer versions are ignored.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppState {
    /// Path of the most recently started custom file. Stored as a string so
    /// the file stays human-readable; non-UTF-8 paths are simply not saved.
    pub last_custom_file: Option<String>,
}

/// Path to `~/.typerush/state.json`.
pub fn state_path() -> PathBuf {
    storage::data_dir().join("state.json")
}

/// Load state from `path`. Missing or corrupt files give the default state.
pub fn load_from_path(path: &Path) -> AppState {
    let Ok(raw) = fs::read_to_string(path) else {
        return AppState::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Write `state` to `path`, creating the parent directory if needed. Uses the
/// same atomic, fsynced write as `stats.json`, so a crash mid-save can never
/// leave a truncated file behind.
pub fn save_to_path(path: &Path, state: &AppState) -> std::io::Result<()> {
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    storage::write_atomic(path, &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load_from_path(&dir.path().join("nope.json")),
            AppState::default()
        );
    }

    #[test]
    fn corrupt_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        fs::write(&path, "{ broken json").unwrap();
        assert_eq!(load_from_path(&path), AppState::default());
    }

    #[test]
    fn roundtrip_preserves_custom_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let state = AppState {
            last_custom_file: Some("/tmp/typing-fodder.txt".to_string()),
        };
        save_to_path(&path, &state).unwrap();
        assert_eq!(load_from_path(&path), state);
    }

    #[test]
    fn save_creates_missing_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("fresh")
            .join(".typerush")
            .join("state.json");
        save_to_path(&path, &AppState::default()).unwrap();
        assert!(path.exists());
    }

    /// Forward compatibility: an empty object and unknown fields both load.
    #[test]
    fn empty_object_and_unknown_fields_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        fs::write(&path, "{}").unwrap();
        assert_eq!(load_from_path(&path), AppState::default());
        fs::write(
            &path,
            r#"{"last_custom_file": "/tmp/x.txt", "future_thing": 42}"#,
        )
        .unwrap();
        assert_eq!(
            load_from_path(&path).last_custom_file.as_deref(),
            Some("/tmp/x.txt")
        );
    }

    /// Same atomic-write invariant as stats.json: no temp file left behind,
    /// and the target parses.
    #[test]
    fn atomic_save_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let state = AppState {
            last_custom_file: Some("/tmp/atomic.txt".to_string()),
        };
        save_to_path(&path, &state).unwrap();
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
        assert_eq!(load_from_path(&path), state);
    }
}
