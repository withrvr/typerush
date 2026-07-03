//! User snippet library at `~/.typerush/snippets/`.
//!
//! Any `.txt` file in that directory becomes a selectable item in the main
//! menu under the "Snippets" group. The file's stem (filename without
//! extension) is shown as the row label; its contents are loaded as the word
//! source when selected.
//!
//! Discovery is **lazy and best-effort**: a missing directory or unreadable
//! file is silently treated as "no snippets". The menu falls back to the
//! built-in modes either way.

use std::path::{Path, PathBuf};

use crate::storage;

/// One discovered user snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    /// Display label — the filename without extension.
    pub name: String,
    /// Absolute path to the underlying `.txt` file.
    pub path: PathBuf,
}

/// Directory we scan for user snippets: `~/.typerush/snippets/`.
pub fn snippets_dir() -> PathBuf {
    storage::data_dir().join("snippets")
}

/// Return every `.txt` snippet in `~/.typerush/snippets/`, sorted by name.
///
/// Best-effort: a missing directory returns an empty vec; unreadable entries
/// are silently skipped. The result is stable across runs (alphabetical) so
/// menu indices don't shuffle between launches.
pub fn discover_snippets() -> Vec<Snippet> {
    discover_in(&snippets_dir())
}

/// Path-based variant of [`discover_snippets`]. Used by tests so they can
/// point at a temp directory without touching the user's real home.
pub fn discover_in(dir: &Path) -> Vec<Snippet> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut found: Vec<Snippet> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            // Case-insensitive `.txt` match so `NOTES.TXT` on
            // case-preserving filesystems (Windows, macOS default) is picked
            // up too — otherwise users on those platforms would drop a file
            // in the snippets dir and see nothing appear.
            let is_txt = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("txt"))
                .unwrap_or(false);
            if path.is_file() && is_txt {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())?;
                if name.is_empty() {
                    return None;
                }
                Some(Snippet { name, path })
            } else {
                None
            }
        })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn missing_directory_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let bogus = dir.path().join("does-not-exist");
        let snippets = discover_in(&bogus);
        assert!(snippets.is_empty());
    }

    #[test]
    fn discovers_txt_files_only() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("alpha.txt"), "one two three").unwrap();
        fs::write(dir.path().join("beta.txt"), "four five six").unwrap();
        // Non-txt files should be ignored entirely.
        fs::write(dir.path().join("notes.md"), "ignored").unwrap();
        fs::write(dir.path().join("README"), "ignored").unwrap();

        let snippets = discover_in(dir.path());
        assert_eq!(snippets.len(), 2);
        assert_eq!(snippets[0].name, "alpha");
        assert_eq!(snippets[1].name, "beta");
    }

    #[test]
    fn results_are_sorted_alphabetically() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("zebra.txt"), "z").unwrap();
        fs::write(dir.path().join("apple.txt"), "a").unwrap();
        fs::write(dir.path().join("mango.txt"), "m").unwrap();

        let snippets = discover_in(dir.path());
        let names: Vec<&str> = snippets.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["apple", "mango", "zebra"]);
    }

    #[test]
    fn empty_filename_excluded() {
        let dir = tempfile::tempdir().unwrap();
        // A bare ".txt" has an empty stem — skip it so it doesn't appear as
        // a nameless menu row.
        fs::write(dir.path().join(".txt"), "").unwrap();
        fs::write(dir.path().join("real.txt"), "hi").unwrap();
        let snippets = discover_in(dir.path());
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].name, "real");
    }

    /// Case-preserving filesystems (Windows, macOS default) present `.TXT`
    /// or `.Txt` as the stored extension. Users on those platforms would
    /// have their snippets silently dropped without a case-insensitive
    /// comparison.
    #[test]
    fn discovery_is_case_insensitive_for_txt() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Uppercase.TXT"), "hi").unwrap();
        fs::write(dir.path().join("Mixed.Txt"), "hi").unwrap();
        fs::write(dir.path().join("lower.txt"), "hi").unwrap();

        let snippets = discover_in(dir.path());
        assert_eq!(snippets.len(), 3);
        let names: Vec<&str> = snippets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Uppercase"));
        assert!(names.contains(&"Mixed"));
        assert!(names.contains(&"lower"));
    }
}
