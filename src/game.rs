#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CharState {
    Correct,
    Incorrect,
    Pending,
    Extra,
}

pub fn get_char_states(target: &str, typed: &str) -> Vec<(char, CharState)> {
    let target_chars: Vec<char> = target.chars().collect();
    let typed_chars: Vec<char> = typed.chars().collect();
    let mut result = Vec::with_capacity(target_chars.len() + typed_chars.len());

    for (i, &tc) in target_chars.iter().enumerate() {
        let state = match typed_chars.get(i) {
            Some(&uc) if uc == tc => CharState::Correct,
            Some(_) => CharState::Incorrect,
            None => CharState::Pending,
        };
        result.push((tc, state));
    }

    for &uc in typed_chars.iter().skip(target_chars.len()) {
        result.push((uc, CharState::Extra));
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
