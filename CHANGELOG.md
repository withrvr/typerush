# Changelog

All notable changes to TypeRush are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

_No unreleased changes yet._

---

## [0.2.0] — customization

### Added
- **Configuration file** at `~/.typerush/config.toml`. Entirely optional — a
  missing or malformed file silently falls back to defaults. See
  `config.example.toml` and `docs/USAGE.md` for the full schema.
- **Built-in themes**: `dark` (the existing look, still the default), `light`,
  `monokai`, `dracula`. Pick one with `theme = "monokai"` in the config.
  Each theme paints its own canonical background — light is genuinely light
  even on a dark terminal, monokai and dracula look like the real schemes
  regardless of host terminal. Dark uses `Color::Reset` so the user's
  terminal background still shines through.
- **Per-slot color overrides** on top of any built-in theme. Ten themable
  slots (`accent`, `secondary`, `correct`, `incorrect`, `pending`, `extra`,
  `mode_tag`, `error`, `neutral`, `background`) cover every UI element.
- **Color parser** for `#RRGGBB` hex strings and the 16 ANSI color names
  (case-insensitive, `_` / `-` separators tolerated).
- **`--theme <name>` CLI flag** for one-shot overrides. Wins over the config.
- **`--list-themes` CLI flag** prints every built-in theme name and exits —
  useful for shell-completion scripts.
- **Default mode + per-mode defaults** in the config — pre-selects the
  matching menu row at startup.
- **`docs/DEVELOPMENT.md`** — practical local-dev workflow (running with
  CLI args, sandboxed config testing via `HOME` redirect, `cargo-watch`
  patterns for TUI apps, recommended two-terminal loop).
- Test count grew from 4 to 43 — color parsing, config deserialization,
  theme resolution, override application, CLI precedence, menu-row
  matching, render-level background paint, custom-mode auto-finish.

### Changed
- Zen mode now desaturates to theme-aware `pending` / `neutral` colors instead
  of hardcoded grays, keeping the screen readable on light backgrounds.
- Help overlay lists the config file path alongside the stats file path.
- Menu screen gained 2 rows of top padding so the TYPERUSH banner no longer
  hugs the terminal's title bar / tab strip.
- Light theme foreground palette darkened across every slot — the previous
  values were carried over from the dark theme and washed out on white.
  All slots now land at ≥4.5:1 contrast against the `#FAFAFA` background.

### Fixed
- Cursor no longer flickers / shifts the line horizontally at the end of a
  word. The cursor is now steady and always reserves a fixed-width slot.
  _(Carried over from the unreleased section of v0.1.)_
- **Custom-file mode (`--file <path>`) now auto-finishes** when the user
  types the last word, matching the documented behavior in
  `docs/USAGE.md`. Previously the session sat waiting for `Esc`.
- **Progress-gauge timer label** is now explicitly styled (`secondary` +
  bold) instead of falling through to whatever default ratatui picked.
  Readable on both the filled and unfilled portions of the bar across
  every built-in theme.
- **Plain (unstyled) text on the light theme** — menu list items, the
  stats table's date column, and plain help-overlay lines — now render
  in `theme.neutral` rather than terminal-default foreground. Previously
  they were near-white on white when running the light theme on a dark
  terminal.

### Compatibility
- Existing CLI flags (`--time`, `--words`, `--quote`, `--code`, `--zen`,
  `--file`) are unchanged.
- `~/.typerush/stats.json` format is unchanged — old session history loads
  exactly as before.

---

## [0.1.0] — initial release

### Added
- Cross-platform TUI built with `ratatui` + `crossterm`.
- Six game modes: Time, Words, Quote, Code (Rust/Python/JS), Zen, Custom file.
- Live WPM + accuracy display updated every 100ms.
- Character-by-character color feedback.
- Persistent stats in `~/.typerush/stats.json`.
- Stats history screen with sparkline trend.
- CLI flags to skip the menu (`--time`, `--words`, `--quote`, `--code`,
  `--zen`, `--file`).
- Floating `?` help overlay.
- Unit tests for the matcher.
- GitHub Actions CI: build + test + clippy on Linux, macOS, Windows.

[Unreleased]: https://github.com/withrvr/typerush/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/withrvr/typerush/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/withrvr/typerush/releases/tag/v0.1.0
