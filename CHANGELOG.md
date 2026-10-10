# Changelog

All notable changes to TypeRush are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

_No unreleased changes yet._

---

## [0.4.0] — more content

More to type: a bigger word pool, a programming-symbols drill, four more
code languages, punctuation and number drills, and your own text files
straight from the menu.

### Added

- **10,000-word English pool.** `--big`, or `pool = "extended"` in a new
  `[words]` config section. The default pool (~430 frequent words) is
  unchanged.
- **Programming-symbols mode.** A `symbols` row in the menu (25 or 50 tokens)
  drills the keys typists under-train: `=>`, `(){};`, `&&`, `?.`, `<T>` and
  friends, plus randomly composed punctuation runs. Also `--symbols N` and
  `[defaults] mode = "symbols"` / `symbol_count`. Personal bests are kept per
  count (`symbols-25`, `symbols-50`) and the progress gauge counts tokens.
- **Go, Java, SQL and Shell** in the `code` row, next to Rust, Python and
  JavaScript. `--code` and `[defaults] code_lang` accept `go`/`golang`,
  `java`, `sql`, `shell`/`sh`/`bash`.
- **Punctuation and numbers drills.** `--punctuation` adds commas, periods,
  quotes and brackets to about a quarter of random words; `--numbers` mixes
  in 1–4 digit numbers. Also `punctuation = true` / `numbers = true` under
  `[words]`. They apply to time and words modes; zen stays plain.
- **Separate personal bests for harder settings.** A time or words run with
  `--big`, `--punctuation` or `--numbers` is saved under its own label —
  `time-30s+10k`, `words-50+p+n`, … — so it never competes with plain runs.
  Plain runs keep their usual labels.
- **Snippet library.** Every `.txt` file in `~/.typerush/snippets/` (any
  capitalisation of `.txt`) is an option in the menu's new `custom` row,
  sorted by name. `--list-snippets` prints them as `name<TAB>path`.
- **"custom" menu row that remembers your last file.** The file you last
  typed — from `--file` or a snippet — is offered first in the `custom` row
  on the next launch, saved in `~/.typerush/state.json`. It is stored as an
  absolute path, so it works whichever directory you start TypeRush from,
  and it is only saved once the file has actually loaded. With no files yet,
  the row explains how to add one.
- **Launch video** in the README: a looping silent GIF linking to the full
  23-second MP4 with sound (`docs/brag/`, made with `/brag`), recorded from
  the real binary.

### Changed

- Menu rows wider than the menu box (seven code languages, many snippets)
  wrap onto an indented line below instead of running off the edge. Long
  snippet names are shortened with `…`. Every option stays clickable.
- Custom-file errors name the file (`can't read /home/you/notes.txt: No such file or
  directory`, `notes.txt is empty`) instead of a bare OS error.
- The help overlay shows where snippets go.
- The Stats screen's Mode column is two cells wider, to fit labels such as
  `words-100+10k+p+n`.

### Fixed

- `--time 0`, `--words 0` and `--symbols 0` are rejected with a clear error.
  They used to start a session with nothing to type that only Esc could end.
- Control characters in a custom file, a file name or an error message can
  no longer reach the terminal as escape sequences: they are dropped from the
  text you type and shown as `?` in names and messages.

### Compatibility

- Every existing CLI flag, mode, menu row, key, theme and config key works as
  before; the new flags and config keys are additive. `--file` now accepts
  any path your OS allows.
- `stats.json` is unchanged and old records load as before. Saved personal
  bests keep their mode labels (`code-javascript` included); only runs with
  the new word settings get a suffix.
- `state.json` is new and optional; it is written atomically, like
  `stats.json`, and a missing or corrupt one is simply ignored.

---

## [0.3.0] — 2026-10-09 — smarter stats

Planned as a stats release. Two additions came in beyond that plan: the
**grouped main menu** (pulled forward from v0.4) and **mouse support with
keyboard alternates** for accessibility.

### Added

- **Per-key accuracy heatmap.** TypeRush now tracks, for every character you
  are asked to type (the space between words included), how often you hit it
  and how often you typed something else instead. After enough sessions the Stats screen shows
  a "key accuracy" panel with up to 5 of your worst keys, including hit/total
  counts and a colour-coded accuracy percentage (red < 80%, amber < 93%,
  green otherwise). The panel shows "no key data yet (keep typing!)" until at
  least one key has come up 3 or more times. Per-key data is stored in
  `stats.json` alongside each session record.
- **Per-mode personal bests.** The Results screen now shows the personal best
  specifically for the mode you just finished (a `mode best` row) instead
  of the all-time best across all modes. A `★ new best!` badge fires whenever
  you beat your previous record for that mode, including on your first session.
  Zen-mode results display `— (zen not saved)` since Zen sessions are not saved.
- **Daily streak counter.** The Stats screen summary now shows how many
  consecutive calendar days (in local time) you have at least one session on.
  Streak counts backward from today (or yesterday — the streak is still active
  if you haven't typed yet today). Displayed as "1 day", "N days", or "—" when
  the streak is broken.
- **Average WPM over the last 7 and 30 days.** Two WPM averages appear in
  the Stats screen summary, over the last 7 and 30 calendar days in local time
  (today included — the same day boundaries as the streak). Both show "—" when
  no sessions fall within the window.
- **Backward-compatible stats file.** New `key_hits` / `key_misses` fields
  use `#[serde(default)]` so existing `~/.typerush/stats.json` records without
  them load cleanly — no migration needed.

### Added (input)

- **Mouse support.** Click a menu option to start it, click any footer hint
  (`Enter start`, `s stats`, `esc finish`, `? help`, …) to do what its key
  does, scroll the wheel to move through the menu, and click anywhere to close
  the help overlay or an error message. Every mouse action maps to an existing
  key (WCAG 2.1.1). A click only fires when the button is pressed and released
  on the same target, so sliding off — or dragging onto another option —
  cancels it (WCAG 2.5.2).
- **Alternate keys.** `F1` opens help (next to `?`), `F5` restarts while
  typing (next to `Ctrl+R`), `Space` starts the selected menu mode (next to
  `Enter`), and `s` opens stats from the menu (next to `Tab`).

### Changed

- **Main menu grouped by category.** `time`, `words` and `code` each get a
  heading with their options on the line below (`15s 30s 60s 120s`,
  `10 25 50 100`, `rust python javascript`), then `quote`, `zen`, `stats`,
  `quit`, with spacing between groups. `↑/↓` (`j/k`) moves between categories, `←/→` (`h/l`) between the
  options in a category.
- **Stats screen layout redesigned.** The summary card and the new key-accuracy
  heatmap sit side by side in a two-column top row, followed by the WPM
  sparkline and the recent-sessions table. The sparkline is slightly shorter
  (5 rows instead of 7) and the footer takes one row, so the recent-sessions
  table still shows 5 sessions in a 24-row terminal. The mode column is wide
  enough for `code-javascript`.
- **Short terminals.** The menu scrolls to keep the selected option visible
  (and clickable), and the help overlay's close hint sits in its bottom border
  so it shows even when the box is cut short. Error messages get a box tall
  enough for the whole message.
- **Footers.** The typing footer now reads `esc finish` (Esc ends the session
  and shows results; it never went straight to the menu) and drops `? help`,
  since `?` is typed as a character there. The help overlay is sized to its
  content and fits an 80×24 terminal.
- **Results screen body expanded** from `Constraint::Length(9)` to
  `Constraint::Length(10)` to accommodate the mode-specific best row.

### Performance

- **Stats and Results screens no longer read `stats.json` on every frame.**
  The history is read once and kept in memory; each save hands back the
  updated list, so finishing a session no longer re-reads the file. The Stats
  summary (streak, averages, key heatmap) is computed once each time the
  screen opens rather than on every redraw.

### Fixed

- **Stable ordering in the key-accuracy panel.** Keys with identical accuracy
  (e.g. two keys both at 80%) are now broken ties alphabetically, so they no
  longer swap positions and flicker between renders.
- **Crash-safe stats writes.** `stats.json` is now written atomically (temp
  file flushed to disk, then renamed over the old one), so a crash or power
  loss mid-save can't leave a half-written or empty file. A symlinked
  `stats.json` (e.g. into a dotfiles folder) stays a symlink, its permissions
  are kept, and two TypeRush windows saving at once never share a temp file.
  Works on Linux, macOS, and Windows.
- **A corrupt `stats.json` is no longer silently wiped.** If the file can't be
  parsed when a session is saved, it is copied to a timestamped
  `stats.json.corrupt-YYYYMMDD-HHMMSS.mmm` before a fresh history is started,
  so a later corruption can't overwrite an earlier backup.
- **A session could be saved twice.** Opening help from the Results screen and
  closing it saved the same session again (and hid the `★ new best!` badge).
  Each finished game is now saved exactly once.
- **Accurate Results screen for unsaved sessions.** Sessions that are not
  recorded (Zen mode, shorter than 1 second, or zero keystrokes) no longer
  shift the "vs last" delta by one session, and can no longer flash a
  `★ new best!` badge for a record that was never kept. If no session for the
  mode has been recorded yet, the mode best shows `—`.

### Compatibility

- Existing CLI flags, modes, and config are unchanged. The stats file only
  gains two optional fields (`key_hits`, `key_misses`). Old `stats.json` records load correctly and contribute to
  global stats; they just won't provide per-key heatmap data.

---

## [0.2.1] — pure character checking

### Changed
- **Pure character checking.** Typing is now one stream of characters, and
  the space between words is a character like any other. Each key is
  compared with the character under the cursor — match is correct, anything
  else is wrong — and the cursor moves on one slot either way. A space typed
  mid-word is a wrong character and no longer jumps to the next word. The run
  ends when the last character is typed; no trailing space is needed.
  Backspace walks back over spaces one character at a time.
- The `extra` color slot is now unused (typing can no longer run past the end
  of a word). It is still accepted in config files so existing configs load.

### Fixed
- **Wrong key on a space no longer piles up letters** ([#8]). Typing a letter
  where a space belongs used to append surplus characters to the word and
  leave the cursor stuck until space was pressed. It now counts as a wrong
  character: the space is shown red and underlined and the cursor moves on.

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

[Unreleased]: https://github.com/withrvr/typerush/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/withrvr/typerush/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/withrvr/typerush/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/withrvr/typerush/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/withrvr/typerush/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/withrvr/typerush/releases/tag/v0.1.1
[#8]: https://github.com/withrvr/typerush/issues/8
