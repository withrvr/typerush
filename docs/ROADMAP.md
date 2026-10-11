# TypeRush — Roadmap

What's done, what's next.

```
KEY:  ✅ done    🚧 in progress    🔲 planned    💡 idea
```

---

## ✅ Shipped (v0.1)

### Core engine
- ✅ Cross-platform terminal setup (raw mode + alt screen + panic hook)
- ✅ 100ms tick event loop
- ✅ Character-by-character matcher (`game.rs`)
- ✅ Live WPM + accuracy with the standard 5-char-per-word formula

### Modes
- ✅ Time mode (15 / 30 / 60 / 120 seconds)
- ✅ Words mode (10 / 25 / 50 / 100 words)
- ✅ Quote mode
- ✅ Code mode (Rust, Python, JavaScript)
- ✅ Zen mode (no timer, no stats)
- ✅ Custom file via `--file <path>`

### UI
- ✅ ASCII banner main menu
- ✅ Live colored typing screen (green/red/dim)
- ✅ **Steady, non-blinking cursor** (no layout jitter)
- ✅ Progress gauge for word/time modes
- ✅ Results screen with WPM delta vs last session
- ✅ Stats history screen (table + WPM sparkline)
- ✅ Floating `?` help overlay
- ✅ Error modal for bad CLI inputs / missing files

### Persistence
- ✅ Auto-save sessions to `~/.typerush/stats.json`
- ✅ Personal best tracking
- ✅ Average accuracy

### Project hygiene
- ✅ Unit tests for the matcher
- ✅ GitHub Actions CI (Linux / macOS / Windows)
- ✅ Clippy clean with `-D warnings`
- ✅ Inline doc comments

---

## ✅ Shipped (v0.2 — Customization)

- ✅ `~/.typerush/config.toml` for themes and defaults
- ✅ Named-color + hex (`#RRGGBB`) theme support
- ✅ Default word count / time / mode picked from config
- ✅ Light theme
- ✅ Built-in themes: `dark`, `light`, `monokai`, `dracula`
- ✅ Per-slot color overrides on top of any built-in theme
- ✅ `--theme <name>` CLI override + `--list-themes` discovery flag
- ✅ `Ctrl+Backspace` deletes the current word (terminals that send `^H`, plus `Ctrl+W`)

---

## ✅ Shipped (v0.2.1 — Pure character checking)

- ✅ Typing is one stream of characters; the space between words is a character like any other
- ✅ A wrong key where a space belongs is one wrong character (shown as a red, underlined space) instead of piling up letters on the word ([#8](https://github.com/withrvr/typerush/issues/8))
- ✅ A space typed mid-word is a wrong character — it no longer jumps to the next word
- ✅ The run ends on the last character (no trailing space needed)
- ✅ Backspace walks back over spaces one character at a time

---

## ✅ Shipped (v0.3 — Smarter stats)

- ✅ Per-key accuracy heatmap (find your problem keys)
- ✅ Per-mode personal bests (separate PB for time-30s vs words-50)
- ✅ Daily streak counter
- ✅ Average WPM over the last 7 / 30 days
- ✅ Crash-safe (atomic, fsynced) stats writes; a corrupt `stats.json` is backed up, never wiped
- ✅ Stats screens never read `stats.json` per frame (in-memory session cache)

**Added to v0.3 beyond the original plan:**

- ✅ Main menu grouped by category (heading, options on the line below) with
  2-D arrow navigation — pulled forward from v0.4's "visual section gaps"
- ✅ Mouse support (click menu options and footer hints, wheel scrolls the
  menu) with every action also on the keyboard (WCAG 2.1.1) and
  release-to-activate clicks (WCAG 2.5.2)
- ✅ Alternate keys: `F1` help, `F5` restart, `Space` start, `s` stats

---

## ✅ Shipped (v0.4 — More content)

- ✅ Bigger English word pool (10k) — `--big` or `[words] pool = "extended"`
- ✅ Programming-symbols mode (focus on `(){};=>` etc.) — `symbols` menu row, `--symbols N`
- ✅ More languages: Go, Java, SQL, Shell
- ✅ Punctuation / numbers toggle — `--punctuation`, `--numbers`, or `[words]`;
  `p` / `n` / `b` in the menu, `--no-…` switches to turn them off
- ✅ Menu fits a standard 80×24 terminal (compact banner, `▲/▼ more` when it
  can't)
- ✅ A personal best per custom file (`custom-notes`)
- ✅ Custom snippet library: drop `.txt` files in `~/.typerush/snippets/` (`--list-snippets`)
- ✅ "custom" menu row + last-picked-file memory in `~/.typerush/state.json`

(The original v0.4 item "visual section gaps in the main menu" shipped early,
in v0.3, as the category-grouped menu.)

---

## 🚧 In progress

- 🚧 Release v0.2.1, v0.3.0, v0.4.0, v0.4.1, v0.5.0 and v0.6.0: git tags, GitHub release binaries,
  crates.io. The latest published release is v0.2.0 — pre-built binaries on
  the [Releases page](https://github.com/withrvr/typerush/releases) and
  `cargo install typerush` both started with v0.1.1 / v0.2.0.

---

## ✅ v0.4.1 — Polish, speed and stats by category

Worked in this order, one commit (or a few) per step, on one branch and
one pull request:

1. ✅ Plan (this section) and the future-features list below
2. ✅ Dependencies on their latest versions; code moved to the new APIs
3. ✅ Leaner code: duplicate types, dead fields and hand-rolled helpers
   removed (nothing the standard library or an existing crate already does)
4. ✅ Security: custom files read with a size limit, never from a device
   or pipe; a dependency vulnerability check in CI
5. ✅ Typing screen: the cursor line never scrolls off screen, time mode
   never runs out of words, and only the visible lines are laid out
6. ✅ Themes: every built-in theme meets a contrast minimum (checked by a
   test), so text is easy on the eye in each one
7. ✅ Stats by category: a personal best per mode, each category's own
   sessions, averages and time typed, and a ★ on each mode's best run
8. ✅ A learnings log (bugs found and how they were fixed, features and how
   they were built) and a contributor skill file that points to it
9. ✅ Review, end-to-end test, docs, version 0.4.1

**Added to v0.4.1 after using it:**

- ✅ Results ignores keys you're still typing when a run ends (only Enter,
  F5, Esc, Tab, F1 and Ctrl+C act there)
- ✅ Key hints on the bottom row of every screen, worded the same way
- ✅ One layout grid: title row, blank row, full-width box; text two cells
  in; values in one column; the menu in the same frame with the banner
  inside
- ✅ Stats opens on the run you just finished; `[all]` button; a title row
  whose buttons and counter never move

## ✅ v0.5 — Quality of life

Worked in this order, one commit (or a few) per step, on one branch and
one pull request:

1. ✅ Plan (this section)
2. ✅ Pause / resume mid-session: `Ctrl+P` / `F2` stops the clock and hides
   the text; paused time never counts
3. ✅ Replay a recent session word-for-word: every session keeps the text
   it typed; pick one on the Stats screen (or `--replay N`) and type the
   same words again, compared with the original run
4. ✅ Daily challenge: the same words for everyone on a given day, from a
   seed of the date (`daily` menu row, `--daily`), with today's best
5. ✅ Export stats as CSV: `typerush export [FILE]`, and `e` on the Stats
   screen
6. ✅ `typerush config path / show / get / set / reset` and
   `typerush --init-config` (also `config init`): edit
   `~/.typerush/config.toml` from the shell, comments and layout kept
7. ✅ In-app Settings screen: theme (previewed as you pick), start mode,
   word settings, reset to defaults — saved to `config.toml` at once
8. ✅ Review, end-to-end test, docs, version 0.5.0

**Added to v0.5 along the way:**

- ✅ Stats lists have a selected row (keys, wheel, click) — what Enter
  replays
- ✅ Menu order: categories first (time, words, code, symbols, custom),
  then single runs (daily, quote, zen); `stats`, `settings` and `quit`
  share the last row, so the whole menu still fits 80×24
- ✅ A notice box for messages that aren't errors (the export), `~` for
  the home folder in paths on screen
- ✅ Fixed: a non-hex color in `[colors]` crashed at startup; a pipe or
  device at `config.toml` could hang or fill memory

## ✅ v0.6 — Profile and deeper results

Worked in this order, one commit (or a few) per step, on one branch and
one pull request:

1. ✅ Plan (this section)
2. ✅ Raw WPM (every keystroke, errors included), consistency (how even
   your speed was, second by second) and a character breakdown (correct /
   incorrect / missed) on Results, saved with every session
3. ✅ Results chart: WPM, raw speed and errors second by second for the
   session you just finished ([#4](https://github.com/withrvr/typerush/issues/4))
4. ✅ History: raw WPM and consistency columns; filters by date range and
   word settings (the mode is the category), sortable by date, WPM,
   accuracy, raw, consistency or time
5. ✅ Level and experience points, earned from time spent typing; the XP a
   run earned on Results
6. ✅ Runs started vs completed (a run ended with Esc, restarted or left
   isn't completed)
7. ✅ Profile screen: level, typing since, days typed, streaks, started vs
   completed, time and characters typed, and the best for every time and
   word count side by side
8. ✅ Activity calendar on the Profile screen: a year of days shaded by how
   long you typed, with a day you can pick to see its runs
9. ✅ Review, end-to-end test, docs, version 0.6.0

**Added to v0.6 along the way:**

- ✅ `Esc` ends a run *early*: saved, but not counted as completed
- ✅ The CSV export gains raw WPM, consistency, missed and completed
- ✅ `profile` joins `stats`, `settings` and `quit` on the menu's last row

## 🚧 v0.7 — Practice modes

Worked in this order, one commit (or a few) per step, on one branch and
one pull request:

1. 🚧 Plan (this section)
2. ✅ Quick restart on `Tab` while typing, and repeat the run you just
   finished (the same words) from Results with `F6`
3. ✅ A `[typing]` section in the config and on the Settings screen:
   hide the live WPM, and blind mode (no red/green while typing, the
   numbers only at the end)
4. ✅ Stop on error: `letter` (a wrong key doesn't move the cursor) or
   `word` (a word with a mistake can't be left until it's fixed)
5. ✅ Cursor styles — underline, block, bar — and an optional key sound
   (the terminal's bell, on every key or only on mistakes)
6. ✅ Pace cursor: a second, dimmer cursor moving at your best for the
   mode, or at a target WPM
7. ✅ Practise missed words: every session keeps the words you got wrong;
   a `missed` run is built from your most-missed recent words
8. ✅ Idle detection: a run with a long stretch without a key (10 s by
   default) is not saved, and Results says why
9. ✅ Custom themes from files in `~/.typerush/themes/`, on top of a
   built-in or from scratch, listed and picked like the built-in ones
10. 🔲 Review, end-to-end test, docs, version 0.7.0

---

## 🔲 Planned

---

## 💡 Ideas (not committed)

- 💡 Multiplayer race over a LAN
- 💡 Per-finger heatmap (left vs right hand, weak fingers)
- 💡 Adaptive practice: re-roll words containing your worst keys
- 💡 Webhook to post your PB to a chat channel
- 💡 Voice-over for accessibility
- 💡 Animated on-screen keyboard for learning to touch-type
  ([#5](https://github.com/withrvr/typerush/issues/5))

Got an idea that should be on this list? [Open an issue](https://github.com/withrvr/typerush/issues/new/choose).
