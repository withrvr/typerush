//! Word sources — picks the text the user has to type.
//!
//! Each public function returns a `Vec<String>` of "words", split on
//! whitespace. The typing engine treats each entry as one token that needs to
//! be matched character-by-character, separated by a single space in the UI.

pub mod english;
pub mod quotes;

use rand::seq::SliceRandom;
use rand::thread_rng;

/// Pick `count` random English words from the built-in 1000-word pool.
/// Words may repeat — the pool is sampled with replacement.
pub fn random_words(count: usize) -> Vec<String> {
    let mut rng = thread_rng();
    let pool = english::ENGLISH_1000;
    (0..count)
        .map(|_| pool.choose(&mut rng).copied().unwrap_or("the").to_string())
        .collect()
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
}

/// Read a user-supplied text file and split it into words on whitespace.
/// Empty files / missing files surface as `Err`.
pub fn words_from_file(path: &str) -> anyhow::Result<Vec<String>> {
    let content = std::fs::read_to_string(path)?;
    Ok(content.split_whitespace().map(|s| s.to_string()).collect())
}
