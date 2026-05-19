# Changelog

All notable changes to TypeRush are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

_No unreleased changes yet._

---

## [0.3.0] — smarter stats

### Added

- **Per-key accuracy heatmap.** TypeRush now tracks how often you type each
  key correctly vs. incorrectly. After enough sessions the Stats screen shows
  a "key accuracy" panel with up to 5 of your worst keys, including hit/total
  counts and a colour-coded accuracy percentage (red < 80%, amber < 93%,
  green otherwise). The panel shows "no key data yet (keep typing!)" until at
  least one key has been pressed 3 or more times. Per-key data is stored in
  `stats.json` alongside each session record.
- **Per-mode personal bests.** The Results screen now shows the personal best
  specifically for the mode you just finished (e.g. `time-30s best`) instead
  of the all-time best across all modes. A `★ new best!` badge fires whenever
  you beat your previous record for that mode, including on your first session.
  Zen-mode results display `— (zen not saved)` since Zen sessions are not saved.
- **Daily streak counter.** The Stats screen summary now shows how many
  consecutive calendar days (in local time) you have at least one session on.
  Streak counts backward from today (or yesterday — the streak is still active
  if you haven't typed yet today). Displayed as "1 day", "N days", or "—" when
  the streak is broken.
- **Average WPM over the last 7 and 30 days.** Two rolling-window WPM
  averages appear in the Stats screen summary. Both show "—" when no sessions
  fall within the window.
- **Backward-compatible stats file.** New `key_hits` / `key_misses` fields
  use `#[serde(default)]` so existing `~/.typerush/stats.json` records without
  them load cleanly — no migration needed.

### Changed

- **Stats screen layout redesigned.** The summary card and the new key-accuracy
  heatmap sit side by side in a two-column top row, followed by the WPM
  sparkline and the recent-sessions table. The sparkline is slightly shorter
  (5 rows instead of 7) so the full screen still fits in a 24-row terminal.
- **Results screen body expanded** from `Constraint::Length(9)` to
  `Constraint::Length(10)` to accommodate the mode-specific best row.

### Compatibility

- Existing CLI flags, modes, config, and the stats file format are all
  unchanged. Old `stats.json` records load correctly and contribute to
  global stats; they just won't provide per-key heatmap data.

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
- Test count grew from 4 to 48 — color parsing, config deserialization,
  theme resolution, override application, CLI precedence, menu-row
  matching, render-level background paint, custom-mode auto-finish,
  Ctrl+H/W/Backspace dispatch.

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
- **Ctrl+Backspace now actually deletes the current word.** Most terminals
  send `Ctrl+Backspace` as a literal `^H` byte — crossterm reports it as
  `Char('h') + CTRL`, not as `Backspace + CTRL` — so the old keymap was
  silently dropping it into the "ignore control chords" branch. The
  keymap now also recognises `Ctrl+W` (Unix "kill word" muscle memory).

### Compatibility
- Existing CLI flags (`--time`, `--words`, `--quote`, `--code`, `--zen`,
  `--file`) are unchanged.
- `~/.typerush/stats.json` format is unchanged — old session history loads
  exactly as before.

---

## [0.1.1] — initial release

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
- `ARCHITECTURE.md`, `ROADMAP.md`, `CONTRIBUTING.md`, `CHANGELOG.md`.
- Module-level (`//!`) and item-level (`///`) doc comments throughout the
  codebase.
- GitHub issue & pull-request templates.
- Release workflow that builds binaries for Linux / macOS / Windows on every
  `v*` tag.

### Fixed
- Cursor no longer flickers / shifts the line horizontally at the end of a
  word. The cursor is now steady and always reserves a fixed-width slot.

### Changed
- README rewritten to be more approachable. Technical content moved into
  dedicated docs.
- Demo GIF uses absolute GitHub raw URL for correct display on crates.io.

[Unreleased]: https://github.com/withrvr/typerush/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/withrvr/typerush/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/withrvr/typerush/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/withrvr/typerush/releases/tag/v0.1.1
