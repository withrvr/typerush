//! Built-in theme catalogue.
//!
//! Add a new theme by declaring a `pub const` and appending it to [`ALL`].
//! That's all the wiring required — the loader and `--list-themes` pick it up
//! automatically.

use ratatui::style::Color;

use super::ThemePalette;

/// The original TypeRush look — cyan/green/yellow on a black terminal.
/// This is the default when no `theme = ...` is set in the config.
///
/// `background = Color::Reset` keeps the user's terminal background untouched,
/// so anyone who picks "dark" on e.g. a solarized-dark terminal still gets
/// their terminal's chrome — no surprise color change.
pub const DARK: ThemePalette = ThemePalette {
    accent: Color::Cyan,
    secondary: Color::Yellow,
    correct: Color::Green,
    incorrect: Color::Red,
    pending: Color::DarkGray,
    extra: Color::Red,
    mode_tag: Color::Magenta,
    error: Color::Red,
    neutral: Color::White,
    background: Color::Reset,
};

/// Softer palette intended for terminals with a light background.
/// Forces an off-white background so the theme actually feels "light" even
/// when the user's terminal itself is dark. Foreground colors are
/// deliberately darkened compared to the dark-theme equivalents — anything
/// too pale would wash out against the white background.
pub const LIGHT: ThemePalette = ThemePalette {
    accent: Color::Rgb(0x01, 0x6F, 0xA0), // deeper cyan — more contrast on white
    secondary: Color::Rgb(0xA0, 0x6E, 0x00), // darker amber
    correct: Color::Rgb(0x3F, 0x82, 0x3F), // darker green
    incorrect: Color::Rgb(0xC5, 0x3B, 0x30), // darker red
    pending: Color::Rgb(0x55, 0x57, 0x5C), // mid-dark gray — readable, not loud
    extra: Color::Rgb(0xC5, 0x3B, 0x30),
    mode_tag: Color::Rgb(0x88, 0x1F, 0x88),   // deeper purple
    error: Color::Rgb(0xB0, 0x0F, 0x3C),      // darker error red
    neutral: Color::Rgb(0x20, 0x22, 0x28),    // near-black body text
    background: Color::Rgb(0xFA, 0xFA, 0xFA), // off-white
};

/// Classic Monokai — pink/green/yellow on the canonical warm dark backdrop.
pub const MONOKAI: ThemePalette = ThemePalette {
    accent: Color::Rgb(0x66, 0xD9, 0xEF),    // monokai cyan
    secondary: Color::Rgb(0xE6, 0xDB, 0x74), // monokai yellow
    correct: Color::Rgb(0xA6, 0xE2, 0x2E),   // monokai green
    incorrect: Color::Rgb(0xF9, 0x26, 0x72), // monokai pink
    pending: Color::Rgb(0x75, 0x71, 0x5E),   // dim gray-brown
    extra: Color::Rgb(0xFD, 0x97, 0x1F),     // orange
    mode_tag: Color::Rgb(0xAE, 0x81, 0xFF),  // purple
    error: Color::Rgb(0xF9, 0x26, 0x72),
    neutral: Color::Rgb(0xF8, 0xF8, 0xF2), // monokai foreground
    background: Color::Rgb(0x27, 0x28, 0x22), // canonical monokai background
};

/// Dracula — purple/pink/cyan on the canonical `#282a36` background.
pub const DRACULA: ThemePalette = ThemePalette {
    accent: Color::Rgb(0x8B, 0xE9, 0xFD),    // dracula cyan
    secondary: Color::Rgb(0xF1, 0xFA, 0x8C), // dracula yellow
    correct: Color::Rgb(0x50, 0xFA, 0x7B),   // dracula green
    incorrect: Color::Rgb(0xFF, 0x55, 0x55), // dracula red
    pending: Color::Rgb(0x62, 0x72, 0xA4),   // dracula comment
    extra: Color::Rgb(0xFF, 0xB8, 0x6C),     // dracula orange
    mode_tag: Color::Rgb(0xFF, 0x79, 0xC6),  // dracula pink
    error: Color::Rgb(0xFF, 0x55, 0x55),
    neutral: Color::Rgb(0xF8, 0xF8, 0xF2), // dracula foreground
    background: Color::Rgb(0x28, 0x2A, 0x36), // canonical dracula background
};

/// `(name, palette)` for every built-in theme.
///
/// Adding a new theme = append a row here. The loader and `--list-themes`
/// both iterate this list.
pub const ALL: &[(&str, ThemePalette)] = &[
    ("dark", DARK),
    ("light", LIGHT),
    ("monokai", MONOKAI),
    ("dracula", DRACULA),
];

/// Look up a theme by name (case-insensitive). Returns `None` for unknown names.
pub fn by_name(name: &str) -> Option<ThemePalette> {
    let lower = name.to_lowercase();
    ALL.iter()
        .find(|(theme_name, _)| *theme_name == lower)
        .map(|(_, palette)| *palette)
}

/// All built-in theme names, in registration order.
pub fn names() -> Vec<&'static str> {
    ALL.iter().map(|(name, _)| *name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_theme() {
        assert!(by_name("monokai").is_some());
        assert!(by_name("Monokai").is_some()); // case-insensitive
        assert!(by_name("DRACULA").is_some());
    }

    #[test]
    fn lookup_unknown_theme_returns_none() {
        assert!(by_name("nonexistent").is_none());
        assert!(by_name("").is_none());
    }

    #[test]
    fn names_contains_all_four_builtins() {
        let names = names();
        assert_eq!(names.len(), 4);
        assert!(names.contains(&"dark"));
        assert!(names.contains(&"light"));
        assert!(names.contains(&"monokai"));
        assert!(names.contains(&"dracula"));
    }

    #[test]
    fn default_palette_is_dark() {
        let default_palette: ThemePalette = ThemePalette::default();
        assert_eq!(default_palette, DARK);
    }

    /// Dark theme must not paint a background — the user's terminal bg shines
    /// through, preserving the v0.1 look on every terminal.
    #[test]
    fn dark_background_is_reset() {
        assert_eq!(DARK.background, Color::Reset);
    }

    /// Light/monokai/dracula must paint their own background so the theme
    /// looks the same regardless of the host terminal.
    #[test]
    fn non_dark_themes_paint_explicit_backgrounds() {
        assert_ne!(LIGHT.background, Color::Reset);
        assert_eq!(MONOKAI.background, Color::Rgb(0x27, 0x28, 0x22));
        assert_eq!(DRACULA.background, Color::Rgb(0x28, 0x2A, 0x36));
    }
}
