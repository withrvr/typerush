//! Subcommands: things TypeRush does from the shell without opening the
//! app (`typerush export`, …). Each prints its result and returns; errors
//! are printed by `main` like any other.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::{storage, text};

/// Subcommands of `typerush`.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Write your session history as CSV, to FILE or to the terminal.
    Export {
        /// File to write (replaced if it exists). Leave out to print.
        file: Option<PathBuf>,
    },
}

/// Run `command` against the history at `stats_file`, printing to `out`
/// (the terminal) and notes to `err`.
pub fn run(
    command: &Command,
    stats_file: &Path,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<()> {
    match command {
        Command::Export { file } => export(stats_file, file.as_deref(), out, err),
    }
}

/// `typerush export [FILE]`.
fn export(
    stats_file: &Path,
    file: Option<&Path>,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<()> {
    let sessions = storage::load_sessions_from_path(stats_file).with_context(|| {
        format!(
            "can't read {}",
            text::printable(&stats_file.display().to_string())
        )
    })?;
    let Some(file) = file else {
        out.write_all(storage::to_csv(&sessions).as_bytes())?;
        return Ok(());
    };
    let shown = text::printable(&file.display().to_string()).into_owned();
    storage::export_csv(file, &sessions).with_context(|| format!("can't write {shown}"))?;
    writeln!(err, "exported {} to {shown}", sessions_text(sessions.len()))?;
    Ok(())
}

/// "1 session", "42 sessions".
pub fn sessions_text(count: usize) -> String {
    match count {
        1 => "1 session".to_string(),
        n => format!("{n} sessions"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `export` prints CSV, or writes it to a file and says so; a missing
    /// history exports just the header.
    #[test]
    fn export_prints_or_writes_csv() {
        let dir = tempfile::tempdir().unwrap();
        let stats_file = dir.path().join("stats.json");
        let record = storage::SessionRecord {
            wpm: 50.0,
            mode: "words-25".into(),
            ..Default::default()
        };
        storage::save_session_to_path(&stats_file, &record).unwrap();

        let (mut out, mut err) = (Vec::new(), Vec::new());
        let command = Command::Export { file: None };
        run(&command, &stats_file, &mut out, &mut err).unwrap();
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.starts_with(storage::CSV_HEADER), "{printed}");
        assert!(printed.contains(",words-25,50.00,"), "{printed}");

        let target = dir.path().join("out").join("stats.csv");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let command = Command::Export {
            file: Some(target.clone()),
        };
        run(&command, &stats_file, &mut out, &mut err).unwrap();
        assert!(out.is_empty());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), printed);
        let note = String::from_utf8(err).unwrap();
        assert!(note.starts_with("exported 1 session to "), "{note}");

        let mut out = Vec::new();
        let command = Command::Export { file: None };
        let none = dir.path().join("none.json");
        run(&command, &none, &mut out, &mut Vec::new()).unwrap();
        assert_eq!(out, format!("{}\n", storage::CSV_HEADER).into_bytes());
    }
}
