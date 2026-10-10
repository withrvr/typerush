//! Display helpers for text that comes from the user's files: snippet and
//! file names, the paths in error messages, and the words of a custom file.

use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// The bidirectional controls: they make text display in a different order
/// than it is stored, so a name can look like something it isn't.
fn is_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
    )
}

/// Characters that must never be drawn as-is: control characters (a Linux
/// file name may contain ESC, which starts a terminal escape sequence) and
/// bidirectional controls.
pub fn is_unsafe(c: char) -> bool {
    c.is_control() || is_bidi_control(c)
}

/// Characters with no visible form of their own: Unicode's
/// Default_Ignorable_Code_Point set (the byte-order mark some Windows editors
/// write at the start of a UTF-8 file, zero-width spaces, joiners and
/// non-joiners, word joiners, variation selectors, soft hyphens, Hangul
/// fillers, tags, the bidi controls, …) plus the interlinear-annotation
/// controls.
pub fn is_invisible(c: char) -> bool {
    is_bidi_control(c)
        || matches!(
            c,
            '\u{00AD}'
                | '\u{034F}'
                | '\u{115F}'..='\u{1160}'
                | '\u{17B4}'..='\u{17B5}'
                | '\u{180B}'..='\u{180F}'
                | '\u{200B}'..='\u{200D}'
                | '\u{2060}'..='\u{206F}'
                | '\u{3164}'
                | '\u{FE00}'..='\u{FE0F}'
                | '\u{FEFF}'
                | '\u{FFA0}'
                | '\u{FFF0}'..='\u{FFFB}'
                | '\u{1BCA0}'..='\u{1BCA3}'
                | '\u{1D173}'..='\u{1D17A}'
                | '\u{E0000}'..='\u{E0FFF}'
        )
}

/// Characters a typing target must not contain: control characters and
/// [`is_invisible`] ones. The typing screen draws one cell per character and
/// zero-width ones aren't drawn at all, so an invisible character would be
/// one the user can't see but has to type (a byte-order mark used to make a
/// file's first word impossible). That includes the zero-width non-joiner a
/// Persian keyboard types on Shift+Space: such words are typed without it.
pub fn is_untypeable(c: char) -> bool {
    c.is_control() || is_invisible(c)
}

/// `text` with every character matching `replace` shown as `?`.
fn replace_matching(text: &str, replace: fn(char) -> bool) -> Cow<'_, str> {
    if !text.contains(replace) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.chars()
            .map(|c| if replace(c) { '?' } else { c })
            .collect(),
    )
}

/// `text` with every [`is_untypeable`] character shown as `?`: nothing that
/// could disturb the terminal, and nothing invisible — so two names that
/// differ only by a hidden character also look different.
pub fn printable(text: &str) -> Cow<'_, str> {
    replace_matching(text, is_untypeable)
}

/// `text` with only [`is_unsafe`] characters shown as `?` — for output that
/// must still name a real file (`--list-snippets` paths), where a soft hyphen
/// or zero-width space is part of the name.
pub fn path_text(text: &str) -> Cow<'_, str> {
    replace_matching(text, is_unsafe)
}

/// `text` cut to at most `max_width` terminal cells by replacing its middle
/// with `…` (`quarterly-r…nal-draft-q1`). The end usually tells similar names
/// apart, so it stays visible. Width is measured in cells (a CJK character
/// is two), and cuts fall on grapheme boundaries so an accent never loses its
/// letter — macOS often stores names decomposed, as `e` + U+0301.
pub fn shorten(text: &str, max_width: usize) -> Cow<'_, str> {
    // Measured grapheme by grapheme — how the terminal buffer draws it, and
    // the same rule the cut below uses, so head and tail can't overlap.
    if text
        .graphemes(true)
        .map(UnicodeWidthStr::width)
        .sum::<usize>()
        <= max_width
    {
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
        assert_eq!(printable("a\u{200F}b"), "a?b");
        assert!(matches!(printable("plain name"), Cow::Borrowed(_)));
    }

    #[test]
    fn invisible_characters_are_untypeable() {
        for c in [
            '\u{FEFF}',
            '\u{200B}',
            '\u{200C}',
            '\u{200D}',
            '\u{FE0F}',
            '\u{00AD}',
            '\u{034F}',
            '\u{3164}',
            '\u{E0041}',
            '\u{FFF9}',
            '\u{1b}',
            '\u{202E}',
        ] {
            assert!(is_untypeable(c), "{c:?}");
        }
        for c in ['a', 'é', ' ', '漢', '(', '…', '\u{0301}', 'ی'] {
            assert!(!is_untypeable(c), "{c:?}");
        }
    }

    #[test]
    fn path_text_keeps_invisible_but_real_characters() {
        assert_eq!(path_text("re\u{00AD}port"), "re\u{00AD}port");
        assert_eq!(path_text("a\u{1b}b\nc"), "a?b?c");
        assert_eq!(path_text("report\u{202E}txt.exe"), "report?txt.exe");
    }

    /// A name that differs from another only by a hidden character must
    /// look different.
    #[test]
    fn printable_reveals_invisible_characters() {
        assert_eq!(printable("\u{200B}notes"), "?notes");
        assert_eq!(printable("\u{FEFF}notes"), "?notes");
        assert_eq!(printable("no\u{200D}tes"), "no?tes");
        assert_eq!(printable("notes\u{3164}"), "notes?");
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
