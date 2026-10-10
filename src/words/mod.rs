//! Word sources — picks the text the user has to type.
//!
//! Each public function returns a `Vec<String>` of "words", split on
//! whitespace. The typing engine treats each entry as one token that needs to
//! be matched character-by-character, separated by a single space in the UI.

pub mod english;
pub mod quotes;
pub mod snippets;
pub mod symbols;

use std::io::Read;
use std::path::Path;

use anyhow::Context;
use rand::seq::IndexedRandom;
use rand::{Rng, RngExt};

/// Which English-word pool to draw from in [`random_words_from`].
///
/// `Common` is the original frequency-sorted pool that's been used since v0.1
/// — fastest to type, no surprises. `Extended` adds a much larger 10,000-word
/// dictionary on top, which trades some smoothness for vocabulary breadth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WordPool {
    /// The default pool: ~430 frequent words (`english::ENGLISH_COMMON`).
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
    let mut rng = rand::rng();
    let words: &[&str] = match pool {
        WordPool::Common => english::ENGLISH_COMMON,
        WordPool::Extended => english::ENGLISH_10000,
    };
    (0..count)
        .map(|_| {
            // ~12% of slots become numbers when the toggle is on — frequent
            // enough to be felt, rare enough not to dominate the session.
            if decor.numbers && rng.random_bool(0.12) {
                return random_number(&mut rng);
            }
            let base = words.choose(&mut rng).copied().unwrap_or("the").to_string();
            if decor.punctuation && rng.random_bool(0.25) {
                decorate_with_punctuation(&base, &mut rng)
            } else {
                base
            }
        })
        .collect()
}

/// Generate a short numeric literal (1–4 digits). Used by the numbers toggle.
fn random_number(rng: &mut impl Rng) -> String {
    // 1–4 digits, each length equally likely.
    let len = rng.random_range(1..=4);
    let mut s = String::with_capacity(len);
    for i in 0..len {
        // Avoid leading zero on multi-digit numbers — they look weird ("042").
        let digit = if i == 0 && len > 1 {
            rng.random_range(1..=9)
        } else {
            rng.random_range(0..=9)
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
    if rng.random_bool(0.2) {
        let pairs = [("\"", "\""), ("'", "'"), ("(", ")"), ("[", "]")];
        let (open, close) = pairs[rng.random_range(0..pairs.len())];
        return format!("{}{}{}", open, word, close);
    }
    let tail = TAIL.choose(rng).copied().unwrap_or(",");
    format!("{}{}", word, tail)
}

/// Pick a random famous programming quote from the built-in list, split into
/// whitespace-separated words.
pub fn random_quote() -> Vec<String> {
    let mut rng = rand::rng();
    let quote = quotes::QUOTES.choose(&mut rng).copied().unwrap_or("");
    quote.split_whitespace().map(|s| s.to_string()).collect()
}

/// Pick a random short code snippet for the given language, split into
/// whitespace-separated words.
pub fn random_code_snippet(lang: CodeLang) -> Vec<String> {
    let mut rng = rand::rng();
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

/// Largest custom file read: 1 MiB is about 170,000 words, far more than
/// one sitting, and keeps a huge file from taking all memory.
const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// Read a user-supplied text file and split it into words on whitespace.
/// A missing or unreadable file surfaces as `Err` naming the path — a stale
/// remembered file or deleted snippet otherwise shows a bare OS error.
///
/// Only regular files up to [`MAX_FILE_BYTES`] are read: a pipe or a device
/// (`--file /dev/zero`) would otherwise block forever or fill memory.
///
/// Characters that can't be typed are dropped (see `text::is_untypeable`):
/// a byte-order mark would otherwise make the first word impossible to get
/// right, and control characters drawn as-is are terminal escape sequences.
pub fn words_from_file(path: &Path) -> anyhow::Result<Vec<String>> {
    // The path is shown made safe: it can contain any character, a newline
    // or an escape sequence included.
    let shown = || crate::text::printable(&path.display().to_string()).into_owned();
    let metadata = std::fs::metadata(path).with_context(|| format!("can't read {}", shown()))?;
    if !metadata.is_file() {
        anyhow::bail!("{} is not a regular file", shown());
    }
    if metadata.len() > MAX_FILE_BYTES {
        anyhow::bail!("{} is too big to type (over 1 MiB)", shown());
    }
    // `take` still bounds the read if the file grows after the size check;
    // a character cut in half at that limit is dropped, not an error.
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(MAX_FILE_BYTES).read_to_end(&mut bytes))
        .with_context(|| format!("can't read {}", shown()))?;
    let content = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(err) if err.utf8_error().error_len().is_none() => {
            let valid = err.utf8_error().valid_up_to();
            String::from_utf8_lossy(&err.as_bytes()[..valid]).into_owned()
        }
        Err(_) => anyhow::bail!("{} is not UTF-8 text", shown()),
    };
    Ok(content
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|c| !crate::text::is_untypeable(*c))
                .collect::<String>()
        })
        .filter(|word| !word.is_empty())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Escape sequences and other control characters in a custom file never
    /// reach the typing screen; words made only of them disappear.
    #[test]
    fn file_words_drop_unsafe_characters() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.txt");
        std::fs::write(&path, "plain \u{1b}[2Jclear \u{7} na\u{202E}me").unwrap();
        assert_eq!(
            words_from_file(&path).unwrap(),
            ["plain", "[2Jclear", "name"]
        );
    }

    /// A pipe, device or directory is refused instead of blocking forever
    /// or filling memory, and so is a file over the size limit.
    #[test]
    fn file_words_refuse_non_files_and_huge_files() {
        let dir = tempfile::tempdir().unwrap();
        let err = words_from_file(dir.path()).unwrap_err();
        assert!(err.to_string().contains("not a regular file"), "{err}");
        let big = dir.path().join("big.txt");
        std::fs::write(&big, "a ".repeat(MAX_FILE_BYTES as usize / 2 + 1)).unwrap();
        let err = words_from_file(&big).unwrap_err();
        assert!(err.to_string().contains("too big"), "{err}");
        #[cfg(unix)]
        {
            let err = words_from_file(Path::new("/dev/zero")).unwrap_err();
            assert!(err.to_string().contains("not a regular file"), "{err}");
        }
    }

    /// A character cut in half at the end (as the size limit can do to a
    /// file that grows while it is read) is dropped; text that isn't UTF-8
    /// at all is refused by name.
    #[test]
    fn file_words_drop_a_cut_character_and_refuse_non_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let cut = dir.path().join("cut.txt");
        std::fs::write(&cut, b"hello wor\xE2\x82").unwrap();
        assert_eq!(words_from_file(&cut).unwrap(), ["hello", "wor"]);
        let latin1 = dir.path().join("latin1.txt");
        std::fs::write(&latin1, b"caf\xE9 au lait").unwrap();
        let err = words_from_file(&latin1).unwrap_err();
        assert!(err.to_string().contains("not UTF-8"), "{err}");
    }

    /// A UTF-8 byte-order mark (some Windows editors write one) must not
    /// become part of the first word.
    #[test]
    fn file_words_ignore_byte_order_mark() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bom.txt");
        std::fs::write(&path, "\u{FEFF}hello world").unwrap();
        assert_eq!(words_from_file(&path).unwrap(), ["hello", "world"]);
    }

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
        assert!(english::ENGLISH_10000.len() > english::ENGLISH_COMMON.len() * 5);
        assert_eq!(english::ENGLISH_10000.len(), 10_000);
    }

    /// Both pools are drawn from at random, in classrooms and on streamed
    /// screens: no profanity, slurs or sexual terms. Every common word is in
    /// the extended pool too, and neither pool repeats a word (a repeat would
    /// be picked twice as often).
    #[test]
    fn word_pools_are_family_friendly() {
        use std::collections::HashSet;
        const BLOCKED: &[&str] = &[
            "anal",
            "bastard",
            "bitch",
            "bitchiest",
            "bondage",
            "boob",
            "bullshit",
            "chinked",
            "clit",
            "cock",
            "condom",
            "cunt",
            "cunts",
            "cybersex",
            "dick",
            "dildo",
            "fag",
            "fagged",
            "fagot",
            "fetishist",
            "fuck",
            "fucks",
            "genitalia",
            "kinky",
            "milf",
            "morons",
            "nigger",
            "niggards",
            "nipples",
            "nudes",
            "nudity",
            "orgasms",
            "panties",
            "penis",
            "piss",
            "porn",
            "pussy",
            "rapist",
            "redneck",
            "retard",
            "scrotums",
            "sexiness",
            "sexting",
            "sexually",
            "sexy",
            "shit",
            "shittiest",
            "slut",
            "spank",
            "spanks",
            "testicle",
            "tit",
            "tits",
            "vagina",
            "vibrator",
            "wank",
            "wanking",
            "whore",
            "xxx",
        ];
        let common: HashSet<&str> = english::ENGLISH_COMMON.iter().copied().collect();
        let extended: HashSet<&str> = english::ENGLISH_10000.iter().copied().collect();
        assert_eq!(
            common.len(),
            english::ENGLISH_COMMON.len(),
            "repeat in common"
        );
        assert_eq!(
            extended.len(),
            english::ENGLISH_10000.len(),
            "repeat in extended"
        );
        assert!(common.is_subset(&extended));
        for word in BLOCKED {
            assert!(!extended.contains(word), "{word:?} is in a word pool");
        }
        assert!(extended
            .iter()
            .all(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase())));
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

    #[test]
    fn random_number_avoids_leading_zero_for_multi_digit() {
        // Generate many random numbers and ensure no multi-digit one starts with 0.
        let mut rng = rand::rng();
        for _ in 0..200 {
            let n = random_number(&mut rng);
            if n.len() > 1 {
                assert_ne!(&n[0..1], "0", "number {} has leading zero", n);
            }
        }
    }
}
