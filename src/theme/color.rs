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
    if hex.len() != 6 {
        return Err(anyhow!(
            "hex color must be exactly 6 characters after '#', got '{}'",
            hex
        ));
    }
    let red = u8::from_str_radix(&hex[0..2], 16)
        .map_err(|_| anyhow!("invalid red byte in hex color '#{}'", hex))?;
    let green = u8::from_str_radix(&hex[2..4], 16)
        .map_err(|_| anyhow!("invalid green byte in hex color '#{}'", hex))?;
    let blue = u8::from_str_radix(&hex[4..6], 16)
        .map_err(|_| anyhow!("invalid blue byte in hex color '#{}'", hex))?;
    Ok(Color::Rgb(red, green, blue))
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

    #[test]
    fn trims_whitespace() {
        assert_eq!(parse_color("  cyan  ").unwrap(), Color::Cyan);
    }
}
