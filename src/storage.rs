//! Persistent session history.
//!
//! Every finished session (apart from Zen mode) is appended as a JSON record to
//! `~/.typerush/stats.json`. The file is a simple flat array — no schema, no
//! migrations — so users can hand-edit or back it up trivially.
//!
//! All functions here are best-effort: the typing app never crashes if stats
//! can't be loaded or saved (the worst case is the user not seeing their PB).

use anyhow::Result;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

/// A single completed typing session, persisted to disk.
///
/// Field names are part of the on-disk format — don't rename them without a
/// migration story.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Final words-per-minute.
    pub wpm: f64,
    /// Final accuracy, 0–100.
    pub accuracy: f64,
    /// Mode label e.g. "time-30s", "words-50", "code-rust". See `Mode::label`.
    pub mode: String,
    /// Number of words the user advanced past during the session.
    pub word_count: usize,
    /// Characters typed that matched the target.
    pub correct_chars: usize,
    /// Every keystroke counted toward accuracy (correct + wrong + extras + spaces).
    pub total_chars: usize,
    /// Duration of the session in seconds.
    pub duration_secs: f64,
    /// When the session ended, in local time.
    pub timestamp: DateTime<Local>,
}

/// Directory we write to: `$HOME/.typerush`. Falls back to the current
/// directory if `$HOME` can't be resolved.
pub fn data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".typerush")
}

/// Full path to the JSON history file.
pub fn stats_path() -> PathBuf {
    data_dir().join("stats.json")
}

/// Append `record` to the stats file, creating the data directory if needed.
///
/// The whole file is rewritten on each save — fine in practice because the
/// file is tiny (a few hundred bytes per session) and the user only saves
/// once at the end of a session.
pub fn save_session(record: &SessionRecord) -> Result<()> {
    let directory = data_dir();
    fs::create_dir_all(&directory)?;
    let path = stats_path();

    let mut history: Vec<SessionRecord> = if path.exists() {
        let raw = fs::read_to_string(&path)?;
        // If the file is corrupt or empty, start over rather than panicking.
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        vec![]
    };
    history.push(record.clone());
    fs::write(&path, serde_json::to_string_pretty(&history)?)?;
    Ok(())
}

/// Load every saved session in the order they were recorded (oldest first).
/// Returns an empty vec when the file doesn't exist yet.
pub fn load_sessions() -> Result<Vec<SessionRecord>> {
    let path = stats_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

/// All-time best WPM across every saved session.
pub fn personal_best(sessions: &[SessionRecord]) -> Option<f64> {
    sessions
        .iter()
        .map(|s| s.wpm)
        .fold(None, |best, wpm| match best {
            None => Some(wpm),
            Some(current_best) if wpm > current_best => Some(wpm),
            Some(current_best) => Some(current_best),
        })
}

/// Mean accuracy across every saved session, 0–100. `None` if no sessions.
pub fn average_accuracy(sessions: &[SessionRecord]) -> Option<f64> {
    if sessions.is_empty() {
        return None;
    }
    let total: f64 = sessions.iter().map(|s| s.accuracy).sum();
    Some(total / sessions.len() as f64)
}
