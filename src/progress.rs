//! Levels and experience points, earned from time spent typing, and the
//! Profile screen's summary of everything typed so far.
//!
//! Every saved session earns one XP per whole second typed (paused time is
//! already left out of its duration) — so practice counts, however fast.
//! Each level asks for a little more than the last: 300 XP (five minutes)
//! from level 1 to 2, then 150 more for every level after.

use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::storage::{self, SessionRecord};

/// XP a session earned: one per whole second typed.
pub fn session_xp(session: &SessionRecord) -> u64 {
    session.duration_secs.max(0.0) as u64
}

/// XP earned by all of `sessions`.
pub fn total_xp<'a>(sessions: impl IntoIterator<Item = &'a SessionRecord>) -> u64 {
    sessions.into_iter().map(session_xp).sum()
}

/// XP it takes to go from `level` to the next one.
pub fn xp_to_next(level: u32) -> u64 {
    300 + 150 * u64::from(level.saturating_sub(1))
}

/// Where a total of XP puts you.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    /// The level reached, from 1.
    pub level: u32,
    /// XP earned since reaching it.
    pub into: u64,
    /// XP this level takes to finish ([`xp_to_next`]).
    pub needed: u64,
}

impl Default for Level {
    /// Level 1, nothing earned yet.
    fn default() -> Self {
        Self::of(0)
    }
}

impl Level {
    /// The level a total of `xp` reaches.
    pub fn of(mut xp: u64) -> Self {
        let mut level = 1;
        // ponytail: walks the levels one by one — level 200 is ~830 hours
        // of typing, so the loop stays tiny.
        while xp >= xp_to_next(level) {
            xp -= xp_to_next(level);
            level += 1;
        }
        Self {
            level,
            into: xp,
            needed: xp_to_next(level),
        }
    }

    /// How far into the level, 0.0–1.0.
    pub fn ratio(self) -> f64 {
        self.into as f64 / self.needed as f64
    }
}

/// One calendar day of typing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Day {
    pub runs: usize,
    pub secs: f64,
}

/// A standard run's best: (wpm, accuracy, runs played), `None` if never played.
pub type StandardBest = Option<(f64, f64, usize)>;

/// The time and word counts the Profile screen lines up side by side.
pub const STANDARD_TIMES: [u64; 4] = [15, 30, 60, 120];
pub const STANDARD_WORDS: [usize; 4] = [10, 25, 50, 100];

/// Everything the Profile screen shows, worked out from the history when
/// the screen opens — never per frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub xp: u64,
    pub level: Level,
    /// The day of the first saved session.
    pub since: Option<NaiveDate>,
    /// Typing per calendar day: days typed, streaks and the calendar.
    pub days: BTreeMap<NaiveDate, Day>,
    pub streak: u32,
    pub best_streak: u32,
    pub started: u64,
    /// Saved runs not ended with Esc.
    pub completed: usize,
    pub sessions: usize,
    pub time_typed_secs: f64,
    /// Every key typed, right or wrong.
    pub chars_typed: u64,
    /// Plain `time-15s` … `time-120s` and `words-10` … `words-100` bests.
    pub times: [StandardBest; 4],
    pub words: [StandardBest; 4],
    /// The mode played most, and how often.
    pub favourite: Option<(String, usize)>,
}

impl Profile {
    /// The profile of `history`, with `started` runs counted
    /// (`App::runs_started`).
    pub fn new(history: &[SessionRecord], started: u64) -> Self {
        let xp = total_xp(history);
        let mut days: BTreeMap<NaiveDate, Day> = BTreeMap::new();
        for session in history {
            let day = days.entry(session.timestamp.date_naive()).or_default();
            day.runs += 1;
            day.secs += session.duration_secs.max(0.0);
        }
        let bests = storage::bests_by_mode(history);
        let best_of = |label: String| -> StandardBest {
            let best = bests.iter().find(|best| best.mode == label)?;
            let record = &history[best.best_index];
            Some((record.wpm, record.accuracy, best.sessions))
        };
        let favourite = bests
            .iter()
            // Most runs; the first in menu order on a tie.
            .fold(None::<&storage::ModeBest>, |most, best| match most {
                Some(most) if most.sessions >= best.sessions => Some(most),
                _ => Some(best),
            })
            .map(|best| (best.mode.clone(), best.sessions));
        Self {
            xp,
            level: Level::of(xp),
            since: days.keys().next().copied(),
            streak: storage::streak(history),
            best_streak: longest_streak(days.keys().copied()),
            days,
            started,
            completed: history.iter().filter(|s| !s.ended_early).count(),
            sessions: history.len(),
            time_typed_secs: history.iter().map(|s| s.duration_secs.max(0.0)).sum(),
            chars_typed: history.iter().map(|s| s.total_chars as u64).sum(),
            times: STANDARD_TIMES.map(|seconds| best_of(format!("time-{seconds}s"))),
            words: STANDARD_WORDS.map(|count| best_of(format!("words-{count}"))),
            favourite,
        }
    }
}

/// The longest run of consecutive days in `days` (in order).
fn longest_streak(days: impl IntoIterator<Item = NaiveDate>) -> u32 {
    let (mut best, mut run, mut previous) = (0, 0, None::<NaiveDate>);
    for day in days {
        run = match previous {
            Some(previous) if previous.succ_opt() == Some(day) => run + 1,
            _ => 1,
        };
        best = best.max(run);
        previous = Some(day);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(duration_secs: f64) -> SessionRecord {
        SessionRecord {
            duration_secs,
            ..SessionRecord::default()
        }
    }

    #[test]
    fn xp_is_whole_seconds_typed() {
        assert_eq!(session_xp(&session(30.9)), 30);
        assert_eq!(session_xp(&session(-5.0)), 0);
        assert_eq!(total_xp(&[session(60.0), session(15.5)]), 75);
    }

    /// 300 XP for level 2, 450 more for level 3, …; the remainder carries.
    #[test]
    fn levels_ask_a_little_more_each_time() {
        assert_eq!(
            Level::of(0),
            Level {
                level: 1,
                into: 0,
                needed: 300
            }
        );
        assert_eq!(Level::of(299).level, 1);
        assert_eq!(
            Level::of(300),
            Level {
                level: 2,
                into: 0,
                needed: 450
            }
        );
        assert_eq!(Level::of(300 + 450 + 10).into, 10);
        assert_eq!(Level::of(300 + 450 + 10).level, 3);
        assert!((Level::of(300 + 225).ratio() - 0.5).abs() < 1e-9);
        // Ten hours of typing: 34,500 XP reaches level 21.
        assert_eq!(Level::of(36_000).level, 21);
    }

    fn played(mode: &str, wpm: f64, days_ago: u64) -> SessionRecord {
        SessionRecord {
            mode: mode.into(),
            wpm,
            accuracy: 95.0,
            duration_secs: 30.0,
            total_chars: 160,
            timestamp: chrono::Local::now() - chrono::Days::new(days_ago),
            ..SessionRecord::default()
        }
    }

    /// The profile adds everything up: XP and level, first day, days typed,
    /// streaks (current and longest), completed runs, time and keys typed,
    /// the plain time / words bests side by side, the favourite mode.
    #[test]
    fn profile_adds_it_all_up() {
        let mut history = vec![
            played("time-30s", 60.0, 10),
            played("time-30s", 70.0, 9),
            played("time-30s+p", 99.0, 8),
            played("words-25", 50.0, 1),
            played("time-30s", 65.0, 0),
        ];
        history[3].ended_early = true;
        let profile = Profile::new(&history, 9);
        assert_eq!(profile.xp, 150);
        assert_eq!(profile.level, Level::of(150));
        assert_eq!(profile.since, Some(history[0].timestamp.date_naive()));
        assert_eq!(profile.days.len(), 5);
        assert_eq!((profile.streak, profile.best_streak), (2, 3));
        assert_eq!((profile.started, profile.completed), (9, 4));
        assert_eq!(profile.chars_typed, 800);
        assert_eq!(profile.times[1], Some((70.0, 95.0, 3)), "plain only");
        assert_eq!(profile.times[0], None);
        assert_eq!(profile.words[1], Some((50.0, 95.0, 1)));
        assert_eq!(profile.favourite, Some(("time-30s".into(), 3)));
        assert_eq!(Profile::new(&[], 0).level, Level::default());
    }

    #[test]
    fn longest_streak_finds_the_longest_run() {
        let day = |d| NaiveDate::from_ymd_opt(2026, 1, d).unwrap();
        assert_eq!(longest_streak([]), 0);
        assert_eq!(longest_streak([day(1), day(2), day(4), day(5), day(6)]), 3);
        assert_eq!(longest_streak([day(9)]), 1);
    }
}
