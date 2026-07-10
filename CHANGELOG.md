# Changelog

All notable changes to TypeRush are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

_No unreleased changes yet._

---

## [0.5.0] — quality of life

### Added

- **Pause / resume mid-session.** `Ctrl+P` freezes the session: the clock,
  WPM, and time-mode countdown all stop, a "paused" modal appears, and every
  key except `Ctrl+P` (resume), `Esc` (end session), and `Ctrl+C` (quit) is
  ignored so you can't type into a paused session by accident. Paused time
  never counts toward your stats.
- **Word-for-word session replay.** Every saved session now records its full
  target word list. Press `p` on the Results screen to immediately re-run the
  words you just typed, or select any of your last 10 sessions on the Stats
  screen (`↑/↓`, then `Enter`) to replay it under its original mode — a
  `time-30s` replay gets the same 30-second clock over the same words.
  Sessions recorded before v0.5.0 show a friendly "no replay data" message.
- **CSV export.** `typerush --export-csv [path]` writes the whole session
  history as CSV — to the given file (confirmation on stderr) or to stdout
  when the path is omitted, so it pipes cleanly. Columns: RFC 3339
  timestamp, mode, wpm, accuracy, word_count, correct_chars, total_chars,
  duration_secs.
- **Daily challenge.** A new `Daily challenge` menu row (and `--daily` flag)
  runs 25 words picked deterministically from the date — everyone on the
  planet types the same list on the same day, regardless of pool or
  decoration settings. Sessions save as `daily-YYYY-MM-DD`, so each day has
  its own personal best. The generator is a self-contained SplitMix64
  sequence, guaranteed not to change under dependency upgrades (and a
  building block for the future LAN race mode, which needs peers to agree on
  a shared word list).
- **`typerush config` subcommands.** `config get <key>`, `config set <key>
  <value>`, `config show`, `config reset`, and `config path` edit
  `~/.typerush/config.toml` without opening an editor. Keys are whitelisted
  (`theme`, `defaults.*`, `words.*`, `colors.*`) and values are validated
  with the exact rules the app applies at startup, so a successful `set` can
  never produce a config that warns at launch. Edits preserve every comment
  in the file; `reset` backs the file up to `config.toml.bak` before
  removing it.
- **In-app Settings screen.** A new `Settings` row under the menu's More
  section opens a three-row screen: a theme picker and a default-mode picker
  (`←/→` to cycle — changes apply to the live UI instantly and persist to
  the config), plus a reset-to-defaults row that backs up and clears the
  config file.
- **`typerush --init-config`.** Writes the fully commented starter config
  (the shipped `config.example.toml`) to `~/.typerush/config.toml` and
  refuses to overwrite an existing file.

### Changed

- The typing-screen footer now reads `ctrl+p pause · ctrl+r restart · esc
  menu · ctrl+c quit` (the old `? help` hint was misleading — `?` is a
  typeable character during a session).
- The Stats screen's recent-sessions table is selectable (highlight follows
  `↑/↓`), and its footer documents the replay keys.
- The menu's More section now holds Stats, Settings, and Quit.
- Internal: the shared reset tail of `start_game` is factored into
  `reset_session_counters`, reused by the new `start_replay`.

### Fixed

- Nothing — no open defects were carried into this release.

### Compatibility

- All existing CLI flags, modes, keybindings, themes, and config keys are
  unchanged. New keys don't collide: `Ctrl+P` and Results-screen `p` were
  previously unbound; the Stats screen's `↑/↓/Enter/r` were no-ops.
- `~/.typerush/stats.json` gains one optional `words` field per new record
  (`#[serde(default)]`): old records load fine (they just can't be
  replayed), and older TypeRush versions ignore the extra field.
- `state.json`, `aggregate.json`, and the `config.toml` schema are
  untouched. The `config` subcommands and Settings screen only write keys
  the loader already understood.
- New dependency `toml_edit` (already present indirectly via `toml`) powers
  comment-preserving config edits.

---

## [0.4.0] — more content & menu UX

### Added

- **10,000-word English pool.** A new `ENGLISH_10000` pool sits alongside the
  existing common-word pool. Opt in with `--big` or `[words] pool = "extended"`
  in `config.toml`. The default behavior is unchanged (Common pool, ≈1k words).
- **Programming-symbols mode.** A new `Mode::Symbols(N)` drills punctuation —
  tokens like `=>`, `(){};`, `&&`, `?.`, `[]`, `<T>` — sourced from a curated
  pool plus a handful of randomly composed tokens. Menu rows for 25 and 50
  tokens; `--symbols N` from the CLI; per-mode PB tracked as `symbols-N`.
- **Four new Code languages.** Added Go, Java, SQL, and Shell snippet pools.
  Menu rows: `Code · Go`, `Code · Java`, `Code · SQL`, `Code · Shell`. CLI
  accepts `--code go|java|sql|shell` plus the usual aliases (`golang`, `sh`,
  `bash`).
- **Punctuation / numbers toggles.** When enabled, randomly attach
  punctuation marks to ~25% of words and replace ~12% of slots with random
  1–4 digit numbers in Time / Words modes. Toggle via the `[words]` config
  section or the `--punctuation` / `--numbers` CLI flags. Zen mode ignores
  these toggles to stay calm.
- **Custom snippet library.** Drop `.txt` files into `~/.typerush/snippets/`
  and they appear as `Snippet · <name>` rows under the menu's Custom
  section. New `--list-snippets` CLI flag prints every snippet TypeRush
  found, one per line (`name\tpath`).
- **"Custom" menu row + last-file memory.** The menu now has a dedicated
  Custom row even when `--file` isn't passed. The path of the most recent
  custom session is persisted to `~/.typerush/state.json` so the menu
  pre-fills the row with `Custom · <truncated path>` across launches.
- **Visual section gaps in the main menu.** Rows are grouped under muted
  `── Section ──` headers (Time / Words / Quote / Code / Symbols / Zen /
  Custom / More). Arrow-key navigation skips headers automatically so the
  highlight always lands on a selectable row.
- **`docs/USAGE.md`** documents the new flags, modes, snippet workflow, and
  the new `[words]` config section. **`config.example.toml`** carries
  inline comments for every new option. **`docs/ARCHITECTURE.md`** picks up
  the new modules (`state.rs`, `words/snippets.rs`, `words/symbols.rs`) and
  gains a "When you add a new code language" runbook.

### Changed

- `App::new` now takes the word pool and decoration as required arguments
  instead of defaulting them silently. Callers in main and tests pass the
  desired values explicitly. (Internal-only change — no end-user impact.)
- The menu builder is now `app::build_menu(snippets, last_custom_file)`
  instead of a `default_menu()` constant — the same function is used by the
  app and by tests.
- The mode label `Mode::Code(JavaScript)` is documented as deliberately
  serializing to `"code-javascript"` (not the slug `"js"`) for backward
  compatibility with v0.3 and earlier `stats.json` records.

### Compatibility

- Existing CLI flags (`--time`, `--words`, `--quote`, `--code`, `--zen`,
  `--file`, `--theme`, `--list-themes`) are unchanged. The new flags
  (`--big`, `--punctuation`, `--numbers`, `--symbols`, `--list-snippets`)
  are purely additive.
- `~/.typerush/stats.json` and `aggregate.json` formats are unchanged. Old
  session records load and aggregate correctly; no migration needed.
- `~/.typerush/config.toml` adds an optional `[words]` section and three
  new optional `[defaults]` keys (`symbol_count`, plus extended `code_lang`
  aliases). Existing configs continue to load without modification.
- `~/.typerush/state.json` is new in v0.4.0 and entirely optional — TypeRush
  creates it lazily the first time a custom-file session is started.

---

## [0.3.0] — 2026-06-01 — smarter stats

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

### Performance

- **Stats and Results screens no longer read `stats.json` on every frame.**
  The session history is loaded once when you enter either screen and cached
  in memory for the duration of the visit, then invalidated when a new session
  is saved.
- **Per-key accuracy is now O(1) to display.** A small running aggregate
  (`~/.typerush/aggregate.json`) keeps cumulative per-key hit/miss totals,
  updated incrementally on each save, so the heatmap no longer rescans the full
  history every render. The aggregate is rebuilt automatically from existing
  sessions on first launch after upgrading.

### Fixed

- **Stable ordering in the key-accuracy panel.** Keys with identical accuracy
  (e.g. two keys both at 80%) are now broken ties alphabetically, so they no
  longer swap positions and flicker between renders.

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

[Unreleased]: https://github.com/withrvr/typerush/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/withrvr/typerush/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/withrvr/typerush/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/withrvr/typerush/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/withrvr/typerush/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/withrvr/typerush/releases/tag/v0.1.1
