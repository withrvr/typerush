//! User snippet library at `~/.typerush/snippets/`.
//!
//! Every `.txt` file in that directory (any capitalisation of the extension)
//! becomes an option in the main menu's `custom` row, labelled with the file
//! name minus the extension; picking it types the file.
//!
//! Discovery is best-effort: a missing directory or an unreadable entry just
//! means fewer snippets, never an error.

use std::path::{Path, PathBuf};

use crate::storage;

/// One discovered user snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    /// The file name without `.txt`. The menu label starts from this (see
    /// `app::build_menu`, which keeps every label distinct).
    pub name: String,
    /// Path to the `.txt` file.
    pub path: PathBuf,
    /// `path` canonicalized once at discovery (`None` if that failed), so a
    /// menu rebuild only canonicalizes the remembered file, not every snippet.
    pub canonical_path: Option<PathBuf>,
}

/// Directory we scan for user snippets: `~/.typerush/snippets/`.
pub fn snippets_dir() -> PathBuf {
    storage::data_dir().join("snippets")
}

/// Every `.txt` snippet in `~/.typerush/snippets/`, in name order.
pub fn discover_snippets() -> Vec<Snippet> {
    discover_in(&snippets_dir())
}

/// Every `.txt` snippet in `dir`, sorted by name ignoring case, so the menu
/// order is the same on every launch and on every platform.
pub fn discover_in(dir: &Path) -> Vec<Snippet> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut found: Vec<Snippet> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            // `NOTES.TXT` counts too: Windows and macOS keep whatever case
            // the file was created with.
            let is_txt = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"));
            if !is_txt || !path.is_file() {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            if name.is_empty() {
                return None;
            }
            let canonical_path = std::fs::canonicalize(&path).ok();
            Some(Snippet {
                name,
                path,
                canonical_path,
            })
        })
        .collect();

    // "apple, mango, Zebra" rather than byte order's "Zebra, apple, mango";
    // ties are broken by exact name, then path, so the order never varies.
    found.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path.cmp(&b.path))
    });

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

    #[test]
    fn order_ignores_case() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["Zebra.txt", "apple.txt", "mango.txt"] {
            fs::write(dir.path().join(name), "x").unwrap();
        }
        let names: Vec<String> = discover_in(dir.path())
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert_eq!(names, ["apple", "mango", "Zebra"]);
    }

    /// Two files that differ only in the case of `.txt` can coexist on a
    /// case-sensitive filesystem (Linux): both are found, in a stable order.
    /// (The menu gives them distinct labels.)
    #[cfg(target_os = "linux")]
    #[test]
    fn same_name_different_extension_case_both_found() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("notes.txt"), "a").unwrap();
        fs::write(dir.path().join("notes.TXT"), "b").unwrap();
        let found = discover_in(dir.path());
        let files: Vec<_> = found.iter().map(|s| s.path.file_name().unwrap()).collect();
        assert_eq!(files, ["notes.TXT", "notes.txt"]);
        assert!(found.iter().all(|s| s.name == "notes"));
    }

    #[test]
    fn canonical_path_is_recorded() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "x").unwrap();
        let snippet = &discover_in(dir.path())[0];
        assert_eq!(
            snippet.canonical_path,
            Some(fs::canonicalize(&snippet.path).unwrap())
        );
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
