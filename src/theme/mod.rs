//! Theming — the color palette every UI module reads from.
//!
//! A `ThemePalette` is a flat struct of named color slots. Built-in palettes
//! (dark / light / monokai / dracula) live in [`builtin`]; the user can also
//! pick one by name in `~/.typerush/config.toml` and optionally override
//! individual slots.
//!
//! All UI code reads from the active `ThemePalette` rather than hardcoding
//! `Color::*`, so adding a new theme is a matter of dropping a `ThemePalette`
//! constant into [`builtin`] and listing it in [`builtin::ALL`].

pub mod builtin;
pub mod color;

use ratatui::style::Color;

/// Every themable color slot in the UI.
///
/// Each field maps to one or more concrete rendering decisions. Keeping the
/// list deliberately small (≈8 slots) avoids overwhelming users who just want
/// to tweak a color or two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemePalette {
    /// Primary brand color — banner, cursor, gauge fill, "time" number, titles.
    pub accent: Color,
    /// Secondary accent — used for emphasized numbers (WPM) and section titles.
    pub secondary: Color,
    /// Correctly-typed characters and the accuracy percentage.
    pub correct: Color,
    /// Wrongly-typed characters.
    pub incorrect: Color,
    /// Not-yet-typed characters and muted footer / label text.
    pub pending: Color,
    /// Surplus characters typed past the end of a word.
    pub extra: Color,
    /// `[mode]` badge in the header.
    pub mode_tag: Color,
    /// Error modal border / red highlights.
    pub error: Color,
    /// Emphasized neutral text — session counts, char totals on results.
    /// Maps to the terminal's natural foreground on each theme (white on dark,
    /// near-black on light) so it stays readable on every background.
    pub neutral: Color,
    /// Whole-screen background fill. Set to `Color::Reset` to leave the
    /// terminal's native background alone (used by the `dark` theme so it
    /// behaves identically to v0.1). Other themes paint their canonical
    /// background so the theme looks the same regardless of which terminal
    /// the user is on.
    pub background: Color,
}

impl ThemePalette {
    /// Apply a list of (slot-name, color-string) overrides on top of a base palette.
    ///
    /// Unknown slot names are silently ignored — they would have failed earlier
    /// during config deserialization in any sane setup, but we never want a
    /// typo'd override to crash the app.
    pub fn with_overrides(mut self, overrides: &[(&str, Color)]) -> Self {
        for (slot, color) in overrides {
            match *slot {
                "accent" => self.accent = *color,
                "secondary" => self.secondary = *color,
                "correct" => self.correct = *color,
                "incorrect" => self.incorrect = *color,
                "pending" => self.pending = *color,
                "extra" => self.extra = *color,
                "mode_tag" => self.mode_tag = *color,
                "error" => self.error = *color,
                "neutral" => self.neutral = *color,
                "background" => self.background = *color,
                _ => {}
            }
        }
        self
    }
}

impl Default for ThemePalette {
    /// The default theme is `dark` — the original TypeRush look.
    fn default() -> Self {
        builtin::DARK
    }
}
