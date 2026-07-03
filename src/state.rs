//! Small key-value state persisted alongside the stats file.
//!
//! `~/.typerush/state.json` is a tiny JSON blob that remembers UI preferences
//! between launches — currently just the last `--file` path the user typed
//! against, so the "Custom" menu row stays useful after the CLI flag is gone.
//!
//! Like stats persistence, every operation here is best-effort: a missing,
//! corrupt or unreadable file is treated as "no state", never as an error.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::storage;

// Per-thread override for the `state.json` location. `App::new` and
// `persist_custom_file` both touch disk, so tests that construct an `App`
// would otherwise clobber the real user's `~/.typerush/state.json`. A
// thread-local path lets each test point at its own tempdir without
// coordinating a global mutex.
#[cfg(test)]
thread_local! {
    static TEST_STATE_PATH: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// Redirect `state_path()` to `path` for the current thread. Test-only.
/// Pass `None` to clear the override and go back to `~/.typerush/state.json`.
#[cfg(test)]
pub(crate) fn set_test_state_path(path: Option<PathBuf>) {
    TEST_STATE_PATH.with(|cell| *cell.borrow_mut() = path);
}

/// On-disk shape of `state.json`.
///
/// Every field is optional and uses `#[serde(default)]` so the loader can
/// add new fields in future versions without breaking old files.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppState {
    /// Absolute path to the most recently used custom file (`--file <path>`
    /// or a snippet picked from the menu). `None` means the user has never
    /// run a custom-file session.
    pub last_custom_file: Option<String>,
}

/// Path to `~/.typerush/state.json` (or the thread-local test override,
/// when the caller is a `cfg(test)` build that installed one).
pub fn state_path() -> PathBuf {
    #[cfg(test)]
    {
        if let Some(p) = TEST_STATE_PATH.with(|c| c.borrow().clone()) {
            return p;
        }
    }
    storage::data_dir().join("state.json")
}

/// Load saved state from disk. Missing / corrupt files return `Default`.
pub fn load_state() -> AppState {
    load_from_path(&state_path())
}

/// Path-based variant of [`load_state`]. Exposed so tests can use a temp dir.
pub fn load_from_path(path: &Path) -> AppState {
    let Ok(raw) = fs::read_to_string(path) else {
        return AppState::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Persist `state` to `~/.typerush/state.json`. Best-effort — never panics.
pub fn save_state(state: &AppState) {
    let path = state_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = fs::write(path, json);
    }
}

/// Path-based variant of [`save_state`]. Used by tests.
#[cfg(test)]
fn save_to_path(path: &Path, state: &AppState) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        let state = load_from_path(&path);
        assert_eq!(state, AppState::default());
    }

    #[test]
    fn corrupt_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        fs::write(&path, "{ broken json").unwrap();
        let state = load_from_path(&path);
        assert_eq!(state, AppState::default());
    }

    #[test]
    fn roundtrip_preserves_custom_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let state = AppState {
            last_custom_file: Some("/tmp/typing-fodder.txt".to_string()),
        };
        save_to_path(&path, &state).unwrap();
        let loaded = load_from_path(&path);
        assert_eq!(loaded, state);
    }

    #[test]
    fn empty_object_loads_as_default() {
        // Forward-compat: an empty JSON object should yield a default state,
        // not error out. Old versions of TypeRush may have written one.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        fs::write(&path, "{}").unwrap();
        let state = load_from_path(&path);
        assert_eq!(state, AppState::default());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        // Forward-compat: future versions may add fields; old TypeRush must
        // still be able to read its own subset.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        fs::write(
            &path,
            r#"{"last_custom_file": "/tmp/x.txt", "future_thing": 42}"#,
        )
        .unwrap();
        let state = load_from_path(&path);
        assert_eq!(state.last_custom_file.as_deref(), Some("/tmp/x.txt"));
    }
}
