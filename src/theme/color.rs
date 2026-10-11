//! Parse user-supplied color strings into ratatui `Color` values.
//!
//! Two accepted forms:
//!   - Hex: `#RRGGBB` (case-insensitive, leading `#` required).
//!   - Named: any of the standard 16 ANSI palette names (`cyan`, `darkgray`,
//!     `red`, …). Case-insensitive. Underscores accepted (`dark_gray`).
//!
//! Anything else returns `Err`. The error is human-readable so we can surface
//! it in the one-time error modal on a bad config.

use anyhow::{anyhow, Result};
use ratatui::style::Color;

/// Parse one color string into a `Color`.
///
/// # Examples
/// ```ignore
/// parse_color("#FF00FF").unwrap();   // Rgb(255, 0, 255)
/// parse_color("cyan").unwrap();      // Color::Cyan
/// parse_color("dark_gray").unwrap(); // Color::DarkGray
/// parse_color("not-a-color");        // Err
/// ```
pub fn parse_color(input: &str) -> Result<Color> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("empty color string"));
    }
    if let Some(rest) = trimmed.strip_prefix('#') {
        return parse_hex(rest);
    }
    parse_named(trimmed)
}

fn parse_hex(hex: &str) -> Result<Color> {
    // Checked as characters first: slicing by byte would panic inside a
    // multi-byte character, and `from_str_radix` alone would accept a sign.
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(anyhow!(
            "hex color must be '#' and exactly 6 hex digits (0-9, a-f), got '#{}'",
            hex
        ));
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
    Ok(Color::Rgb(byte(0), byte(2), byte(4)))
}

fn parse_named(name: &str) -> Result<Color> {
    let normalized: String = name
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_' && *c != '-')
        .collect::<String>()
        .to_lowercase();
    match normalized.as_str() {
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "gray" | "grey" => Ok(Color::Gray),
        "darkgray" | "darkgrey" => Ok(Color::DarkGray),
        "lightred" => Ok(Color::LightRed),
        "lightgreen" => Ok(Color::LightGreen),
        "lightyellow" => Ok(Color::LightYellow),
        "lightblue" => Ok(Color::LightBlue),
        "lightmagenta" => Ok(Color::LightMagenta),
        "lightcyan" => Ok(Color::LightCyan),
        "white" => Ok(Color::White),
        "reset" | "default" => Ok(Color::Reset),
        _ => Err(anyhow!("unknown color name '{}'", name)),
    }
}

/// WCAG contrast ratio between two RGB colors (1.0–21.0); `None` when
/// either isn't RGB — a terminal palette color has no fixed value.
pub fn contrast(a: Color, b: Color) -> Option<f64> {
    fn luminance(color: Color) -> Option<f64> {
        let Color::Rgb(r, g, b) = color else {
            return None;
        };
        let channel = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        Some(0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b))
    }
    let (a, b) = (luminance(a)?, luminance(b)?);
    Some((a.max(b) + 0.05) / (a.min(b) + 0.05))
}

/// `color` written the way [`parse_color`] reads it: `#RRGGBB`, an ANSI
/// name (`dark_gray`) or `reset` — for printing config values.
pub fn color_name(color: Color) -> String {
    let name = match color {
        Color::Rgb(red, green, blue) => return format!("#{red:02X}{green:02X}{blue:02X}"),
        Color::Black => "black",
        Color::Red => "red",
        Color::Green => "green",
        Color::Yellow => "yellow",
        Color::Blue => "blue",
        Color::Magenta => "magenta",
        Color::Cyan => "cyan",
        Color::Gray => "gray",
        Color::DarkGray => "dark_gray",
        Color::LightRed => "light_red",
        Color::LightGreen => "light_green",
        Color::LightYellow => "light_yellow",
        Color::LightBlue => "light_blue",
        Color::LightMagenta => "light_magenta",
        Color::LightCyan => "light_cyan",
        Color::White => "white",
        Color::Reset => "reset",
        // Never produced by `parse_color` or a built-in theme.
        Color::Indexed(index) => return format!("indexed {index}"),
    };
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_color() {
        assert_eq!(parse_color("#FF00FF").unwrap(), Color::Rgb(255, 0, 255));
        assert_eq!(parse_color("#000000").unwrap(), Color::Rgb(0, 0, 0));
        assert_eq!(parse_color("#ffffff").unwrap(), Color::Rgb(255, 255, 255));
    }

    #[test]
    fn parses_named_color_case_insensitive() {
        assert_eq!(parse_color("cyan").unwrap(), Color::Cyan);
        assert_eq!(parse_color("CYAN").unwrap(), Color::Cyan);
        assert_eq!(parse_color("Cyan").unwrap(), Color::Cyan);
    }

    #[test]
    fn parses_compound_named_colors() {
        assert_eq!(parse_color("dark_gray").unwrap(), Color::DarkGray);
        assert_eq!(parse_color("dark-gray").unwrap(), Color::DarkGray);
        assert_eq!(parse_color("darkgray").unwrap(), Color::DarkGray);
        assert_eq!(parse_color("light_red").unwrap(), Color::LightRed);
    }

    #[test]
    fn rejects_short_hex() {
        assert!(parse_color("#FFF").is_err());
        assert!(parse_color("#1234567").is_err());
    }

    #[test]
    fn rejects_unknown_name() {
        assert!(parse_color("burnt-orange").is_err());
        assert!(parse_color("").is_err());
    }

    #[test]
    fn rejects_invalid_hex_chars() {
        assert!(parse_color("#GGGGGG").is_err());
    }

    /// Regression: a six-*byte* string that isn't six hex digits used to be
    /// sliced by byte, panicking inside a multi-byte character (`#aé€`), and
    /// a sign was read as part of the number (`#+F+F+F`).
    #[test]
    fn rejects_non_hex_bytes_without_panicking() {
        for bad in ["#aé€", "#ééé", "#+F+F+F", "#-1-1-1", "# FFFFF"] {
            assert!(parse_color(bad).is_err(), "{bad:?}");
        }
    }

    /// Every color a theme or the parser makes is written back in a form
    /// the parser reads as the same color.
    #[test]
    fn color_names_read_back() {
        let mut colors = vec![Color::Rgb(0x0A, 0xBC, 0xFF), Color::Reset];
        for (_, palette) in crate::theme::builtin::ALL {
            colors.extend([palette.accent, palette.pending, palette.background]);
        }
        colors.extend([
            Color::DarkGray,
            Color::LightMagenta,
            Color::Gray,
            Color::White,
        ]);
        for color in colors {
            assert_eq!(parse_color(&color_name(color)).unwrap(), color, "{color:?}");
        }
        assert_eq!(color_name(Color::Rgb(255, 0, 15)), "#FF000F");
    }

    #[test]
    fn trims_whitespace() {
        assert_eq!(parse_color("  cyan  ").unwrap(), Color::Cyan);
    }
}
