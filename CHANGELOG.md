# Changelog

All notable changes to TypeRush are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

_No unreleased changes yet._

---

## [0.7.0] — practice modes

More ways to practise: drill your mistakes, make every error count, race
your best, type blind — and make TypeRush look the way you want.

### Added

- **Practise missed words:** every session keeps the words you got wrong
  (even ones you fixed). The `missed` menu row (or `--missed`) builds a run
  from your most-missed recent words.
- **Repeat a run:** `F6` on Results types the same words again, as a
  replay compared with the run you just finished.
- **Quick restart on `Tab`** while typing (like `F5` / `Ctrl+R`).
- **A `[typing]` section** in `config.toml`, all of it also on the Settings
  screen and in `typerush config`:
  - `live_wpm` — hide WPM and accuracy while typing;
  - `blind` — no right/wrong colors while typing, numbers only at the end
    (`--blind`);
  - `stop_on_error` — `letter` (a wrong key doesn't move the cursor) or
    `word` (a word with a mistake can't be left until it's fixed)
    (`--stop-on-error`);
  - `cursor` — `underline`, `block` or `bar`;
  - `sound` — the terminal's bell on mistakes or on every key;
  - `pace` — a second cursor moving at your best for the mode (on a
    replay, the original run) or at a target WPM (`--pace`);
  - `idle_limit` — a run with that many seconds without a key (10 by
    default) isn't saved, and Results says why.
- **Custom themes:** a TOML file in `~/.typerush/themes/` is a theme named
  after the file, on top of a built-in; pick it like any other.
  `--list-themes` lists them and notes hard-to-read colors.
- `missed` can be the start mode (`mode = "missed"`).

### Changed

- The typing footer says `Tab / F5 restart`; the Results footer gains
  `F6 repeat` (`Ctrl+C` still quits there, as everywhere).
- On a short terminal the Settings list scrolls with the selection.
- The blank line under the menu's last option gives way before the banner
  on a terminal too short for both.
- The help overlay lists `Tab` and `F6`.
- **The Results chart is labelled on round numbers:** at least four labels
  on each axis and as many as fit, every label on its value.
- **The Stats WPM trend shows as many sessions as the box is wide** (it
  was the last 20), and says how many.
- **Nothing is drawn against a box's side any more** ([#20]): the chart,
  the WPM trend and the Stats tables are inset like the text in every other
  box, the help box is wide enough for its longest line, and Settings shows
  the selected row by its label and value rather than a marker in the
  border's first cell.

---

## [0.6.0] — profile and deeper results

See more of every run, and of all of them: a chart and new figures on
Results, filters on your history, levels, and a Profile with a year of
practice on a calendar.

### Added

- **A chart of the run on Results** ([#4]): the WPM so far, the raw
  speed of each second and a dot on every second with a mistake. Runs
  under two seconds, and zen, say why there's no chart.
- **Raw WPM** (every key typed, right or wrong), **consistency** (how even
  your pace was, second by second: 100% = the same speed every second) and
  **chars: correct · incorrect · missed** (missed = text left when a run is
  ended early) on Results. Raw WPM and consistency are saved, and shown as
  new columns in the Stats sessions list — raw WPM even for old sessions.
- **Filter and sort your history** on the Stats screen: dates (`d`: all
  time, today, 7 days, 30 days, a year), word settings (`w`: any, plain,
  `+p`, `+n`, `+10k`), sort (`s`: date, WPM, raw, accuracy, consistency,
  time) and `r` to reverse — as clickable chips on the list's bottom
  border. The filters apply to the summary, trend and bests table too, and
  stay as you change category and come back.
- **Levels and XP:** one XP per second typed; Results shows what a run
  earned (`+31 xp · level 7 · 62%`) and any level-up.
- **A Profile screen** (`u`, or `profile` on the menu's last row): level
  and XP, the day you started, days typed, current and longest streak, runs
  started and completed, time and keys typed, your best for 15/30/60/120 s
  next to 10/25/50/100 words, and the mode you play most.
- **An activity calendar** on the Profile: the last year, a column per
  week, each day shaded by how long you typed (`· ░ ▒ ▓ █`); pick a day
  with the arrows (`Home` for today) or a click to see its runs, time and
  best.
- The CSV export gains `raw_wpm`, `consistency`, `missed` and `completed`
  columns (at the end, so existing columns don't move).

### Changed

- **`Esc` while typing ends the run early:** it is still saved (if a
  second or longer), but counts as started, not completed.
- The Results trend strip is replaced by the run's chart (Stats keeps a
  trend per category); the Results box has two more lines.
- `state.json` also keeps how many runs you've started, and since when.
- The help overlay mentions the Stats filters, the Profile and the calendar.

### Fixed

- **The help box, the error / notice boxes, the Results chart and the
  filled part of the typing progress bar now use the theme's colors.** They
  were drawn on the terminal's own background — black on `light`,
  `monokai` and `dracula`. Every screen in every state, on every theme, is
  now checked for it.

---

## [0.5.0] — quality of life

Pause a run, type any session again word for word, a daily challenge,
your history as CSV, and settings you can change without opening a file.

### Added

- **Pause and resume** a run with `Ctrl+P` / `F2`. The clock stands still
  and the words are hidden until you resume; paused time never counts —
  not towards WPM, a time run's clock, or the time saved in your stats.
  `Esc` still finishes the run and `Ctrl+R` / `F5` still restart it.
- **Replay a session word for word.** Every session now keeps the words it
  typed. On the Stats screen, pick one with `↑` / `↓` (or a click) and
  press `Enter` to type exactly those words again — on the bests table,
  your best run in that mode. `typerush --replay` does the same for your
  last session, `--replay 3` for the third most recent. A replay ends the
  way the original did, is saved as `<mode>+replay` (so words you've seen
  never set your real best), and Results shows how you did against the
  original (`+12 vs original`). Restarting replays it again.
- **Daily challenge** — the `daily` menu row, `--daily`, or
  `mode = "daily"` in `[defaults]`: 40 words from the 10,000-word pool,
  the same for everyone on a given day and new every day. Results shows
  **today best**, counting only runs of today's words.
- **Export your history as CSV:** `typerush export` prints it,
  `typerush export FILE` writes it, and `e` on the Stats screen writes
  `~/.typerush/stats.csv`.
- **`typerush config`** — change settings from the shell without opening
  the file: `config show` (every setting and the value in use, marking
  defaults and values that are ignored, and why), `config get NAME`,
  `config set NAME VALUE`, `config reset [NAME]`, `config path`. Values
  are checked with the same rules TypeRush uses at start, and the file is
  edited in place: your comments and layout are kept. Resetting
  everything moves the file aside instead of deleting it.
- **`typerush --init-config`** (or `typerush config init`) writes a starter
  `~/.typerush/config.toml`: every setting at its default, each explained.
  It won't replace a config you already have.
- **A Settings screen** (`c` in the menu, or the new `settings` option):
  theme, start mode, punctuation, numbers and the 10k pool, plus reset to
  defaults (it asks first). Changes are saved at once and used at once — a
  theme shows as you pick it. Keyboard and mouse (`‹` / `›` on each value).

### Changed

- **The menu lists categories first, then single runs:** time, words,
  code, symbols, custom, then daily, quote, zen. `stats`, `settings` and
  `quit` share the last row, so the two new options still fit a standard
  80×24 terminal. The Stats bests table follows the same order.
- **Stats lists have a selected row** (highlighted) that `↑` / `↓`, the
  mouse wheel and a click move; the list scrolls with it. The footer now
  offers `Enter replay` and `e export`.
- **The help overlay** covers pause, replay, export and settings, and still
  fits 80×24.
- `config.example.toml` holds the built-in defaults (it used to pick the
  monokai theme), so as a starter file it changes nothing until edited.
- Paths shown on screen write your home folder as `~`, and text in the
  error box keeps its inset when it wraps.

### Fixed

- **A color like `"#aé€"` in `[colors]` crashed TypeRush at startup,** and
  `"#+F+F+F"` was accepted as a color. Hex colors must now be exactly six
  hex digits.
- **`config.toml` is only read if it is a regular file of at most 1 MiB,**
  like a custom file: a named pipe there used to hang startup, and a device
  or huge file was read into memory whole.

---

## [0.4.1] — polish, speed and stats by category

A faster, safer, easier-to-read TypeRush, and stats you can look at one
mode at a time.

### Added

- **Stats by category.** On the Stats screen, `←` / `→` moves between
  **all** sessions, a **bests** table (one row per mode: best WPM with that
  run's accuracy and date, average WPM, number of runs) and one category per
  mode you have played. A mode's category shows its own summary, key
  accuracy, WPM trend and every one of its sessions.
- **★ on record runs.** In the sessions list, the session holding its
  mode's best is marked with a star.
- **Stats opens on the run you just finished:** `Tab` on Results shows that
  mode's category. `[all]` in the title (or `a`) jumps back to every
  session; from the menu, Stats stays on the category you last looked at.
- **A steady Stats title:** `◆ stats   [all]   ‹ ›   01/05   time-30s` —
  the buttons and the zero-padded counter come before the category name, so
  nothing moves when the name's length changes. `[all]`, `‹` and `›` are
  clickable.
- **The sessions list scrolls** with `↑` / `↓` (`k` / `j`), `PgUp` /
  `PgDn`, `Home` / `End` or the mouse wheel, and its title says which rows
  are showing (`1–5 of 42`). It used to show only the last 10 sessions.
- **Time typed** in the Stats summary.
- The Stats screen remembers its category when you come back to it.

### Fixed

- **Keys typed after the run ends no longer skip the results.** A fast
  typist is still typing when the last word is done, and a stray `r`, `m`,
  `s` or `q` used to restart, leave or quit before the results could be
  read. On the Results screen only keys you never press while typing act now:
  `Enter` / `F5` / `Ctrl+R` restart, `Esc` menu, `Tab` stats, `F1` help,
  `Ctrl+C` quit.
- **The cursor no longer runs off the bottom of the typing box.** In a long
  run, after about one screenful of words (≈180 at 80×24) you were typing
  blind. The box now scrolls so the cursor's line stays second from the top.
  Lines are measured in terminal cells, so wide (CJK) characters and words
  longer than the box (a URL in a custom file) can't push it off either.
- **Time and zen runs never run out of words.** A fast typist used to reach
  the end of the 300 (time) or 500 (zen) generated words, after which every
  key was ignored. Words are now added as you go.
- **Readable colors in every theme.** Untyped text on `monokai` and
  `dracula` was about 3:1 against the background; every text color of
  `light`, `monokai` and `dracula` is now at least 4.5:1 (WCAG AA, checked
  by a test) with untyped text still dimmer than typed text. On `dark`,
  wrong characters and the mode tag use the brighter red and magenta.
- **The progress-bar label is readable.** It was yellow on the cyan fill
  (about 1.1:1) on `monokai`, `dracula` and `dark`; it now sits on the
  theme background.
- **`--file` only reads regular files, up to 1 MiB.** `--file /dev/zero`
  or a named pipe used to hang, and a huge file was read into memory whole.
- **`--words` and `--symbols` go up to 10,000, `--time` up to 3600.** A huge
  count used to try to build billions of words before the first frame.
  `word_count`, `time_seconds` and `symbol_count` in the config follow the
  same rule (out of range: a warning and the default).
- A custom file that isn't UTF-8 now says so ("is not UTF-8 text") instead
  of a generic read error.

### Changed

- **Key hints always sit on the bottom row,** in the same place on every
  screen (they were three rows up while typing, two on Results), worded
  the same way everywhere (`Enter`, `Esc`, `Tab`, `F1`, `Ctrl+C`; quit
  last).
- **One alignment grid for every screen:** the title on the first row, a
  blank row, then the content; text inside every box starts two cells in;
  every value on Results and in the Stats summary starts in the same
  column. The Results box fits its six lines (no empty rows), the Stats
  summary's two columns have room between them, the typing header lines up
  with the other titles, and the menu leaves a blank line above the word
  settings.
- **The main menu follows the same layout:** a `◆ select mode` title row,
  then one full-width box with the TypeRush banner inside at the top and
  the modes centered below it as one block (headings and options still
  lined up with each other). When the blank lines between categories don't all fit (80×24),
  none are drawn, instead of dense rows at the top and spaced ones below.
- **Dependencies upgraded:** ratatui 0.30 (crossterm 0.29 through its
  re-export, only the features TypeRush uses), rand 0.10, toml 1. The
  `dirs` crate is gone (the standard library's `home_dir` is used). The
  previous lockfile had four security advisories (anyhow, lru ×2, paste);
  the new one has none.
- **Faster redraws.** The typing screen styles only the lines on screen, so
  a keystroke costs the same at word 10 and word 1000.
- **Leaner code:** duplicate config enums, dead counters and an unused
  theme slot removed (about 150 fewer lines). The `[colors] extra` key is
  still accepted (and a bad value still warned about) so old configs load.
- **CI:** a weekly and on-change `cargo audit`, Dependabot for crates and
  Actions, read-only workflow tokens, `--locked` builds and a Rust 1.88
  (MSRV, now declared in `Cargo.toml`) check.
- **Docs:** `docs/LEARNINGS.md` (every bug so far: symptom, cause, fix and
  the test that guards it) and a contributor skill file; the roadmap lists
  the profile and practice features planned for v0.6 and v0.7.

---

## [0.4.0] — more content

More to type: a bigger word pool, a programming-symbols drill, four more
code languages, punctuation and number drills, and your own text files
straight from the menu.

### Added

- **10,000-word English pool.** `--big`, or `pool = "extended"` in a new
  `[words]` config section. The default pool (~430 frequent words) is
  unchanged. The pool is family-friendly: profanity, slurs and sexual terms
  were taken out (a test keeps them out), so it is safe in a classroom or on
  a streamed screen.
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
- **Word settings in the menu.** `p`, `n` and `b` toggle punctuation,
  numbers and the 10k pool while TypeRush is open. Their state is shown —
  as `on` / `off`, not by colour alone — on the menu's bottom border, where
  each one is also clickable.
- **Off switches.** `--no-big`, `--no-punctuation` and `--no-numbers` give a
  plain run when the config turns a setting on; with both forms given, the
  last one wins.
- **Separate personal bests for harder settings.** A time or words run with
  `--big`, `--punctuation` or `--numbers` is saved under its own label —
  `time-30s+10k`, `words-50+p+n`, … — so it never competes with plain runs.
  Plain runs keep their usual labels.
- **A personal best per custom file.** A custom session is saved as
  `custom-` plus the file name (`custom-notes` for `notes.txt`), so a short
  drill and a long essay no longer share one best or one Results comparison.
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

- The main menu fits itself to the terminal, so the whole menu — the new
  `symbols` and `custom` rows, `stats` and `quit` included — is on screen at
  launch on a standard 80×24 terminal. With room to spare it looks as in
  v0.3 (big banner, a blank line between categories); with less, it uses a
  two-line banner and drops only as many blank lines as it must, top first.
  Only a terminal too short for the whole list scrolls, and the border then
  says `▲ more` / `▼ more`.
- The menu box is at least 64 columns wide, so the seven code languages fit
  on one line at 80 columns. Rows still too wide (many snippets, a narrow
  terminal) wrap onto an indented line below. Long snippet names are
  shortened with `…`. Every option stays clickable.
- Custom-file errors name the file (`can't read /home/you/notes.txt: No such file or
  directory`, `notes.txt is empty`) instead of a bare OS error.
- The help overlay shows where snippets go and the `p` / `n` / `b` keys.
- The Stats screen's Mode column is two cells wider, to fit labels such as
  `words-100+10k+p+n`; longer labels are shortened with `…`.

### Fixed

- **AltGr keys type on Windows.** Windows reports AltGr as Ctrl+Alt, and the
  typing screen dropped every Ctrl chord — so on German, French, Spanish,
  Polish and other layouts `{ } [ ] | @ ~ \` could not be typed at all.
  Characters typed with AltGr now count; Ctrl+letter chords are still
  ignored.
- **A broken `config.toml` no longer drops your flags.** When the config
  couldn't be read or parsed, `--theme` (since v0.2) and the new word
  switches were silently ignored; they now apply on top of the defaults, and
  the warning is still shown.
- Snippets whose file name isn't valid UTF-8 (possible on Linux) were
  silently left out of the menu and `--list-snippets`; they are listed now,
  with `�` for the bad bytes.
- `--time 0`, `--words 0` and `--symbols 0` are rejected with a clear error.
  They used to start a session with nothing to type that only Esc could end.
- Control characters in a custom file, a file name or an error message can
  no longer reach the terminal as escape sequences: they are dropped from the
  text you type and shown as `?` in names and messages.
- A custom file saved with a byte-order mark (some Windows editors add one)
  no longer makes its first word impossible to type; invisible characters
  such as zero-width spaces are dropped from custom text too.

### Compatibility

- Every existing CLI flag, mode, menu row, key, theme and config key works as
  before; the new flags and config keys are additive. `--file` now accepts
  any path your OS allows.
- `stats.json` is unchanged and old records load as before. Saved personal
  bests keep their mode labels (`code-javascript` included); only runs with
  the new word settings get a suffix. Custom-file sessions saved before
  v0.4 keep their plain `custom` label and still show in the history; new
  custom sessions are saved per file (`custom-notes`).
- Zen is exactly as in v0.3: the common words, never decorated, whatever
  the word settings are.
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

[Unreleased]: https://github.com/withrvr/typerush/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/withrvr/typerush/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/withrvr/typerush/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/withrvr/typerush/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/withrvr/typerush/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/withrvr/typerush/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/withrvr/typerush/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/withrvr/typerush/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/withrvr/typerush/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/withrvr/typerush/releases/tag/v0.1.1
[#8]: https://github.com/withrvr/typerush/issues/8
[#4]: https://github.com/withrvr/typerush/issues/4
[#20]: https://github.com/withrvr/typerush/issues/20
