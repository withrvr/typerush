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
use std::{
    fs,
    path::{Path, PathBuf},
};

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
    /// The session's full target word list, in order — everything the user was
    /// asked to type (including words never reached in a time-mode session),
    /// so the session can be replayed word-for-word.
    /// Added in v0.5.0; old records without this field deserialise to an empty
    /// vec, which the replay UI reports as "no replay data".
    #[serde(default)]
    pub words: Vec<String>,
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

/// Full path to the incremental key-accuracy aggregate file.
fn aggregate_path() -> PathBuf {
    data_dir().join("aggregate.json")
}

/// Cumulative per-key press totals across **all** saved sessions.
///
/// Stored in `~/.typerush/aggregate.json` and updated with each session save
/// (O(keys_in_session)), so key-accuracy reads stay O(1) regardless of how
/// large `stats.json` grows.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregateStats {
    /// Total correct presses per key across every saved session.
    #[serde(default)]
    pub key_hits: HashMap<String, u64>,
    /// Total wrong / extra presses per key across every saved session.
    #[serde(default)]
    pub key_misses: HashMap<String, u64>,
}

/// Load the aggregate from disk. Returns a zeroed `AggregateStats` when the
/// file is missing or unreadable (first run, or after manual deletion).
pub fn load_aggregate() -> AggregateStats {
    let path = aggregate_path();
    if !path.exists() {
        return AggregateStats::default();
    }
    let Ok(raw) = fs::read_to_string(&path) else {
        return AggregateStats::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Persist the aggregate to disk. Best-effort — never panics.
pub fn save_aggregate(agg: &AggregateStats) {
    let path = aggregate_path();
    if let Ok(json) = serde_json::to_string_pretty(agg) {
        let _ = fs::write(path, json);
    }
}

/// Apply one session's key data to an in-memory aggregate. Pure — no disk I/O.
/// Call `save_aggregate` separately when you want to persist the result.
pub fn apply_session_to_aggregate(agg: &mut AggregateStats, record: &SessionRecord) {
    for (k, &v) in &record.key_hits {
        *agg.key_hits.entry(k.clone()).or_insert(0) += v;
    }
    for (k, &v) in &record.key_misses {
        *agg.key_misses.entry(k.clone()).or_insert(0) += v;
    }
}

/// Append `record` to the stats file, creating the data directory if needed.
/// Also updates `aggregate.json` incrementally so key-accuracy reads stay fast.
///
/// The whole stats file is rewritten on each save — fine in practice because
/// the file is tiny and the user only saves once at the end of a session.
pub fn save_session(record: &SessionRecord) -> Result<()> {
    fs::create_dir_all(data_dir())?;
    save_session_to_path(&stats_path(), record)?;
    let mut agg = load_aggregate();
    apply_session_to_aggregate(&mut agg, record);
    save_aggregate(&agg);
    Ok(())
}

/// Load every saved session in the order they were recorded (oldest first).
/// Returns an empty vec when the file doesn't exist yet.
pub fn load_sessions() -> Result<Vec<SessionRecord>> {
    load_sessions_from_path(&stats_path())
}

/// Path-based variant of [`save_session`]. Exposed for tests (and any future
/// caller that wants to control where stats are stored).
pub fn save_session_to_path(path: &Path, record: &SessionRecord) -> Result<()> {
    let mut history: Vec<SessionRecord> = if path.exists() {
        let raw = fs::read_to_string(path)?;
        // If the file is corrupt or empty, start over rather than panicking.
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        vec![]
    };
    history.push(record.clone());
    fs::write(path, serde_json::to_string_pretty(&history)?)?;
    Ok(())
}

/// Path-based variant of [`load_sessions`]. A missing file is treated as an
/// empty history; a corrupt file falls back to an empty list (the same
/// loss-tolerant behavior used in production).
pub fn load_sessions_from_path(path: &Path) -> Result<Vec<SessionRecord>> {
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
/// Used by tests; production code uses `key_accuracy_from_aggregate`.
#[cfg(test)]
fn key_accuracy(sessions: &[SessionRecord], min_presses: u64) -> Vec<KeyAccuracyStat> {
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

    // Worst keys first (lowest accuracy ascending); break ties alphabetically
    // so equal-accuracy keys never swap positions between renders.
    stats.sort_by(|a, b| {
        a.accuracy
            .partial_cmp(&b.accuracy)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.key.cmp(&b.key))
    });
    stats
}

/// Same as `key_accuracy` but reads from a pre-computed `AggregateStats`
/// instead of iterating all sessions. O(distinct_keys) — always fast.
/// Prefer this in the render path.
pub fn key_accuracy_from_aggregate(agg: &AggregateStats, min_presses: u64) -> Vec<KeyAccuracyStat> {
    let all_keys: std::collections::HashSet<&str> = agg
        .key_hits
        .keys()
        .chain(agg.key_misses.keys())
        .map(String::as_str)
        .collect();

    let mut stats: Vec<KeyAccuracyStat> = all_keys
        .into_iter()
        .filter_map(|k| {
            let c = k.chars().next()?;
            let h = agg.key_hits.get(k).copied().unwrap_or(0);
            let m = agg.key_misses.get(k).copied().unwrap_or(0);
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

    stats.sort_by(|a, b| {
        a.accuracy
            .partial_cmp(&b.accuracy)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.key.cmp(&b.key))
    });
    stats
}

/// Render the full session history as CSV (v0.5.0, `--export-csv`).
///
/// One row per session, oldest first — the same order as `stats.json`.
/// Timestamps are RFC 3339 so spreadsheets and scripts parse them without
/// guessing; numeric columns use two decimal places. Per-key maps and the
/// replay word list are deliberately not exported — CSV is for the tabular
/// stats, the JSON file remains the lossless source.
pub fn sessions_to_csv(sessions: &[SessionRecord]) -> String {
    let mut out = String::from(
        "timestamp,mode,wpm,accuracy,word_count,correct_chars,total_chars,duration_secs\n",
    );
    for s in sessions {
        out.push_str(&format!(
            "{},{},{:.2},{:.2},{},{},{},{:.2}\n",
            csv_escape(&s.timestamp.to_rfc3339()),
            csv_escape(&s.mode),
            s.wpm,
            s.accuracy,
            s.word_count,
            s.correct_chars,
            s.total_chars,
            s.duration_secs,
        ));
    }
    out
}

/// Minimal CSV quoting (RFC 4180): fields containing a comma, quote, or
/// newline are wrapped in double quotes with inner quotes doubled. Everything
/// else passes through untouched.
fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
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
            words: vec![],
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
    fn key_accuracy_stable_order_for_equal_accuracy() {
        // 'r' and 'n' both at 80% — alphabetical tiebreaker must keep 'n' before 'r'.
        let s = make_record_with_keys(
            50.0,
            "time-30s",
            0,
            &[("r", 4), ("n", 4)],
            &[("r", 1), ("n", 1)], // r=80%, n=80%
        );
        let stats = key_accuracy(&[s], 1);
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].key, 'n');
        assert_eq!(stats[1].key, 'r');
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

    // ═══════════════════════════════════════════════════════════════════════
    //  Phase 5 — comprehensive stats-analysis test suite (v0.3.0)
    //
    //  Goal: prove that every analysis helper produces correct results
    //  across a wide variety of session-history shapes, including the
    //  legacy formats produced by v0.1 and v0.2 (no key_hits / key_misses).
    // ═══════════════════════════════════════════════════════════════════════

    // ── Scenario 1: empty history ────────────────────────────────────────────

    #[test]
    fn scenario_1_empty_history_all_helpers_return_safe_defaults() {
        let sessions: Vec<SessionRecord> = vec![];
        assert_eq!(personal_best(&sessions), None);
        assert_eq!(personal_best_for_mode(&sessions, "time-30s"), None);
        assert_eq!(average_accuracy(&sessions), None);
        assert_eq!(streak(&sessions), 0);
        assert_eq!(avg_wpm_last_n_days(&sessions, 7), None);
        assert_eq!(avg_wpm_last_n_days(&sessions, 30), None);
        assert!(key_accuracy(&sessions, 1).is_empty());
    }

    // ── Scenario 2: exactly 10 sessions, varied modes ────────────────────────

    fn scenario_10_varied() -> Vec<SessionRecord> {
        vec![
            make_record_with_keys(45.0, "time-15s", 0, &[("e", 50)], &[("e", 5)]),
            make_record_with_keys(52.0, "time-30s", 0, &[("t", 80)], &[("t", 4)]),
            make_record_with_keys(48.0, "time-60s", 1, &[("a", 90)], &[("a", 12)]),
            make_record_with_keys(60.0, "time-30s", 1, &[("o", 70)], &[("o", 8)]),
            make_record_with_keys(55.0, "words-25", 2, &[("i", 60)], &[("i", 6)]),
            make_record_with_keys(58.0, "words-50", 2, &[("n", 65)], &[("n", 10)]),
            make_record_with_keys(70.0, "time-30s", 3, &[("s", 75)], &[("s", 3)]),
            make_record_with_keys(40.0, "quote", 4, &[("h", 30)], &[("h", 9)]),
            make_record_with_keys(50.0, "code-rust", 5, &[("r", 40)], &[("r", 4)]),
            make_record_with_keys(65.0, "time-30s", 6, &[("u", 45)], &[("u", 7)]),
        ]
    }

    #[test]
    fn scenario_2_ten_varied_personal_best_is_max() {
        let s = scenario_10_varied();
        assert_eq!(personal_best(&s), Some(70.0));
    }

    #[test]
    fn scenario_2_ten_varied_mode_pb_filters_correctly() {
        let s = scenario_10_varied();
        // Three time-30s sessions: 52, 60, 70 → PB = 70.
        assert_eq!(personal_best_for_mode(&s, "time-30s"), Some(70.0));
        // Single time-60s session.
        assert_eq!(personal_best_for_mode(&s, "time-60s"), Some(48.0));
        // Single quote session.
        assert_eq!(personal_best_for_mode(&s, "quote"), Some(40.0));
        // No code-python sessions exist.
        assert_eq!(personal_best_for_mode(&s, "code-python"), None);
    }

    #[test]
    fn scenario_2_ten_varied_average_accuracy_is_mean() {
        let s = scenario_10_varied();
        // All records were built with accuracy = 95.0.
        let avg = average_accuracy(&s).unwrap();
        assert!((avg - 95.0).abs() < 0.001);
    }

    #[test]
    fn scenario_2_ten_varied_key_accuracy_top_worst() {
        let s = scenario_10_varied();
        let stats = key_accuracy(&s, 1);
        // We seeded 10 distinct keys, all should appear (each has ≥ 33 hits).
        assert_eq!(stats.len(), 10);
        // Worst-first: the lowest accuracy ratio is whichever (hits / total) is smallest.
        // h = 30, m = 9 → 30/39 = 76.9% — 'h' is the worst key.
        assert_eq!(stats[0].key, 'h');
    }

    // ── Scenario 3: a single session ─────────────────────────────────────────

    #[test]
    fn scenario_3_single_session_all_helpers() {
        let s = vec![make_record(60.0, "time-30s", 0)];
        assert_eq!(personal_best(&s), Some(60.0));
        assert_eq!(personal_best_for_mode(&s, "time-30s"), Some(60.0));
        assert_eq!(personal_best_for_mode(&s, "words-25"), None);
        assert!((average_accuracy(&s).unwrap() - 95.0).abs() < 0.001);
        assert_eq!(streak(&s), 1);
        assert!((avg_wpm_last_n_days(&s, 7).unwrap() - 60.0).abs() < 0.001);
        assert!((avg_wpm_last_n_days(&s, 30).unwrap() - 60.0).abs() < 0.001);
    }

    // ── Scenario 4: very large history (10,000 sessions) ─────────────────────

    #[test]
    fn scenario_4_large_history_personal_best_correct_and_fast() {
        // 10k sessions. WPM increases monotonically; the final session is the
        // PB. We also seed `key_hits` on every record so key_accuracy has work.
        let sessions: Vec<SessionRecord> = (0..10_000)
            .map(|i| {
                make_record_with_keys(
                    50.0 + (i as f64) * 0.001,
                    "time-30s",
                    (i % 30) as i64, // spread over ~30 days
                    &[("a", 20), ("b", 18)],
                    &[("a", 2), ("b", 4)],
                )
            })
            .collect();

        let start = std::time::Instant::now();
        let pb = personal_best(&sessions).unwrap();
        let mode_pb = personal_best_for_mode(&sessions, "time-30s").unwrap();
        let avg = average_accuracy(&sessions).unwrap();
        let keys = key_accuracy(&sessions, 1);
        let elapsed = start.elapsed();

        // Sanity checks.
        assert!((pb - 59.999).abs() < 0.01);
        assert!((mode_pb - 59.999).abs() < 0.01);
        assert!((avg - 95.0).abs() < 0.001);
        // 'b' is worse than 'a' (b: 18/22 vs a: 20/22) → worst comes first.
        assert_eq!(keys[0].key, 'b');
        assert_eq!(keys[1].key, 'a');
        // 10k records should aggregate well under 1s on any modern machine.
        assert!(
            elapsed.as_secs() < 5,
            "analysis took too long: {:?}",
            elapsed
        );
    }

    // ── Scenario 5: large history — streak computation ───────────────────────

    #[test]
    fn scenario_5_large_history_streak_picks_recent_run() {
        // Sessions on days 0..30, then a gap, then days 60..80. Streak should
        // be 30 (today + 29 yesterdays) since the active run ends today.
        let mut sessions = vec![];
        for d in 0..30 {
            sessions.push(make_record(50.0, "time-30s", d));
        }
        for d in 60..80 {
            sessions.push(make_record(50.0, "time-30s", d));
        }
        assert_eq!(streak(&sessions), 30);
    }

    // ── Scenario 6: backward compat — pure v0.1/v0.2 JSON loads correctly ───

    #[test]
    fn scenario_6_pure_legacy_json_loads_and_aggregates() {
        // Mimic a real-world ~/.typerush/stats.json from v0.1 — no key fields.
        let legacy_json = r#"[
            {"wpm": 42.5, "accuracy": 94.0, "mode": "time-15s", "word_count": 10, "correct_chars": 50, "total_chars": 53, "duration_secs": 15.0, "timestamp": "2024-01-01T10:00:00+00:00"},
            {"wpm": 50.0, "accuracy": 96.5, "mode": "time-30s", "word_count": 25, "correct_chars": 125, "total_chars": 130, "duration_secs": 30.0, "timestamp": "2024-01-02T11:00:00+00:00"},
            {"wpm": 55.5, "accuracy": 97.0, "mode": "words-25", "word_count": 25, "correct_chars": 138, "total_chars": 142, "duration_secs": 30.0, "timestamp": "2024-01-03T12:00:00+00:00"},
            {"wpm": 48.0, "accuracy": 92.5, "mode": "code-rust", "word_count": 30, "correct_chars": 100, "total_chars": 108, "duration_secs": 25.0, "timestamp": "2024-01-04T13:00:00+00:00"},
            {"wpm": 60.0, "accuracy": 98.0, "mode": "time-30s", "word_count": 30, "correct_chars": 150, "total_chars": 153, "duration_secs": 30.0, "timestamp": "2024-01-05T14:00:00+00:00"}
        ]"#;
        let sessions: Vec<SessionRecord> =
            serde_json::from_str(legacy_json).expect("legacy JSON must load");

        // 1. Every record loaded.
        assert_eq!(sessions.len(), 5);
        // 2. Each one has empty key maps (no per-key data).
        for s in &sessions {
            assert!(s.key_hits.is_empty());
            assert!(s.key_misses.is_empty());
        }
        // 3. v0.1/v0.2 helpers still work over legacy data.
        assert_eq!(personal_best(&sessions), Some(60.0));
        assert_eq!(personal_best_for_mode(&sessions, "time-30s"), Some(60.0));
        assert_eq!(personal_best_for_mode(&sessions, "code-rust"), Some(48.0));
        let avg = average_accuracy(&sessions).unwrap();
        let expected = (94.0 + 96.5 + 97.0 + 92.5 + 98.0) / 5.0;
        assert!((avg - expected).abs() < 0.001);
        // 4. v0.3 helpers handle legacy data gracefully (no key data → empty heatmap).
        assert!(key_accuracy(&sessions, 1).is_empty());
    }

    // ── Scenario 7: mixed legacy + new format in one file ────────────────────

    #[test]
    fn scenario_7_mixed_legacy_and_new_records_coexist() {
        let mixed_json = r#"[
            {"wpm": 50.0, "accuracy": 95.0, "mode": "time-30s", "word_count": 25, "correct_chars": 125, "total_chars": 131, "duration_secs": 30.0, "timestamp": "2024-01-01T10:00:00+00:00"},
            {"wpm": 60.0, "accuracy": 96.0, "mode": "time-30s", "word_count": 30, "correct_chars": 150, "total_chars": 156, "duration_secs": 30.0, "timestamp": "2024-01-02T10:00:00+00:00", "key_hits": {"e": 20, "t": 15}, "key_misses": {"e": 2}}
        ]"#;
        let sessions: Vec<SessionRecord> = serde_json::from_str(mixed_json).unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions[0].key_hits.is_empty()); // legacy record
        assert_eq!(sessions[1].key_hits.get("e"), Some(&20)); // new record

        // Aggregation must include both records.
        assert_eq!(personal_best(&sessions), Some(60.0));
        // Key accuracy only sees the new record's contribution.
        let keys = key_accuracy(&sessions, 1);
        let e = keys.iter().find(|k| k.key == 'e').unwrap();
        assert_eq!(e.total, 22);
        assert_eq!(e.hits, 20);
    }

    // ── Scenario 8: full JSON serialization round-trip ──────────────────────

    #[test]
    fn scenario_8_session_record_json_roundtrip_preserves_all_fields() {
        let mut hits: HashMap<String, u64> = HashMap::new();
        hits.insert("a".into(), 12);
        hits.insert("z".into(), 3);
        let mut misses: HashMap<String, u64> = HashMap::new();
        misses.insert("q".into(), 1);

        let original = SessionRecord {
            wpm: 73.4,
            accuracy: 97.2,
            mode: "code-python".into(),
            word_count: 40,
            correct_chars: 200,
            total_chars: 206,
            duration_secs: 45.5,
            timestamp: Local::now(),
            key_hits: hits.clone(),
            key_misses: misses.clone(),
            words: vec!["def".into(), "main():".into()],
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: SessionRecord = serde_json::from_str(&json).unwrap();

        assert!((restored.wpm - 73.4).abs() < 1e-9);
        assert!((restored.accuracy - 97.2).abs() < 1e-9);
        assert_eq!(restored.mode, "code-python");
        assert_eq!(restored.word_count, 40);
        assert_eq!(restored.correct_chars, 200);
        assert_eq!(restored.total_chars, 206);
        assert!((restored.duration_secs - 45.5).abs() < 1e-9);
        assert_eq!(restored.key_hits, hits);
        assert_eq!(restored.key_misses, misses);
        assert_eq!(
            restored.words,
            vec!["def".to_string(), "main():".to_string()]
        );
    }

    // ── Scenario 9: corrupt JSON file → empty vec (no panic) ────────────────

    #[test]
    fn scenario_9_corrupt_json_falls_back_to_empty() {
        // load_sessions_from_path uses serde_json::from_str(...).unwrap_or_default()
        // on a corrupt file, so a malformed history should yield an empty Vec.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stats.json");
        std::fs::write(&path, "{ this is { not [ valid json").unwrap();
        let loaded = load_sessions_from_path(&path).unwrap();
        assert!(loaded.is_empty());
    }

    // ── Scenario 10: empty file → empty vec ─────────────────────────────────

    #[test]
    fn scenario_10_empty_file_loads_as_empty_vec() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stats.json");
        std::fs::write(&path, "").unwrap();
        let loaded = load_sessions_from_path(&path).unwrap();
        assert!(loaded.is_empty());
    }

    // ── Scenario 11: nonexistent file → empty vec ───────────────────────────

    #[test]
    fn scenario_11_missing_file_loads_as_empty_vec() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.json");
        let loaded = load_sessions_from_path(&path).unwrap();
        assert!(loaded.is_empty());
    }

    // ── Scenario 12: save then load roundtrip (single session) ──────────────

    #[test]
    fn scenario_12_save_then_load_single_session() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stats.json");
        let record = make_record_with_keys(
            72.0,
            "time-30s",
            0,
            &[("e", 30), ("t", 25)],
            &[("e", 1), ("t", 3)],
        );
        save_session_to_path(&path, &record).unwrap();
        let loaded = load_sessions_from_path(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert!((loaded[0].wpm - 72.0).abs() < 1e-9);
        assert_eq!(loaded[0].key_hits.get("e"), Some(&30));
        assert_eq!(loaded[0].key_misses.get("t"), Some(&3));
    }

    // ── Scenario 13: save 50 sessions in sequence (file grows correctly) ────

    #[test]
    fn scenario_13_save_appends_correctly_over_many_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stats.json");
        for i in 0..50 {
            let r = make_record(50.0 + i as f64, "time-30s", 0);
            save_session_to_path(&path, &r).unwrap();
        }
        let loaded = load_sessions_from_path(&path).unwrap();
        assert_eq!(loaded.len(), 50);
        // The most recently appended record sits at the end (file is newest-last).
        assert!((loaded.last().unwrap().wpm - 99.0).abs() < 1e-9);
        // Personal best across all 50 saved records.
        assert_eq!(personal_best(&loaded), Some(99.0));
    }

    // ── Scenario 14: save_session into an existing legacy file ──────────────

    #[test]
    fn scenario_14_save_into_existing_legacy_file_preserves_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stats.json");
        // Seed the file with a single v0.1 record (no key fields).
        std::fs::write(
            &path,
            r#"[{"wpm": 50.0, "accuracy": 95.0, "mode": "time-30s", "word_count": 25, "correct_chars": 125, "total_chars": 132, "duration_secs": 30.0, "timestamp": "2024-01-01T10:00:00+00:00"}]"#,
        )
        .unwrap();

        // Save a new v0.3 record into the same file.
        let new_record = make_record_with_keys(65.0, "time-30s", 0, &[("e", 20)], &[("e", 2)]);
        save_session_to_path(&path, &new_record).unwrap();

        // Both records should now be present.
        let loaded = load_sessions_from_path(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        // The legacy record is preserved with empty key maps.
        assert!((loaded[0].wpm - 50.0).abs() < 1e-9);
        assert!(loaded[0].key_hits.is_empty());
        // The new record retains its key data.
        assert_eq!(loaded[1].key_hits.get("e"), Some(&20));
    }

    // ── Scenario 15: streak edge — all sessions on the same day ─────────────

    #[test]
    fn scenario_15_streak_many_sessions_same_day() {
        // 100 sessions today → streak is exactly 1 day.
        let sessions: Vec<_> = (0..100).map(|_| make_record(50.0, "time-30s", 0)).collect();
        assert_eq!(streak(&sessions), 1);
    }

    // ── Scenario 16: 7-day vs 30-day average should differ when warranted ───

    #[test]
    fn scenario_16_rolling_averages_have_different_windows() {
        let sessions = vec![
            make_record(40.0, "time-30s", 0),   // in both windows
            make_record(50.0, "time-30s", 5),   // in both windows
            make_record(80.0, "time-30s", 10),  // only in 30-day
            make_record(100.0, "time-30s", 20), // only in 30-day
        ];
        // 7-day window covers the two recent sessions: avg = (40 + 50) / 2 = 45.
        let avg7 = avg_wpm_last_n_days(&sessions, 7).unwrap();
        assert!((avg7 - 45.0).abs() < 0.001);
        // 30-day window covers all four: avg = (40+50+80+100)/4 = 67.5.
        let avg30 = avg_wpm_last_n_days(&sessions, 30).unwrap();
        assert!((avg30 - 67.5).abs() < 0.001);
    }

    // ── Scenario 17: key accuracy ordering with many keys ───────────────────

    #[test]
    fn scenario_17_key_accuracy_full_alphabet_sorted_worst_first() {
        // Make a single session with 26 keys whose accuracy decreases as the
        // letter ascends — 'a' is best, 'z' is worst.
        let mut hits = vec![];
        let mut misses = vec![];
        for i in 0..26u8 {
            let ch = (b'a' + i) as char;
            let h: u64 = 100 - i as u64; // a→100, z→75
            let m: u64 = i as u64; // a→0,  z→25
                                   // We need to extend the lifetime of the temporary `String` —
                                   // collect into a Vec<String> first, then turn into &str slices.
            hits.push((ch.to_string(), h));
            misses.push((ch.to_string(), m));
        }
        let hits_ref: Vec<(&str, u64)> = hits.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        let misses_ref: Vec<(&str, u64)> = misses.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        let s = make_record_with_keys(50.0, "time-30s", 0, &hits_ref, &misses_ref);
        let stats = key_accuracy(&[s], 1);
        assert_eq!(stats.len(), 26);
        // Worst (z) first, best (a) last.
        assert_eq!(stats[0].key, 'z');
        assert_eq!(stats[25].key, 'a');
        // Sanity: monotonically increasing accuracy.
        for i in 1..stats.len() {
            assert!(stats[i].accuracy >= stats[i - 1].accuracy);
        }
    }

    // ── Scenario 18: key accuracy with unicode characters ──────────────────

    #[test]
    fn scenario_18_key_accuracy_supports_unicode() {
        let s = make_record_with_keys(50.0, "time-30s", 0, &[("é", 8), ("中", 5)], &[("é", 2)]);
        let stats = key_accuracy(&[s], 1);
        let e = stats.iter().find(|k| k.key == 'é').unwrap();
        assert_eq!(e.total, 10);
        assert_eq!(e.hits, 8);
        let zh = stats.iter().find(|k| k.key == '中').unwrap();
        assert_eq!(zh.total, 5);
        assert_eq!(zh.hits, 5);
        assert!((zh.accuracy - 100.0).abs() < 0.001);
    }

    // ── Scenario 19: combined scenario — realistic 30-day user ──────────────

    #[test]
    fn scenario_19_realistic_30_day_user_full_picture() {
        // A user who practiced every day for the last 14 days, taking 2-4
        // sessions per day across a mix of modes.
        let modes = ["time-15s", "time-30s", "words-25", "code-rust", "quote"];
        let mut sessions = vec![];
        for day in 0..14i64 {
            for s_in_day in 0..3 {
                let wpm = 40.0 + (day as f64) * 1.5 + (s_in_day as f64);
                let mode = modes[(day as usize + s_in_day) % modes.len()];
                sessions.push(make_record_with_keys(
                    wpm,
                    mode,
                    day,
                    &[("e", 30), ("t", 25), ("a", 20)],
                    &[("e", 1), ("t", 3), ("a", 2)],
                ));
            }
        }

        // 14 days * 3 sessions/day = 42 records.
        assert_eq!(sessions.len(), 42);
        // Streak should be 14 (full run ending today).
        assert_eq!(streak(&sessions), 14);
        // PB: the highest WPM is day=13, s_in_day=2 → 40 + 13*1.5 + 2 = 61.5.
        assert!((personal_best(&sessions).unwrap() - 61.5).abs() < 0.001);
        // 7-day avg covers days 0-6 (today through 7 days ago): always > 0.
        assert!(avg_wpm_last_n_days(&sessions, 7).is_some());
        // 30-day avg covers everything.
        assert!(avg_wpm_last_n_days(&sessions, 30).is_some());
        // Key accuracy: 't' is worst (25/28 = 89.3%), 'e' best (30/31 = 96.8%).
        let keys = key_accuracy(&sessions, 1);
        assert_eq!(keys[0].key, 't');
        assert_eq!(keys[2].key, 'e');
    }

    // ── Scenario 20: zero-WPM and 100% accuracy degenerates ─────────────────

    #[test]
    fn scenario_20_zero_wpm_does_not_become_none() {
        // A session with WPM 0.0 (e.g. user opened a session, typed nothing,
        // hit Esc immediately — though save_session would skip this in
        // production, the analysis must still handle it cleanly).
        let sessions = vec![make_record(0.0, "time-30s", 0)];
        assert_eq!(personal_best(&sessions), Some(0.0));
        assert_eq!(personal_best_for_mode(&sessions, "time-30s"), Some(0.0));
    }

    // ── Scenario 21: streak with a session exactly 2 days ago ───────────────

    #[test]
    fn scenario_21_session_two_days_ago_is_broken_streak() {
        let sessions = vec![make_record(50.0, "time-30s", 2)];
        // Most recent day is 2 days ago — not today, not yesterday → streak = 0.
        assert_eq!(streak(&sessions), 0);
    }

    // ── Scenario 22: streak with sessions today + 2 days ago (gap) ──────────

    #[test]
    fn scenario_22_streak_today_then_gap() {
        let sessions = vec![
            make_record(50.0, "time-30s", 0), // today
            make_record(50.0, "time-30s", 2), // 2 days ago — does not bridge gap
        ];
        // Streak counts only today; yesterday is missing so the run is 1.
        assert_eq!(streak(&sessions), 1);
    }

    // ── Scenario 23: mode PB unaffected by sessions in other modes ─────────

    #[test]
    fn scenario_23_mode_pb_does_not_leak_across_modes() {
        let sessions = vec![
            make_record(120.0, "code-rust", 0), // very high but wrong mode
            make_record(55.0, "time-30s", 0),
            make_record(60.0, "time-30s", 1),
        ];
        assert_eq!(personal_best_for_mode(&sessions, "time-30s"), Some(60.0));
        // The 120 WPM code session does NOT bleed into time-30s.
    }

    // ── Scenario 24: key accuracy with min_presses set very high ───────────

    #[test]
    fn scenario_24_key_accuracy_min_presses_filters_aggressively() {
        let s = make_record_with_keys(50.0, "time-30s", 0, &[("a", 5), ("b", 50), ("c", 100)], &[]);
        // min_presses = 60: only 'c' (100 total) qualifies.
        let stats = key_accuracy(&[s], 60);
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].key, 'c');
    }

    // ── v0.5.0: CSV export ───────────────────────────────────────────────────

    #[test]
    fn csv_empty_history_is_header_only() {
        let csv = sessions_to_csv(&[]);
        assert_eq!(
            csv,
            "timestamp,mode,wpm,accuracy,word_count,correct_chars,total_chars,duration_secs\n"
        );
    }

    #[test]
    fn csv_row_per_session_in_history_order() {
        let sessions = vec![
            make_record(50.0, "time-30s", 1),
            make_record(60.5, "words-50", 0),
        ];
        let csv = sessions_to_csv(&sessions);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3); // header + 2 rows
        assert!(lines[1].contains("time-30s"));
        assert!(lines[1].contains("50.00"));
        assert!(lines[2].contains("words-50"));
        assert!(lines[2].contains("60.50"));
    }

    #[test]
    fn csv_rows_have_exactly_eight_columns() {
        let sessions = vec![make_record(72.25, "code-rust", 0)];
        let csv = sessions_to_csv(&sessions);
        let row = csv.lines().nth(1).unwrap();
        assert_eq!(row.split(',').count(), 8, "row was: {row}");
        // Timestamp is RFC 3339 — contains a 'T' date/time separator.
        assert!(row.split(',').next().unwrap().contains('T'));
    }

    #[test]
    fn csv_escapes_commas_and_quotes() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_escape("line\nbreak"), "\"line\nbreak\"");
    }

    #[test]
    fn csv_does_not_leak_replay_words_or_key_maps() {
        let mut record = make_record_with_keys(50.0, "time-30s", 0, &[("e", 5)], &[("t", 1)]);
        record.words = vec!["secret-word".into()];
        let csv = sessions_to_csv(&[record]);
        assert!(!csv.contains("secret-word"));
        assert!(!csv.contains("key_hits"));
    }

    // ── Scenario 25: data dir / stats path resolve correctly ───────────────

    #[test]
    fn scenario_25_data_dir_and_stats_path_resolve() {
        let dir = data_dir();
        let stats = stats_path();
        // stats_path is data_dir + "stats.json".
        assert_eq!(stats.file_name().unwrap(), "stats.json");
        assert_eq!(stats.parent().unwrap(), dir);
    }
}
