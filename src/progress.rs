//! Levels and experience points, earned from time spent typing.
//!
//! Every saved session earns one XP per whole second typed (paused time is
//! already left out of its duration) — so practice counts, however fast.
//! Each level asks for a little more than the last: 300 XP (five minutes)
//! from level 1 to 2, then 150 more for every level after.

use crate::storage::SessionRecord;

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
}
