//! Programming-symbols word source.
//!
//! Generates short "tokens" composed primarily of programming punctuation —
//! the keys most typists hit hardest under pressure. The intent is targeted
//! practice on `(){};=>!&|+-*/[]<>`, not realistic code.
//!
//! Each call to [`random_symbol_tokens`] returns a fresh sequence of tokens
//! drawn from a small hand-picked pool plus a handful of randomly composed
//! tokens. Tokens are deliberately 2–6 characters long so the user gets
//! frequent feedback (one space, one accuracy check) per token.

use rand::seq::SliceRandom;
use rand::Rng;

/// Hand-curated tokens that mimic real punctuation sequences from code.
/// Heavy on the keys typists usually struggle with.
const COMMON_TOKENS: &[&str] = &[
    "()", "(){}", "[];", "{};", "=>", "->", "::", "&&", "||", "==", "!=", "<=", ">=", "<<", ">>",
    "++", "--", "+=", "-=", "*=", "/=", "?.", "?:", "??", "()=>", "(a,b)", "{x:y}", "[i++]",
    "x++;", "y--;", "++i", "--i", "!x", "&x", "*x", "x;", "x|y", "x&y", "x^y", "~x", "x;y", ";;",
    "()=>{}", "if{}", "for(;;)", "do{}", "[]", "{}", "<>", "<T>", "</>", "/>", "<!--", "-->", "*/",
    "/*", "//", "**", "/=", "%=", "//=", "==>", "<==", "/**/", "x.y", "a.b", "a::b", "x[0]",
    "y[1]", "p->q", "*ptr", "&ref", "x?y:z",
];

/// Pool of single characters used to build random tokens.
const SYMBOL_CHARS: &[char] = &[
    '(', ')', '{', '}', '[', ']', '<', '>', ';', ':', ',', '.', '!', '?', '&', '|', '=', '+', '-',
    '*', '/', '%', '^', '~', '@', '#', '$', '_',
];

/// Pick `count` whitespace-separated tokens for a symbols-mode session.
///
/// The result is biased toward the curated `COMMON_TOKENS` (≈70%) with the
/// remainder generated from random 2–5-character sequences of `SYMBOL_CHARS`.
/// All tokens contain only printable, easily-typable punctuation — no
/// whitespace inside a token, so the typing engine treats each as one word.
pub fn random_symbol_tokens(count: usize) -> Vec<String> {
    let mut rng = rand::thread_rng();
    let mut tokens = Vec::with_capacity(count);
    for _ in 0..count {
        // 70/30 split between curated tokens and random sequences keeps the
        // practice realistic while still drilling unusual key combinations.
        if rng.gen_bool(0.70) {
            let pick = COMMON_TOKENS.choose(&mut rng).copied().unwrap_or("()");
            tokens.push(pick.to_string());
        } else {
            let len = rng.gen_range(2..=5);
            let mut s = String::with_capacity(len);
            for _ in 0..len {
                if let Some(c) = SYMBOL_CHARS.choose(&mut rng) {
                    s.push(*c);
                }
            }
            tokens.push(s);
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_requested_count() {
        let tokens = random_symbol_tokens(25);
        assert_eq!(tokens.len(), 25);
    }

    #[test]
    fn tokens_contain_no_whitespace() {
        let tokens = random_symbol_tokens(50);
        for t in &tokens {
            assert!(!t.is_empty(), "empty token");
            assert!(
                t.chars().all(|c| !c.is_whitespace()),
                "whitespace in token: {:?}",
                t
            );
        }
    }

    #[test]
    fn zero_count_returns_empty() {
        let tokens = random_symbol_tokens(0);
        assert!(tokens.is_empty());
    }

    #[test]
    fn tokens_are_punctuation_or_alphanumeric_only() {
        // The curated pool contains a few letters inside expressions like
        // `()=>{}` and `(a,b)` — that's intentional. We just want to make
        // sure no whitespace and no unprintable chars slip in.
        let tokens = random_symbol_tokens(100);
        for t in &tokens {
            assert!(
                t.chars().all(|c| !c.is_control() && !c.is_whitespace()),
                "bad char in token: {:?}",
                t
            );
        }
    }
}
