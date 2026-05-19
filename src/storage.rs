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
use std::collections::HashMap;
use std::{fs, path::PathBuf};

/// A single completed typing session, persisted to disk.
///
/// Field names are part of the on-disk format — don't rename them without a
/// migration story. New optional fields use `#[serde(default)]` so old records
/// without them deserialize cleanly.
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
    /// Per-key hit counts: number of times each key was typed correctly.
    /// Key is a single character serialised as a string (JSON map keys must be strings).
    /// Added in v0.3.0; old records without this field deserialise to an empty map.
    #[serde(default)]
    pub key_hits: HashMap<String, u64>,
    /// Per-key miss counts: number of times each key was typed but did not match.
    /// Added in v0.3.0; old records without this field deserialise to an empty map.
    #[serde(default)]
    pub key_misses: HashMap<String, u64>,
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

/// Best WPM ever achieved for the given mode label (e.g. `"time-30s"`).
/// Returns `None` when no sessions for that mode exist.
pub fn personal_best_for_mode(sessions: &[SessionRecord], mode_label: &str) -> Option<f64> {
    sessions
        .iter()
        .filter(|s| s.mode == mode_label)
        .map(|s| s.wpm)
        .fold(None, |best, wpm| match best {
            None => Some(wpm),
            Some(b) if wpm > b => Some(wpm),
            Some(b) => Some(b),
        })
}

/// Current daily streak: the number of consecutive calendar days (in local
/// time) ending on today or yesterday that each have at least one session.
///
/// Examples:
/// - Sessions today only → 1
/// - Sessions yesterday only → 1 (streak still active until tomorrow)
/// - Sessions today + yesterday → 2
/// - Last session 2 days ago → 0 (streak broken)
pub fn streak(sessions: &[SessionRecord]) -> u32 {
    use chrono::Duration;
    use std::collections::BTreeSet;

    if sessions.is_empty() {
        return 0;
    }

    let days_with_sessions: BTreeSet<chrono::NaiveDate> =
        sessions.iter().map(|s| s.timestamp.date_naive()).collect();

    let today = Local::now().date_naive();
    let yesterday = today - Duration::days(1);

    // Streak is only active if there's a session today or yesterday.
    let most_recent = match days_with_sessions.iter().next_back() {
        Some(d) => *d,
        None => return 0,
    };
    if most_recent < yesterday {
        return 0;
    }

    // Walk backwards from the anchor day until we find a gap.
    let anchor = if days_with_sessions.contains(&today) {
        today
    } else {
        yesterday
    };

    let mut count = 0u32;
    let mut day = anchor;
    loop {
        if days_with_sessions.contains(&day) {
            count += 1;
            day -= Duration::days(1);
        } else {
            break;
        }
    }
    count
}

/// Mean WPM across all sessions whose timestamp falls within the last `days`
/// calendar days (counted from now). Returns `None` when no sessions qualify.
pub fn avg_wpm_last_n_days(sessions: &[SessionRecord], days: u32) -> Option<f64> {
    let cutoff = Local::now() - chrono::Duration::days(days as i64);
    let relevant: Vec<f64> = sessions
        .iter()
        .filter(|s| s.timestamp > cutoff)
        .map(|s| s.wpm)
        .collect();
    if relevant.is_empty() {
        None
    } else {
        Some(relevant.iter().sum::<f64>() / relevant.len() as f64)
    }
}

/// Aggregated per-key accuracy across all sessions, sorted by accuracy
/// ascending (worst keys first). Only keys with at least `min_presses`
/// total keystrokes (hits + misses) are included.
pub fn key_accuracy(sessions: &[SessionRecord], min_presses: u64) -> Vec<KeyAccuracyStat> {
    let mut hits: HashMap<char, u64> = HashMap::new();
    let mut misses: HashMap<char, u64> = HashMap::new();

    for session in sessions {
        for (k, &v) in &session.key_hits {
            if let Some(c) = k.chars().next() {
                *hits.entry(c).or_insert(0) += v;
            }
        }
        for (k, &v) in &session.key_misses {
            if let Some(c) = k.chars().next() {
                *misses.entry(c).or_insert(0) += v;
            }
        }
    }

    let all_keys: std::collections::HashSet<char> =
        hits.keys().chain(misses.keys()).copied().collect();

    let mut stats: Vec<KeyAccuracyStat> = all_keys
        .into_iter()
        .filter_map(|c| {
            let h = hits.get(&c).copied().unwrap_or(0);
            let m = misses.get(&c).copied().unwrap_or(0);
            let total = h + m;
            if total < min_presses {
                return None;
            }
            let accuracy = h as f64 / total as f64 * 100.0;
            Some(KeyAccuracyStat {
                key: c,
                total,
                hits: h,
                accuracy,
            })
        })
        .collect();

    // Worst keys first (lowest accuracy ascending).
    stats.sort_by(|a, b| {
        a.accuracy
            .partial_cmp(&b.accuracy)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    stats
}

/// Accuracy statistics for a single key, used by the key-accuracy heatmap.
#[derive(Debug, Clone)]
pub struct KeyAccuracyStat {
    /// The typed character.
    pub key: char,
    /// Total times this key was pressed (hits + misses).
    pub total: u64,
    /// Times this key was pressed at the correct position.
    pub hits: u64,
    /// Accuracy as a percentage, 0–100.
    pub accuracy: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal `SessionRecord` for testing purposes.
    fn make_record(wpm: f64, mode: &str, days_ago: i64) -> SessionRecord {
        let timestamp = Local::now() - chrono::Duration::days(days_ago);
        SessionRecord {
            wpm,
            accuracy: 95.0,
            mode: mode.to_string(),
            word_count: 30,
            correct_chars: 100,
            total_chars: 105,
            duration_secs: 30.0,
            timestamp,
            key_hits: HashMap::new(),
            key_misses: HashMap::new(),
        }
    }

    fn make_record_with_keys(
        wpm: f64,
        mode: &str,
        days_ago: i64,
        hits: &[(&str, u64)],
        misses: &[(&str, u64)],
    ) -> SessionRecord {
        let mut r = make_record(wpm, mode, days_ago);
        r.key_hits = hits.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        r.key_misses = misses.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        r
    }

    // ── personal_best ────────────────────────────────────────────────────────

    #[test]
    fn personal_best_empty_is_none() {
        assert!(personal_best(&[]).is_none());
    }

    #[test]
    fn personal_best_picks_max() {
        let sessions = vec![
            make_record(50.0, "time-30s", 0),
            make_record(80.0, "time-30s", 1),
        ];
        assert_eq!(personal_best(&sessions), Some(80.0));
    }

    // ── personal_best_for_mode ───────────────────────────────────────────────

    #[test]
    fn mode_pb_none_when_no_matching_mode() {
        let sessions = vec![make_record(60.0, "time-30s", 0)];
        assert!(personal_best_for_mode(&sessions, "words-50").is_none());
    }

    #[test]
    fn mode_pb_filters_by_mode() {
        let sessions = vec![
            make_record(100.0, "time-30s", 0),
            make_record(40.0, "words-50", 0),
            make_record(50.0, "words-50", 1),
        ];
        assert_eq!(personal_best_for_mode(&sessions, "words-50"), Some(50.0));
        assert_eq!(personal_best_for_mode(&sessions, "time-30s"), Some(100.0));
    }

    #[test]
    fn mode_pb_single_session() {
        let sessions = vec![make_record(42.0, "quote", 0)];
        assert_eq!(personal_best_for_mode(&sessions, "quote"), Some(42.0));
    }

    // ── streak ───────────────────────────────────────────────────────────────

    #[test]
    fn streak_empty_is_zero() {
        assert_eq!(streak(&[]), 0);
    }

    #[test]
    fn streak_today_is_one() {
        let sessions = vec![make_record(50.0, "time-30s", 0)];
        assert_eq!(streak(&sessions), 1);
    }

    #[test]
    fn streak_yesterday_is_one() {
        // A session yesterday keeps the streak alive for today.
        let sessions = vec![make_record(50.0, "time-30s", 1)];
        assert_eq!(streak(&sessions), 1);
    }

    #[test]
    fn streak_today_and_yesterday_is_two() {
        let sessions = vec![
            make_record(50.0, "time-30s", 0),
            make_record(55.0, "time-30s", 1),
        ];
        assert_eq!(streak(&sessions), 2);
    }

    #[test]
    fn streak_consecutive_five_days() {
        let sessions: Vec<_> = (0..5).map(|d| make_record(50.0, "time-30s", d)).collect();
        assert_eq!(streak(&sessions), 5);
    }

    #[test]
    fn streak_broken_by_gap() {
        // Today + 3 days ago (gap on day 1 and 2) → streak is 1 (only today).
        let sessions = vec![
            make_record(50.0, "time-30s", 0),
            make_record(50.0, "time-30s", 3),
        ];
        assert_eq!(streak(&sessions), 1);
    }

    #[test]
    fn streak_old_session_only_is_zero() {
        // Last session was 5 days ago — more than yesterday.
        let sessions = vec![make_record(50.0, "time-30s", 5)];
        assert_eq!(streak(&sessions), 0);
    }

    #[test]
    fn streak_multiple_sessions_same_day_count_as_one() {
        // Three sessions today should still count as streak = 1.
        let sessions = vec![
            make_record(50.0, "time-30s", 0),
            make_record(52.0, "time-30s", 0),
            make_record(54.0, "time-30s", 0),
        ];
        assert_eq!(streak(&sessions), 1);
    }

    // ── avg_wpm_last_n_days ───────────────────────────────────────────────────

    #[test]
    fn avg_wpm_empty_is_none() {
        assert!(avg_wpm_last_n_days(&[], 7).is_none());
    }

    #[test]
    fn avg_wpm_includes_recent_sessions() {
        let sessions = vec![
            make_record(50.0, "time-30s", 0),
            make_record(60.0, "time-30s", 1),
        ];
        let avg = avg_wpm_last_n_days(&sessions, 7).unwrap();
        assert!((avg - 55.0).abs() < 0.001);
    }

    #[test]
    fn avg_wpm_excludes_old_sessions() {
        let sessions = vec![
            make_record(50.0, "time-30s", 0),
            make_record(100.0, "time-30s", 10), // outside 7-day window
        ];
        let avg = avg_wpm_last_n_days(&sessions, 7).unwrap();
        assert!((avg - 50.0).abs() < 0.001);
    }

    #[test]
    fn avg_wpm_none_when_all_outside_window() {
        let sessions = vec![make_record(50.0, "time-30s", 10)];
        assert!(avg_wpm_last_n_days(&sessions, 7).is_none());
    }

    // ── key_accuracy ─────────────────────────────────────────────────────────

    #[test]
    fn key_accuracy_empty_sessions_returns_empty() {
        let stats = key_accuracy(&[], 1);
        assert!(stats.is_empty());
    }

    #[test]
    fn key_accuracy_aggregates_across_sessions() {
        let s1 = make_record_with_keys(50.0, "time-30s", 0, &[("a", 8)], &[("a", 2)]);
        let s2 = make_record_with_keys(50.0, "time-30s", 1, &[("a", 2)], &[("a", 8)]);
        // Total: 10 hits, 10 misses → 50% accuracy.
        let stats = key_accuracy(&[s1, s2], 1);
        let a_stat = stats.iter().find(|s| s.key == 'a').unwrap();
        assert_eq!(a_stat.total, 20);
        assert_eq!(a_stat.hits, 10);
        assert!((a_stat.accuracy - 50.0).abs() < 0.001);
    }

    #[test]
    fn key_accuracy_filters_by_min_presses() {
        let s = make_record_with_keys(50.0, "time-30s", 0, &[("a", 3), ("b", 1)], &[]);
        // min_presses = 3 → 'a' included (3 hits), 'b' excluded (1 hit).
        let stats = key_accuracy(&[s], 3);
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].key, 'a');
    }

    #[test]
    fn key_accuracy_sorted_worst_first() {
        let s = make_record_with_keys(
            50.0,
            "time-30s",
            0,
            &[("a", 9), ("b", 5)],
            &[("a", 1), ("b", 5)], // a=90%, b=50%
        );
        let stats = key_accuracy(&[s], 1);
        // 'b' is worse (50%) and should come first.
        assert_eq!(stats[0].key, 'b');
        assert_eq!(stats[1].key, 'a');
    }

    #[test]
    fn key_accuracy_perfect_key_is_100_percent() {
        let s = make_record_with_keys(50.0, "time-30s", 0, &[("z", 10)], &[]);
        let stats = key_accuracy(&[s], 1);
        assert_eq!(stats.len(), 1);
        assert!((stats[0].accuracy - 100.0).abs() < 0.001);
    }

    #[test]
    fn session_record_deserialises_without_key_fields() {
        // Simulate a pre-v0.3.0 record that lacks key_hits / key_misses.
        let json = r#"[{
            "wpm": 55.0,
            "accuracy": 96.0,
            "mode": "time-30s",
            "word_count": 25,
            "correct_chars": 200,
            "total_chars": 208,
            "duration_secs": 30.0,
            "timestamp": "2024-01-15T10:30:00+00:00"
        }]"#;
        let records: Vec<SessionRecord> = serde_json::from_str(json).unwrap();
        assert_eq!(records.len(), 1);
        assert!(records[0].key_hits.is_empty());
        assert!(records[0].key_misses.is_empty());
    }
}
