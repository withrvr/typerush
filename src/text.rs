//! Display helpers for text that comes from the user's files: snippet and
//! file names, the paths in error messages, and the words of a custom file.

use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Characters that must never be drawn as-is: control characters (a Linux
/// file name may contain ESC, which starts a terminal escape sequence) and
/// the bidirectional overrides that make text display in a different order
/// than it is stored.
pub fn is_unsafe(c: char) -> bool {
    c.is_control() || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// `text` with every [`is_unsafe`] character shown as `?`.
pub fn printable(text: &str) -> Cow<'_, str> {
    if !text.contains(is_unsafe) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.chars()
            .map(|c| if is_unsafe(c) { '?' } else { c })
            .collect(),
    )
}

/// `text` cut to at most `max_width` terminal cells by replacing its middle
/// with `…` (`quarterly-r…nal-draft-q1`). The end usually tells similar names
/// apart, so it stays visible. Width is measured in cells (a CJK character
/// is two), and cuts fall on grapheme boundaries so an accent never loses its
/// letter — macOS often stores names decomposed, as `e` + U+0301.
pub fn shorten(text: &str, max_width: usize) -> Cow<'_, str> {
    if text.width() <= max_width {
        return Cow::Borrowed(text);
    }
    // One cell for the ellipsis; the rest split between start and end.
    let budget = max_width.saturating_sub(1);
    let head_budget = budget / 2;
    let (mut head_end, mut head_width) = (0, 0);
    for (index, grapheme) in text.grapheme_indices(true) {
        let width = grapheme.width();
        if head_width + width > head_budget {
            break;
        }
        head_width += width;
        head_end = index + grapheme.len();
    }
    // The text is wider than `max_width`, so head and tail can't overlap.
    let tail_budget = budget - head_width;
    let (mut tail_start, mut tail_width) = (text.len(), 0);
    for (index, grapheme) in text.grapheme_indices(true).rev() {
        let width = grapheme.width();
        if tail_width + width > tail_budget {
            break;
        }
        tail_width += width;
        tail_start = index;
    }
    Cow::Owned(format!("{}…{}", &text[..head_end], &text[tail_start..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_replaces_control_and_bidi_characters() {
        assert_eq!(printable("a\u{1b}[31mb\tc"), "a?[31mb?c");
        assert_eq!(printable("x\u{202E}txt.exe"), "x?txt.exe");
        assert!(matches!(printable("plain name"), Cow::Borrowed(_)));
    }

    #[test]
    fn short_text_is_untouched() {
        assert_eq!(shorten("rust", 24), "rust");
        let exactly = "a".repeat(24);
        assert_eq!(shorten(&exactly, 24), exactly);
    }

    /// Long names keep their start and end: the end is what usually tells
    /// similar files apart (and where a " (2)" lives).
    #[test]
    fn long_text_is_shortened_in_the_middle() {
        let text = shorten("rust-borrow-checker-error-messages (2)", 24);
        assert_eq!(text.width(), 24);
        assert!(text.starts_with("rust-borrow"), "{text}");
        assert!(text.ends_with("messages (2)"), "{text}");
        assert_ne!(
            shorten("quarterly-report-final-draft-q1", 24),
            shorten("quarterly-report-final-draft-q2", 24)
        );
    }

    /// Wide characters count two cells each: 20 CJK characters are 40 cells,
    /// over the limit even though they are fewer than 24 characters.
    #[test]
    fn wide_characters_are_measured_in_cells() {
        let label = "漢".repeat(20);
        let text = shorten(&label, 24);
        assert!(text.contains('…'));
        assert!(text.width() <= 24);
    }

    /// A decomposed accent (`e` + U+0301) is never split from its letter.
    #[test]
    fn cuts_fall_on_grapheme_boundaries() {
        let label = "e\u{301}".repeat(30);
        let text = shorten(&label, 24);
        assert!(text.width() <= 24);
        let (head, tail) = text.split_once('…').unwrap();
        for part in [head, tail] {
            assert!(!part.starts_with('\u{301}'), "orphaned accent in {part:?}");
            assert_eq!(part.matches('e').count(), part.matches('\u{301}').count());
        }
    }
}
