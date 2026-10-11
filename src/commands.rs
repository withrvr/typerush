//! Subcommands: things TypeRush does from the shell without opening the
//! app (`typerush export`, …). Each prints its result and returns; errors
//! are printed by `main` like any other.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::config::edit::{self, Source};
use crate::config::load::{self, CliOverrides};
use crate::{storage, text};

/// Subcommands of `typerush`.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Write your session history as CSV, to FILE or to the terminal.
    Export {
        /// File to write (replaced if it exists). Leave out to print.
        file: Option<PathBuf>,
    },
    /// See or change ~/.typerush/config.toml without opening it.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

/// `typerush config …`.
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Print where the config file is.
    Path,
    /// Print every setting with the value TypeRush uses, as TOML.
    Show,
    /// Print the value TypeRush uses for one setting (e.g. defaults.mode).
    Get {
        /// Setting name, as `show` lists them.
        key: String,
    },
    /// Change one setting; the file is created if needed and its comments
    /// kept.
    Set {
        /// Setting name, as `show` lists them.
        key: String,
        /// New value.
        value: String,
    },
    /// Put one setting back to its default, or with no name all of them
    /// (the file is moved aside, not deleted).
    Reset {
        /// Setting name; leave out to reset everything.
        key: Option<String>,
    },
    /// Write a starter config.toml: every setting at its default, explained.
    Init,
}

/// The files a subcommand works on: the real ones in `~/.typerush`, or
/// temporary ones in tests.
pub struct Files<'a> {
    pub stats: &'a Path,
    pub config: &'a Path,
}

/// Run `command`, printing results to `out` (the terminal) and notes to
/// `err`.
pub fn run(
    command: &Command,
    files: &Files,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<()> {
    match command {
        Command::Export { file } => export(files.stats, file.as_deref(), out, err),
        Command::Config { action } => config(action, files.config, out, err),
    }
}

/// `typerush config …` on the config file at `path`.
fn config(
    action: &ConfigAction,
    path: &Path,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<()> {
    let shown = text::printable(&path.display().to_string()).into_owned();
    match action {
        // A real path, so scripts can open it (only control and bidi
        // characters replaced).
        ConfigAction::Path => writeln!(out, "{}", text::path_text(&path.display().to_string()))?,
        ConfigAction::Show => show(path, &shown, out)?,
        ConfigAction::Get { key } => {
            let key = edit::key(key)?;
            let lookups = effective(path, err)?;
            let lookup = lookups.iter().find(|lookup| lookup.key == key);
            if let Some(lookup) = lookup {
                writeln!(out, "{}", lookup.value.plain())?;
                if let Source::Invalid { found, problem } = &lookup.source {
                    writeln!(
                        err,
                        "note: the file's {} = {found} isn't used ({problem}); this is the default",
                        key.name
                    )?;
                }
            }
        }
        ConfigAction::Set { key, value } => {
            let setting = edit::set(path, key, value)?;
            writeln!(out, "{key} = {}", setting.toml())?;
            note_warnings(path, err)?;
        }
        ConfigAction::Reset { key: Some(key) } => {
            edit::reset(path, key)?;
            let lookups = effective(path, err)?;
            if let Some(lookup) = lookups.iter().find(|lookup| lookup.key.name == key) {
                writeln!(out, "{key} = {}  # default", lookup.value.toml())?;
            }
        }
        ConfigAction::Reset { key: None } => match edit::reset_all(path)? {
            Some(backup) => writeln!(
                err,
                "every setting is back to its default; your old file is at {}",
                text::printable(&backup.display().to_string())
            )?,
            None => writeln!(
                err,
                "no config file at {shown}: every setting is already its default"
            )?,
        },
        ConfigAction::Init => {
            edit::init(path)?;
            writeln!(err, "wrote {shown}")?;
        }
    }
    Ok(())
}

/// Every setting as TypeRush uses it. A file that can't be read or parsed
/// is ignored by the app (every default applies): say so on `err`.
fn effective(path: &Path, err: &mut impl Write) -> Result<Vec<edit::Lookup>> {
    let config = match edit::read(path) {
        Ok(config) => config.unwrap_or_default(),
        Err(error) => {
            for line in format!("{error:#}").lines() {
                writeln!(err, "note: {}", text::printable(line))?;
            }
            writeln!(
                err,
                "note: until that's fixed, every setting is its default"
            )?;
            Default::default()
        }
    };
    Ok(edit::effective(&config))
}

/// `typerush config show`: every setting as TOML (`theme = "dark"`), each
/// default or ignored value marked in a comment, under the file's path.
fn show(path: &Path, shown: &str, out: &mut impl Write) -> Result<()> {
    let mut notes = Vec::new();
    let lookups = effective(path, &mut notes)?;
    match std::fs::symlink_metadata(path) {
        Ok(_) => writeln!(out, "# {shown}")?,
        Err(_) => writeln!(
            out,
            "# {shown} (no file yet: `typerush config init` writes one)"
        )?,
    }
    for line in String::from_utf8_lossy(&notes).lines() {
        writeln!(out, "# {line}")?;
    }
    let width = edit::KEYS
        .iter()
        .map(|key| key.name.len())
        .max()
        .unwrap_or(0);
    for lookup in &lookups {
        let line = format!("{:<width$} = {}", lookup.key.name, lookup.value.toml());
        match &lookup.source {
            Source::File => writeln!(out, "{line}")?,
            Source::Default => writeln!(out, "{line:<44}# default")?,
            Source::Invalid { found, problem } => writeln!(
                out,
                "{line:<44}# default: the file's {found} isn't used ({problem})"
            )?,
        }
    }
    Ok(())
}

/// After a change, any problem left in the file (from the loader, which
/// the app runs at start), so it isn't a surprise next launch.
fn note_warnings(path: &Path, err: &mut impl Write) -> Result<()> {
    let (_, warnings) = load::load_from(path, CliOverrides::default());
    for warning in warnings {
        for line in warning.lines() {
            writeln!(err, "note: {}", text::printable(line))?;
        }
    }
    Ok(())
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

    /// Files for a test: the history at `stats`, no config.
    fn files(stats: &Path) -> Files<'_> {
        Files {
            stats,
            config: Path::new("/nonexistent/config.toml"),
        }
    }

    /// Run `typerush config <args>` on the config at `path`: (stdout, stderr).
    fn config_cli(path: &Path, args: &[&str]) -> Result<(String, String)> {
        use clap::Parser;
        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: Command,
        }
        let cli = Cli::try_parse_from(["typerush", "config"].iter().chain(args))?;
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let files = Files {
            stats: Path::new("/nonexistent/stats.json"),
            config: path,
        };
        run(&cli.command, &files, &mut out, &mut err)?;
        Ok((String::from_utf8(out)?, String::from_utf8(err)?))
    }

    /// The config subcommands end to end: init, set, get, show, reset one,
    /// reset all.
    #[test]
    fn config_subcommands() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".typerush").join("config.toml");
        let run = |args: &[&str]| config_cli(&path, args);

        let (out, _) = run(&["path"]).unwrap();
        assert_eq!(out.trim(), path.display().to_string());
        let (out, _) = run(&["show"]).unwrap();
        assert!(out.contains("(no file yet"), "{out}");
        assert!(out.contains("theme                 = \"dark\""), "{out}");
        assert!(out.lines().filter(|l| l.ends_with("# default")).count() == edit::KEYS.len());

        let (_, err) = run(&["init"]).unwrap();
        assert!(err.starts_with("wrote "), "{err}");
        assert!(run(&["init"])
            .unwrap_err()
            .to_string()
            .contains("already exists"));

        let (out, _) = run(&["set", "theme", "Monokai"]).unwrap();
        assert_eq!(out, "theme = \"monokai\"\n");
        let (out, _) = run(&["get", "theme"]).unwrap();
        assert_eq!(out, "monokai\n");
        // A color left unset follows the theme.
        let (out, _) = run(&["get", "colors.background"]).unwrap();
        assert_eq!(out, "#272822\n");
        let (out, _) = run(&["show"]).unwrap();
        let theme = out.lines().find(|l| l.starts_with("theme ")).unwrap();
        assert_eq!(theme, "theme                 = \"monokai\"");
        assert!(run(&["set", "theme", "nope"]).is_err());
        assert!(run(&["get", "nope"])
            .unwrap_err()
            .to_string()
            .contains("unknown setting"));

        let (out, _) = run(&["reset", "theme"]).unwrap();
        assert_eq!(out, "theme = \"dark\"  # default\n");
        let (_, err) = run(&["reset"]).unwrap();
        assert!(err.contains("your old file is at"), "{err}");
        assert!(!path.exists());
        let (_, err) = run(&["reset"]).unwrap();
        assert!(err.contains("no config file"), "{err}");
    }

    /// An ignored value is reported where it is read (`get`, `show`), and a
    /// file the app can't parse is called out: every value is a default then.
    #[test]
    fn config_reports_ignored_values_and_broken_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[defaults]\ntime_seconds = 0\n").unwrap();
        let (out, err) = config_cli(&path, &["get", "defaults.time_seconds"]).unwrap();
        assert_eq!(out, "15\n");
        assert!(err.contains("time_seconds = 0 isn't used"), "{err}");
        let (out, _) = config_cli(&path, &["show"]).unwrap();
        assert!(
            out.contains("# default: the file's 0 isn't used (must be between 1 and 3600)"),
            "{out}"
        );

        std::fs::write(&path, "theme = 5\n").unwrap();
        let (out, err) = config_cli(&path, &["get", "theme"]).unwrap();
        assert_eq!(out, "dark\n");
        assert!(err.contains("can't parse"), "{err}");
        assert!(err.contains("every setting is its default"), "{err}");
        let (out, _) = config_cli(&path, &["show"]).unwrap();
        assert!(out.contains("# note: can't parse"), "{out}");
        // `set` on valid TOML still works, and says what is still wrong.
        let (_, err) = config_cli(&path, &["set", "defaults.mode", "zen"]).unwrap();
        assert!(err.contains("note: could not parse"), "{err}");
    }

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
        run(&command, &files(&stats_file), &mut out, &mut err).unwrap();
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.starts_with(storage::CSV_HEADER), "{printed}");
        assert!(printed.contains(",words-25,50.00,"), "{printed}");

        let target = dir.path().join("out").join("stats.csv");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let command = Command::Export {
            file: Some(target.clone()),
        };
        run(&command, &files(&stats_file), &mut out, &mut err).unwrap();
        assert!(out.is_empty());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), printed);
        let note = String::from_utf8(err).unwrap();
        assert!(note.starts_with("exported 1 session to "), "{note}");

        let mut out = Vec::new();
        let command = Command::Export { file: None };
        let none = dir.path().join("none.json");
        run(&command, &files(&none), &mut out, &mut Vec::new()).unwrap();
        assert_eq!(out, format!("{}\n", storage::CSV_HEADER).into_bytes());
    }
}
