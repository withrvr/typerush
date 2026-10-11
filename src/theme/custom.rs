//! Your own themes: one TOML file each in `~/.typerush/themes/`, named after
//! the file (`ocean.toml` is `ocean`). A file sets any of the color slots
//! `[colors]` takes, on top of a built-in theme (`base = "dracula"`; `dark`
//! when it doesn't say):
//!
//! ```toml
//! base = "dark"
//! background = "#0B1D2A"
//! accent = "#4FC3F7"
//! ```
//!
//! They are read once at start and then picked like the built-in ones —
//! `theme = "ocean"`, `--theme ocean`, the Settings screen, `--list-themes`.
//! A file is held to the same limits as the config: a regular file, small,
//! and a name made of letters, digits, `-` and `_`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::{builtin, ThemePalette};
use crate::config::Colors;

/// Most custom themes read; more files are left out with a warning.
const MAX_THEMES: usize = 100;
/// Longest theme name, in characters.
const MAX_NAME: usize = 32;

/// `~/.typerush/themes`.
pub fn themes_dir() -> PathBuf {
    crate::storage::data_dir().join("themes")
}

/// A custom theme read from its file.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomTheme {
    /// The name it is picked by: the file name without `.toml`, lower case.
    pub name: String,
    pub palette: ThemePalette,
    pub path: PathBuf,
}

/// The custom themes installed for this run of TypeRush ([`install`]).
static INSTALLED: OnceLock<Vec<CustomTheme>> = OnceLock::new();

/// Make `themes` available to theme lookups for the rest of the run. Only
/// the first call counts (TypeRush calls it once, at start; tests never do,
/// so they only ever see the built-in themes).
pub fn install(themes: Vec<CustomTheme>) {
    let _ = INSTALLED.set(themes);
}

/// The installed custom themes (none until [`install`]).
pub fn installed() -> &'static [CustomTheme] {
    INSTALLED.get().map_or(&[], Vec::as_slice)
}

/// Every theme file in `dir` (`*.toml`, in name order), and a warning for
/// each one left out or with a color it couldn't use. A missing folder is
/// no themes and no warning.
pub fn discover_in(dir: &Path) -> (Vec<CustomTheme>, Vec<String>) {
    let mut themes = Vec::new();
    let mut warnings = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (themes, warnings);
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
        })
        .collect();
    // Name order, ignoring case, like the snippets.
    paths.sort_by_cached_key(|path| path.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    for path in paths {
        let shown = crate::text::printable(&path.display().to_string()).into_owned();
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let valid_name = !stem.is_empty()
            && stem.chars().count() <= MAX_NAME
            && stem
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid_name {
            warnings.push(format!(
                "theme {shown} left out: a theme's file name must be letters, digits, - or _ (up to {MAX_NAME})"
            ));
            continue;
        }
        if builtin::ALL.iter().any(|(name, _)| *name == stem) {
            warnings.push(format!(
                "theme {shown} left out: {stem} is a built-in theme's name"
            ));
            continue;
        }
        if themes.iter().any(|theme: &CustomTheme| theme.name == stem) {
            warnings.push(format!(
                "theme {shown} left out: a theme named {stem} is already read"
            ));
            continue;
        }
        if themes.len() == MAX_THEMES {
            warnings.push(format!(
                "more than {MAX_THEMES} themes: {shown} and later ones left out"
            ));
            break;
        }
        // Read like the config: a regular file of at most 1 MiB.
        let text = match crate::config::load::read_config(&path) {
            Ok(Some(text)) => text,
            Ok(None) => continue,
            Err(error) => {
                warnings.push(format!("theme {shown} left out: {error}"));
                continue;
            }
        };
        match parse(&text) {
            Ok((palette, problems)) => {
                warnings.extend(
                    problems
                        .into_iter()
                        .map(|problem| format!("theme {stem}: {problem}")),
                );
                themes.push(CustomTheme {
                    name: stem,
                    palette,
                    path,
                });
            }
            Err(error) => warnings.push(format!("theme {shown} left out: {error}")),
        }
    }
    (themes, warnings)
}

/// A theme file's palette, and a note for each color it couldn't use (that
/// slot keeps the base theme's color). An error when it isn't valid TOML,
/// names an unknown base, or has a setting that isn't a color slot.
pub fn parse(text: &str) -> Result<(ThemePalette, Vec<String>), String> {
    let mut table: toml::Table = text
        .parse()
        .map_err(|error| format!("isn't valid TOML: {error}"))?;
    let base = match table.remove("base") {
        None => builtin::DARK,
        Some(toml::Value::String(name)) => builtin::ALL
            .iter()
            .find(|(builtin, _)| builtin.eq_ignore_ascii_case(&name))
            .map(|(_, palette)| *palette)
            .ok_or_else(|| {
                format!(
                    "base '{}' isn't a built-in theme (try {})",
                    crate::text::printable(&name),
                    builtin::ALL
                        .iter()
                        .map(|(name, _)| *name)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?,
        Some(_) => return Err("base must be the name of a built-in theme".into()),
    };
    let colors: Colors = table.try_into().map_err(|error| format!("{error}"))?;
    let mut problems = Vec::new();
    let palette = crate::config::load::apply_color_overrides(base, &colors, &mut problems);
    Ok((palette, problems))
}

/// Text slots of `palette` that are hard to read on its background — under
/// 4.5:1, the level every built-in theme meets — as notes for
/// `--list-themes`. Nothing for a theme on the terminal's own background
/// (its contrast is the terminal's) or with non-RGB colors.
pub fn contrast_notes(palette: &ThemePalette) -> Vec<String> {
    let slots = [
        ("accent", palette.accent),
        ("secondary", palette.secondary),
        ("correct", palette.correct),
        ("incorrect", palette.incorrect),
        ("pending", palette.pending),
        ("mode_tag", palette.mode_tag),
        ("error", palette.error),
        ("neutral", palette.neutral),
    ];
    slots
        .into_iter()
        .filter_map(|(slot, color)| {
            let ratio = super::color::contrast(color, palette.background)?;
            (ratio < 4.5)
                .then(|| format!("{slot} is {ratio:.1}:1 on the background (4.5:1 reads well)"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn write(dir: &Path, name: &str, text: &str) {
        std::fs::write(dir.join(name), text).unwrap();
    }

    /// A theme starts from its base (dark when it doesn't say) and sets the
    /// slots it names; a bad color keeps the base's slot with a note.
    #[test]
    fn a_theme_file_sets_slots_on_its_base() {
        let (palette, problems) = parse("base = \"Dracula\"\naccent = \"#112233\"\n").unwrap();
        assert_eq!(palette.accent, Color::Rgb(0x11, 0x22, 0x33));
        assert_eq!(palette.background, builtin::DRACULA.background);
        assert!(problems.is_empty());

        let (palette, problems) = parse("background = \"#000000\"\nerror = \"nope\"").unwrap();
        assert_eq!(palette.correct, builtin::DARK.correct);
        assert_eq!(palette.background, Color::Rgb(0, 0, 0));
        assert_eq!(palette.error, builtin::DARK.error);
        assert!(problems[0].contains("'error'"), "{problems:?}");

        for (bad, why) in [
            ("base = \"solarized\"", "isn't a built-in theme"),
            ("base = 3", "base must be"),
            ("acent = \"red\"", "unknown field"),
            ("accent = ", "isn't valid TOML"),
        ] {
            let error = parse(bad).unwrap_err();
            assert!(error.contains(why), "{bad}: {error}");
        }
    }

    /// The folder's *.toml files become themes named after them, in name
    /// order; bad names, built-in names, broken files and non-files are left
    /// out with a warning; a missing folder is just no themes.
    #[test]
    fn themes_are_discovered_with_warnings() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(discover_in(&dir.path().join("none")), (vec![], vec![]));
        write(dir.path(), "Ocean.toml", "accent = \"#4FC3F7\"");
        write(dir.path(), "forest_2.TOML", "base = \"light\"");
        write(dir.path(), "notes.txt", "accent = \"red\"");
        write(dir.path(), "has space.toml", "");
        write(dir.path(), "monokai.toml", "");
        write(dir.path(), "broken.toml", "accent = ");
        std::fs::create_dir(dir.path().join("folder.toml")).unwrap();
        let (themes, warnings) = discover_in(dir.path());
        let names: Vec<&str> = themes.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["forest_2", "ocean"]);
        assert_eq!(themes[0].palette.background, builtin::LIGHT.background);
        assert_eq!(themes[1].palette.accent, Color::Rgb(0x4F, 0xC3, 0xF7));
        let all = warnings.join("\n");
        for reason in [
            "letters, digits",
            "built-in theme's name",
            "isn't valid TOML",
            "not a regular file",
        ] {
            assert!(all.contains(reason), "{reason}: {all}");
        }
        assert_eq!(warnings.len(), 4, "{all}");
    }

    /// Hard-to-read text slots are noted; the built-in themes have none.
    #[test]
    fn contrast_notes_flag_hard_to_read_slots() {
        for (_, palette) in builtin::ALL {
            assert!(contrast_notes(palette).is_empty());
        }
        let (palette, _) = parse("base = \"light\"\npending = \"#EEEEEE\"").unwrap();
        let notes = contrast_notes(&palette);
        assert_eq!(notes.len(), 1);
        assert!(notes[0].starts_with("pending is 1.1:1"), "{notes:?}");
    }
}
