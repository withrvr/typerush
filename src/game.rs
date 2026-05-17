//! Word-matching engine.
//!
//! Given a `target` word (what the user is supposed to type) and a `typed`
//! string (what they've actually typed so far), this module produces a list of
//! `(char, CharState)` pairs that drive the colored UI feedback.
//!
//! The renderer uses this output verbatim — one styled span per character.

/// State of a single character on the screen.
///
/// Maps directly to a color in the typing UI:
/// - `Correct`   → green
/// - `Incorrect` → red
/// - `Pending`   → dim gray (not yet typed)
/// - `Extra`     → red + underline (typed past the end of the target word)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CharState {
    /// The character was typed correctly.
    Correct,
    /// The user typed a different character at this position.
    Incorrect,
    /// The user has not typed this character yet.
    Pending,
    /// The user typed past the end of the target word — these chars are surplus.
    Extra,
}

/// Compare `target` against `typed` character-by-character and return the
/// per-character state list used by the renderer.
///
/// The returned vec always contains at least `target.chars().count()` items;
/// any characters the user typed beyond that length are appended as `Extra`.
///
/// # Examples
/// ```ignore
/// // typing "rust" perfectly:
/// get_char_states("rust", "rust") // 4 × Correct
///
/// // one wrong letter:
/// get_char_states("rust", "rast") // Correct, Incorrect, Correct, Correct
///
/// // halfway through:
/// get_char_states("rust", "ru")   // Correct, Correct, Pending, Pending
///
/// // overshoot:
/// get_char_states("hi", "hiya")   // Correct, Correct, Extra('y'), Extra('a')
/// ```
pub fn get_char_states(target: &str, typed: &str) -> Vec<(char, CharState)> {
    let target_chars: Vec<char> = target.chars().collect();
    let typed_chars: Vec<char> = typed.chars().collect();
    let mut result = Vec::with_capacity(target_chars.len() + typed_chars.len());

    // Pass 1: walk the target word. For each target char, decide whether the
    // user has typed the right one, the wrong one, or nothing yet.
    for (index, &target_char) in target_chars.iter().enumerate() {
        let state = match typed_chars.get(index) {
            Some(&typed_char) if typed_char == target_char => CharState::Correct,
            Some(_) => CharState::Incorrect,
            None => CharState::Pending,
        };
        result.push((target_char, state));
    }

    // Pass 2: anything the user typed past the target length is "extra".
    for &extra_char in typed_chars.iter().skip(target_chars.len()) {
        result.push((extra_char, CharState::Extra));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_correct() {
        let states = get_char_states("rust", "rust");
        assert_eq!(states.len(), 4);
        assert!(states.iter().all(|(_, s)| *s == CharState::Correct));
    }

    #[test]
    fn one_wrong() {
        let states = get_char_states("rust", "rast");
        assert_eq!(states[1].1, CharState::Incorrect);
    }

    #[test]
    fn pending_chars() {
        let states = get_char_states("rust", "ru");
        assert_eq!(states[2].1, CharState::Pending);
        assert_eq!(states[3].1, CharState::Pending);
    }

    #[test]
    fn extra_chars() {
        let states = get_char_states("hi", "hiya");
        assert_eq!(states.len(), 4);
        assert_eq!(states[2].1, CharState::Extra);
        assert_eq!(states[3].1, CharState::Extra);
    }
}
