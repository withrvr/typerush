//! Word sources — picks the text the user has to type.
//!
//! Each public function returns a `Vec<String>` of "words", split on
//! whitespace. The typing engine treats each entry as one token that needs to
//! be matched character-by-character, separated by a single space in the UI.

pub mod english;
pub mod quotes;
pub mod snippets;
pub mod symbols;

use rand::seq::SliceRandom;
use rand::{thread_rng, Rng};

/// Which English-word pool to draw from in [`random_words_from`].
///
/// `Common` is the original frequency-sorted pool that's been used since v0.1
/// — fastest to type, no surprises. `Extended` adds a much larger 10,000-word
/// dictionary on top, which trades some smoothness for vocabulary breadth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WordPool {
    /// Frequency-sorted ≈1,000 most common words. Default.
    #[default]
    Common,
    /// Larger 10,000-word pool — includes the common words plus a wide
    /// dictionary sample. Picked when the user opts in via config or CLI.
    Extended,
}

/// Optional decoration applied on top of random words. Each toggle adds a
/// distinct flavor of "extra characters" to make practice tougher:
/// punctuation drills the keys typists usually under-train, and numbers
/// force home-row → number-row excursions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WordDecor {
    /// Append/prepend punctuation marks to a fraction of words.
    pub punctuation: bool,
    /// Replace a fraction of words with random integer literals.
    pub numbers: bool,
}

/// Pick `count` random English words from the requested pool, optionally
/// decorated with punctuation / numbers.
pub fn random_words_from(count: usize, pool: WordPool, decor: WordDecor) -> Vec<String> {
    let mut rng = thread_rng();
    let words: &[&str] = match pool {
        WordPool::Common => english::ENGLISH_1000,
        WordPool::Extended => english::ENGLISH_10000,
    };
    (0..count)
        .map(|_| {
            // ~12% of slots become numbers when the toggle is on — frequent
            // enough to be felt, rare enough not to dominate the session.
            if decor.numbers && rng.gen_bool(0.12) {
                return random_number(&mut rng);
            }
            let base = words.choose(&mut rng).copied().unwrap_or("the").to_string();
            if decor.punctuation && rng.gen_bool(0.25) {
                decorate_with_punctuation(&base, &mut rng)
            } else {
                base
            }
        })
        .collect()
}

/// Generate a short numeric literal (1–4 digits). Used by the numbers toggle.
fn random_number(rng: &mut impl Rng) -> String {
    // 1–4 digits, weighted toward 2–3 digits so the screen doesn't fill with
    // 0-9 single characters.
    let len = rng.gen_range(1..=4);
    let mut s = String::with_capacity(len);
    for i in 0..len {
        // Avoid leading zero on multi-digit numbers — they look weird ("042").
        let digit = if i == 0 && len > 1 {
            rng.gen_range(1..=9)
        } else {
            rng.gen_range(0..=9)
        };
        s.push(char::from(b'0' + digit as u8));
    }
    s
}

/// Wrap or append a punctuation mark to `word`. Mirrors the punctuation a
/// user actually sees in natural text — commas, periods, semicolons, quotes,
/// occasionally parentheses around a word.
fn decorate_with_punctuation(word: &str, rng: &mut impl Rng) -> String {
    // Punctuation that goes *after* a word (most common in prose).
    const TAIL: &[&str] = &[",", ".", ";", ":", "?", "!", "...", "\"", "'"];
    // 1 in 5 punctuation slots wraps the word with paired marks.
    if rng.gen_bool(0.2) {
        let pairs = [("\"", "\""), ("'", "'"), ("(", ")"), ("[", "]")];
        let (open, close) = pairs[rng.gen_range(0..pairs.len())];
        return format!("{}{}{}", open, word, close);
    }
    let tail = TAIL.choose(rng).copied().unwrap_or(",");
    format!("{}{}", word, tail)
}

/// Number of words in a daily-challenge session (v0.5.0).
pub const DAILY_WORD_COUNT: usize = 25;

/// The daily challenge word list for `date` (v0.5.0).
///
/// Fully deterministic: the same date produces the same `DAILY_WORD_COUNT`
/// words on every machine, platform, and launch, so everyone races the same
/// text on a given day. Words always come from the Common pool with no
/// punctuation / number decoration — the challenge is identical regardless of
/// user config. (This determinism is also groundwork for the future LAN
/// multiplayer mode, where peers must agree on a shared word list.)
///
/// The generator is a self-contained SplitMix64 sequence seeded from the
/// date, rather than `rand`'s `StdRng` — `StdRng` is explicitly not
/// guaranteed to be reproducible across `rand` versions, and the day's
/// challenge must never change under a dependency bump.
pub fn daily_words(date: chrono::NaiveDate) -> Vec<String> {
    use chrono::Datelike;
    let pool = english::ENGLISH_1000;
    // Golden-ratio offset decorrelates consecutive days' seeds.
    let mut state = (date.num_days_from_ce() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    (0..DAILY_WORD_COUNT)
        .map(|_| {
            let index = (splitmix64(&mut state) % pool.len() as u64) as usize;
            pool[index].to_string()
        })
        .collect()
}

/// One step of the SplitMix64 PRNG (public-domain constants from Steele,
/// Lea & Flood). Statistical quality is far beyond what word-picking needs;
/// the point is bit-for-bit stability across platforms and releases.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Pick a random famous programming quote from the built-in list, split into
/// whitespace-separated words.
pub fn random_quote() -> Vec<String> {
    let mut rng = thread_rng();
    let quote = quotes::QUOTES.choose(&mut rng).copied().unwrap_or("");
    quote.split_whitespace().map(|s| s.to_string()).collect()
}

/// Pick a random short code snippet for the given language, split into
/// whitespace-separated words.
pub fn random_code_snippet(lang: CodeLang) -> Vec<String> {
    let mut rng = thread_rng();
    let pool: &[&str] = match lang {
        CodeLang::Rust => quotes::CODE_RUST,
        CodeLang::Python => quotes::CODE_PYTHON,
        CodeLang::JavaScript => quotes::CODE_JS,
        CodeLang::Go => quotes::CODE_GO,
        CodeLang::Java => quotes::CODE_JAVA,
        CodeLang::Sql => quotes::CODE_SQL,
        CodeLang::Shell => quotes::CODE_SHELL,
    };
    let snippet = pool.choose(&mut rng).copied().unwrap_or("");
    snippet.split_whitespace().map(|s| s.to_string()).collect()
}

/// Pick `count` programming-symbol tokens for the symbols-mode session.
pub fn random_symbol_tokens(count: usize) -> Vec<String> {
    symbols::random_symbol_tokens(count)
}

/// Source language for `Mode::Code`. Determines which snippet pool we sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeLang {
    Rust,
    Python,
    JavaScript,
    Go,
    Java,
    Sql,
    Shell,
}

/// Read a user-supplied text file and split it into words on whitespace.
/// Empty files / missing files surface as `Err`.
pub fn words_from_file(path: &str) -> anyhow::Result<Vec<String>> {
    let content = std::fs::read_to_string(path)?;
    Ok(content.split_whitespace().map(|s| s.to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_pool_returns_requested_count() {
        let words = random_words_from(25, WordPool::Common, WordDecor::default());
        assert_eq!(words.len(), 25);
        for w in &words {
            assert!(!w.is_empty());
        }
    }

    #[test]
    fn extended_pool_returns_requested_count() {
        let words = random_words_from(50, WordPool::Extended, WordDecor::default());
        assert_eq!(words.len(), 50);
    }

    #[test]
    fn extended_pool_is_larger_than_common() {
        assert!(english::ENGLISH_10000.len() > english::ENGLISH_1000.len() * 5);
        assert!(english::ENGLISH_10000.len() >= 10_000);
    }

    #[test]
    fn numbers_decoration_eventually_emits_a_number() {
        // With p=0.12 per slot and 1000 slots, a numeric token is virtually
        // guaranteed. Numbers are all-digit; words are all-alphabetic, so a
        // simple `chars().all(is_ascii_digit)` is a reliable detector.
        let decor = WordDecor {
            numbers: true,
            ..Default::default()
        };
        let words = random_words_from(1000, WordPool::Common, decor);
        let any_numeric = words
            .iter()
            .any(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_digit()));
        assert!(any_numeric, "numbers decoration produced no numeric tokens");
    }

    #[test]
    fn punctuation_decoration_eventually_emits_punctuation() {
        let decor = WordDecor {
            punctuation: true,
            ..Default::default()
        };
        let words = random_words_from(1000, WordPool::Common, decor);
        let any_punct = words.iter().any(|w| {
            w.chars()
                .any(|c| !c.is_ascii_alphanumeric() && !c.is_whitespace())
        });
        assert!(
            any_punct,
            "punctuation decoration produced no punctuation tokens"
        );
    }

    #[test]
    fn no_decoration_means_plain_alphabetic_words() {
        let decor = WordDecor::default();
        let words = random_words_from(200, WordPool::Common, decor);
        for w in &words {
            assert!(
                w.chars().all(|c| c.is_ascii_alphabetic()),
                "unexpected punctuation in plain word: {:?}",
                w
            );
        }
    }

    #[test]
    fn all_code_pools_are_non_empty() {
        assert!(!quotes::CODE_RUST.is_empty());
        assert!(!quotes::CODE_PYTHON.is_empty());
        assert!(!quotes::CODE_JS.is_empty());
        assert!(!quotes::CODE_GO.is_empty());
        assert!(!quotes::CODE_JAVA.is_empty());
        assert!(!quotes::CODE_SQL.is_empty());
        assert!(!quotes::CODE_SHELL.is_empty());
    }

    #[test]
    fn random_code_snippet_returns_tokens_for_every_lang() {
        for lang in [
            CodeLang::Rust,
            CodeLang::Python,
            CodeLang::JavaScript,
            CodeLang::Go,
            CodeLang::Java,
            CodeLang::Sql,
            CodeLang::Shell,
        ] {
            let tokens = random_code_snippet(lang);
            assert!(!tokens.is_empty(), "empty snippet for {:?}", lang);
        }
    }

    // ── v0.5.0: daily challenge ─────────────────────────────────────────────

    #[test]
    fn daily_words_is_deterministic_for_a_date() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        assert_eq!(daily_words(date), daily_words(date));
    }

    #[test]
    fn daily_words_differ_between_dates() {
        let a = daily_words(chrono::NaiveDate::from_ymd_opt(2026, 7, 10).unwrap());
        let b = daily_words(chrono::NaiveDate::from_ymd_opt(2026, 7, 11).unwrap());
        assert_ne!(a, b, "consecutive days produced identical challenges");
    }

    #[test]
    fn daily_words_returns_fixed_count_from_common_pool() {
        let words = daily_words(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(words.len(), DAILY_WORD_COUNT);
        for w in &words {
            assert!(
                english::ENGLISH_1000.contains(&w.as_str()),
                "{w:?} is not in the common pool"
            );
        }
    }

    /// Pin the generator's output for one known date. If this test ever
    /// fails, the day's challenge changed under someone's feet (generator or
    /// pool edit) — that is a compatibility break, not a refactor.
    #[test]
    fn daily_words_output_is_pinned_for_known_date() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let first_run = daily_words(date);
        assert_eq!(&first_run[..3], ["save", "used", "now"]);
        // Word picks are spread across the pool, not stuck on one index.
        let distinct: std::collections::HashSet<&String> = first_run.iter().collect();
        assert!(
            distinct.len() > DAILY_WORD_COUNT / 2,
            "suspiciously repetitive daily list: {first_run:?}"
        );
    }

    #[test]
    fn random_number_avoids_leading_zero_for_multi_digit() {
        // Generate many random numbers and ensure no multi-digit one starts with 0.
        let mut rng = rand::thread_rng();
        for _ in 0..200 {
            let n = random_number(&mut rng);
            if n.len() > 1 {
                assert_ne!(&n[0..1], "0", "number {} has leading zero", n);
            }
        }
    }
}
