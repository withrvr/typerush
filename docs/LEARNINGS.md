# TypeRush — Learnings

What we found while building TypeRush: the bugs, why they happened, how they
were fixed and what now guards them; the features, and how they were built.
Add to it as you go — one entry per bug fixed or feature shipped, newest
version first. The `CHANGELOG` says *what* changed for users; this file says
*why* and *how*, for whoever touches the code next.

Each bug entry: **symptom** → **cause** → **fix** → **guard** (the test or
rule that keeps it fixed).

---

## Lessons that keep coming back

- **Measure, don't assert.** v0.2.0's changelog said the gauge label was
  "readable on every built-in theme"; measured in v0.4.1 it was about 1.1:1
  (yellow on cyan). Contrast, speed and size claims get a number or a test.
- **The terminal is a trust boundary.** Anything from a file name, a custom
  file, `stats.json` or an error message can carry an escape sequence. Draw
  it through `text::printable` (names, messages) or drop it with
  `text::is_untypeable` (text to type). Never print raw user text.
- **Every input has a size.** A file, a count on the command line, a list
  that grows while you type: give each one a limit, or a way to keep up
  (`--words 10^12`, `--file /dev/zero` and "time mode runs out of words"
  were all the same mistake).
- **Fix it once, where every caller goes through.** Path rendering, error
  display, character rules and atomic writes each have one helper; a bug
  fixed in one call site but not its siblings came back three times in v0.4.
- **One frame per keystroke must stay cheap.** The render path never reads
  the disk, and does work proportional to what is on screen, not to the
  whole session.
- **Measure the way you draw.** Layout and drawing must use the same width
  rule: `str::width` treats 👍🏽 as 2 cells, the per-character sum as 4, and
  the typing box draws per character.
- **Layout rules live in one place.** Each screen padding its own labels
  and drawing its own footer let every screen drift (labels 11–13 cells,
  footers on three different rows). One footer row in `ui::render`, one
  `ui::label` and `LABEL_WIDTH`, and a test that checks the columns keep
  them in line.
- **Mirror types rot.** A config-side copy of an enum (`DefaultMode`,
  `CodeLangKind`) has to be kept in step by hand; use the real type.
- **Tests never touch the real `~/.typerush`.** Everything that reads or
  writes takes a path; E2E runs use a throwaway `HOME`.
- **Profile before optimising.** A 3000-key tmux burst looked like input
  lag; measured, the terminal's ~4 KB input buffer was dropping keys and
  the app kept up either way, so batching redraws bought nothing and was
  not merged.
- **A rule fixed in one reader comes back in the next.** v0.4.1 bounded
  custom-file reads; the config reader kept `read_to_string` until v0.5.
  When you add a limit, grep for every other place reading the same kind
  of input.
- **Read the terminal's actual bytes.** Ctrl+Backspace arrives as `^H`,
  AltGr arrives as Ctrl+Alt on Windows, `Escape Down` in tmux arrives as
  Alt+Down. Check what the key really sends before writing the keymap.

---

## v0.7.0 — practice modes

### Bugs (caught before they shipped)

- **Settings outgrew short terminals.** Seven typing rows took Settings
  from six rows to thirteen; at 80×24 they fit, but on a shorter window the
  last rows — reset to defaults among them — were cut off with no way to
  reach them. Fix: the list scrolls with the selection, with `▲ / ▼ more`
  on the border, like the menu. Guard:
  `settings_scroll_on_a_short_terminal`. Lesson: every list that grows
  needs a plan for the screen it no longer fits.
- **The menu at 80×24 again.** The `missed` row pushed the menu with a
  snippet library past a standard terminal (the compact banner would have
  gone). Fix: the blank line under the last option now gives way first —
  after the gaps between categories, before the banner.
  `whole_menu_fits_80x24_at_launch` caught it.
- **Custom themes listed in byte order** (`Ocean` before `forest_2`).
  Fix: sorted ignoring case, like snippets. Found by the discovery test.
- **Chart labels off their values.** Asked for more axis labels, the
  first version handed ratatui five or more x labels — and they drifted:
  ratatui spreads the middle x labels over equal slots (label *i* centered
  at (i + ½)·width/count), not at their values; its y labels are placed
  proportionally and were fine. Found driving the release binary
  (`0s … 1s 2s 3s … 4s`). Fix: two blank x labels keep the row, and
  `draw_x_labels` writes each label centered on its own column, found from
  the axis corner ratatui drew. Guard: `chart_is_drawn_under_the_results`
  checks the labels are evenly spaced. Ticks are round (1, 2, 2.5, 5 ×
  10ⁿ), at least four, as many as fit, and each axis ends on its last
  label so labels sit on values.
- **Content against box borders** ([#20]). The chart's y labels, the WPM
  trend's bars and the Stats tables' ★ column started in a box's first
  cell; two help lines filled their box; Settings put its ➤ in the first
  cell. Fix: two cells of padding on those boxes (the tables' Mode column
  one cell narrower so the sessions table still fits 80 columns), a wider
  help box, no marker on Settings (the selected row shows by its bold label
  and highlighted value). Guard: `nothing_touches_a_box_border` finds every
  vertical border (a `│` run, overlays included) on every screen and fails
  on anything drawn beside it — checked to fail with the old padding and
  help width.
- **Found by the last review of the branch** (fixed with a test each):
  the chart's x labels panicked on a terminal 8–9 columns wide (`clamp`
  with min > max) — `no_screen_panics_at_any_size` now draws every screen
  at every width from 1 to 30; `F6` after a run over 64 KiB repeated one
  empty word (it took the saved text, which is empty past the limit — it
  now takes the run's words); stopping on words, the right space refused at
  the end of a wrong word counted as a Space-key mistake; zen runs showed
  "not saved: idle" though zen is never saved; `pace = best` on the daily
  challenge raced any day's best while Results meant today's; the Settings
  typing rows showed the file's values, not `--blind` / `--pace` in use. And
  three tidy-ups: one source for the stats path (`App::stats_file`, read
  through `App::history`), one helper for the typing rows, and word offsets
  for the pace cursor counted only when there is one. Lesson: a value
  shown on screen and a value used must come from the same place.
- **The help box had no room to breathe.** Padding above and below meant
  two more rows in a box that already used all 23 at 80×24; two entries
  that wrapped onto a second line (`p n b`, `Mouse`) were reworded to one
  line each — and the first rewording ran into the right border, which
  `nothing_touches_a_box_border` caught. Guard: `help_fits_80x24` checks
  the blank rows above and below.
- **`--list-themes` flattened a TOML error** into one line of `?`s
  (`printable` replaces the newlines). Fix: printed line by line, as the
  error modal does. Found driving the release binary.

### Features and changes

- **One shape for every choice setting.** `stop_on_error`, `cursor` and
  `sound` are enums with `NAMES` / `name` / `from_name`; the loader
  resolves them through one `named` helper and `typerush config` checks
  them with `Kind::Choice(&NAMES)`, so Settings, the CLI, the config and
  `config set` can't disagree about the names. `pace` (a word or a number)
  and `idle_limit` (seconds, 0 = off) have their own kinds; `pace = 60` and
  `pace = "60"` both parse through an untagged `TextOrNumber`, because a
  type error in TOML makes the loader drop the whole file.
- **Stop on error is decided before anything moves.** A blocked key is
  tallied (it counts against accuracy) and returns early; the last word of
  a text has no space, so in `word` mode the run's end waits on it too.
- **The bar cursor is the terminal's own.** A drawn bar would take a cell
  and shift the line — the jitter the steady cursor was built to avoid. The
  real cursor costs no room; its shape is set when the option changes and
  reset on exit and in the panic hook.
- **The key sound is a BEL byte.** An audio library would need system
  audio headers on Linux to even build; the bell is one byte, and
  terminals already let people choose how it sounds (or flashes).
- **Pace counts in one stream** — word characters plus one per space —
  the same way the cursor advances, so the ghost never drifts from the
  text. `best` is looked up when the run begins; `App::history` loads
  stats.json then if no screen has, never per frame.
- **Missed words are marked as they happen** (`Word::had_error`), not
  derived from the final text: a mistake you fixed is still a word worth
  practising. Words from stats.json are filtered like a custom file's.
- **Custom themes are installed once, globally.** Passing a theme list to
  every lookup (config, CLI, `config set`, Settings, `--list-themes`)
  would touch every caller; a `OnceLock` read by `builtin::by_name` and
  `names` needs none. Tests never install, so they only see built-ins.
- **F6 repeat is a replay.** The run's words are now seen, so it's saved
  as `+replay` and compared with the run — the same rule as any replay.
- **The trend's stars use the list's ★ rule** (`StatsView::best_indices`:
  the session holding each mode's best), on a row of their own under the
  bars, so a best is never hidden behind a tall bar. The trend box grew one
  row; the sessions list shows one fewer at 80×24.

---

## v0.6.0 — profile and deeper results

### Bugs (caught before they shipped)

- **Results walked the whole remaining text on every frame.** The first
  version showed "missed" characters by calling `missed_chars()` while
  drawing; for a 1 MiB custom file ended early that is ~170,000 words per
  frame. Fix: worked out once in `finish_game` (`App::missed`). Rule
  restated: the render path reads results, it doesn't compute them from
  the whole run.
- **A far calendar jump could overflow date arithmetic.** `Home` first
  moved the picked day by a huge number of days, and `NaiveDate + Days`
  panics out of range. Fix: `Home` picks today, and `move_day` uses
  `checked_add_days` / `checked_sub_days` and clamps. Guard:
  `calendar_day_stays_on_the_calendar` moves by `i64::MIN` and `i64::MAX`.
- **The chart's legend vanished.** ratatui hides a chart legend that takes
  more than a quarter of the chart's height — always, at 80×24. Fix: the
  legend is in the box title (with names, not only colors). Found by the
  render test, not by reading the docs.
- **Overlays were black on every painted theme.** Symptom (reported from
  use): the help box, opened from Results or anywhere, and the error and
  notice boxes showed a black background on `light`, `monokai` and
  `dracula`. Cause: they cleared their area with ratatui's `Clear`, which
  resets cells to the terminal's own colors, undoing the theme's
  background paint. Fix: one `ui::clear` that clears and repaints the
  theme's base style, used by all three. Then the Results chart's plot was
  reported black too: `Chart` draws on a canvas of its own that fills its
  cells with the terminal's colors unless the chart is given a style — and
  the first version of the screen-wide test drew Results with no run to
  chart, so it never saw one. Fix: the chart gets the theme's base style;
  the test now draws a real chart. Guards:
  `overlays_use_the_theme_background`, and
  `every_screen_is_on_the_theme_background` — every screen, with real data
  in every widget, on every painted theme: no cell in the terminal's own
  colors. Lesson: a check over "every screen" only covers the widgets the
  test actually makes draw. So the last pass, `no_terminal_colors_anywhere`,
  draws every screen in every state with data in every widget (paused,
  zen, a charted run, every Stats category with filters, a filled
  calendar, a pending reset) under each overlay at three sizes — and it
  found one more: the typing progress bar's filled part, whose
  `gauge_style` had no background. The release binary was swept the same
  way in tmux, counting default-background cells (zero on every painted
  theme; a parser that tracks colors across lines, since `capture-pane`
  only prints changes).
- **A test passed by luck.** `results_show_xp_and_level_up` typed "hello
  world" after `restart()`, which deals new random words; it passed only
  while a frame happened to finish. Fix: set the words after restarting.
  Rule: a test that types must type the words it put there.

### Features and changes

- **Raw WPM, consistency, breakdown.** `App::timeline` tallies keys per
  whole second, indexed by `elapsed()`, so pauses stay out without any
  special case. Raw WPM for old records comes from fields they already
  had (`total_chars`, `duration_secs`) — computed, not stored, so history
  from v0.1 on has it. Consistency is our own measure: 100 minus the
  coefficient of variation of keys per second, idle seconds counted.
- **The Results chart** replaced the trend strip (Stats has a trend per
  category). Whole seconds only: a last part-second divided by its length
  reads as a spike.
- **Filters** live in `StatsView` and are carried over when it is rebuilt,
  like the category. Two lists from one filter: `indices` in time order for
  the summary and trend, `order` sorted for the table. `bests_by_mode`
  became `bests_among` a set of indices so the bests table can follow the
  filters while the ★ and the categories don't.
- **Levels and XP** are pure functions of the history (no stored XP to
  drift out of step): one per second typed, a level curve that grows
  linearly. Worked out with `ResultsComparison` on Results entry.
- **Started vs completed.** Completed comes from records (`ended_early`);
  started needs a count of runs that were never saved, kept in
  `state.json`. It is counted in memory on a run's first key — a disk write
  there would sit in the keystroke path — and saved when a run ends and on
  quit, adding to the count on disk so two windows don't overwrite each
  other. `counting_since` lets sessions saved before counting began count
  as started instead of making "completed" exceed "started".
- **The calendar** is glyph density (`· ░ ▒ ▓ █`) as well as color, so it
  reads without color; shades are quarters of the busiest day shown. Day
  cells are ordinary `Select` click targets, numbered in days back from
  today, so the mouse needed no new kind of action.
- **The Profile at 80×24** is 2 + 11 + 10 rows: two 50/50 boxes (each side
  of the bests table 17 cells, so both fit half of 80) and the calendar
  (a month row and seven day rows). The header of the bests table is built
  with the same widths as its rows after the first one drifted a cell.

---

## v0.5.0 — quality of life

### Bugs

- **A six-byte color that wasn't hex panicked at startup.** Symptom:
  `accent = "#aé€"` in `[colors]` crashed TypeRush before the menu (and
  `"#+F+F+F"` was read as a color). Cause: `parse_hex` checked the length
  in *bytes*, then sliced by byte — inside a multi-byte character — and
  `from_str_radix` accepts a leading sign. Fix: every byte must be a hex
  digit before the value is split. Guard:
  `theme::color::tests::rejects_non_hex_bytes_without_panicking`. Found
  while giving colors to `config set`, which would have passed the same
  string straight through.
- **A pipe or device at `config.toml` could hang or fill memory.** Cause:
  the loader used `read_to_string`, the mistake fixed for custom files in
  v0.4.1 but not for the config. The first fix opened the file and then
  checked its type — but opening a named pipe *blocks* until something
  writes to it; review caught it. Fix: `config::load::read_config` checks
  the path first (regular file, at most 1 MiB), checks the opened file
  again, and bounds the read — one function for the loader, `config` and
  Settings. Guard: `config_must_be_a_small_regular_file` (makes a FIFO).
- **A count mode whose text ran out before its count ignored every key.**
  A replay of a hand-edited record (`words-50` with two words saved) would
  reach the end and sit there: only quote, code and custom runs finished on
  their last word. Fix: every mode with a fixed text finishes when nothing
  is left to type. Guard: `app::tests::short_replay_still_finishes`.

### Features and changes

- **Pause.** Two fields — when the pause began, time spent in earlier
  ones — and `elapsed()` leaves paused time out, so WPM, the time-mode
  clock and the saved duration all follow without knowing about pauses.
  Keys are ignored in `handle_char` / `handle_backspace` (one guard where
  every caller goes through), and the words are hidden: a stopped clock
  shouldn't let you read ahead.
- **Replay.** Sessions keep their text (64 KiB at most, skipped when
  empty, so old files and records are unchanged). `Mode::from_label`
  reverses `Mode::label` — a test round-trips every menu mode — so a record
  becomes a run again; its text is filtered like a custom file's, since
  stats.json can be hand-edited. A replay is saved as `+replay`: seen text
  must not set a real best. `start_game` split into "build the words" and
  `begin`, which both new runs and replays use.
- **Stats selection.** The lists gained a selected row (scrolled into view
  when drawn) instead of a scroll offset alone, so Enter has something to
  replay and a click something to select. The selected-row style is now
  one function shared with the menu.
- **Daily challenge.** The generator is a SplitMix64 in the source, not
  `rand`'s: `rand` documents that its generators' output may change
  between versions, and the daily words must come from the date alone. A
  test pins one day's words (and SplitMix64's published first value). Its
  best compares only runs of the same text (`ResultsComparison`'s
  `same_text`), which is exact where "same date" isn't (a run that ends
  after midnight).
- **CSV export.** Mode labels carry file names: made printable, quoted
  when they hold a comma or quote, and a leading `= + - @` defused with `'`
  so a spreadsheet can't run it as a formula.
- **`typerush config` and Settings.** Both go through `config::edit`:
  `check` reuses the loader's own rules (so `set` never writes something
  the loader warns about — a test sets every key and reloads), values are
  written one canonical way, and `toml_edit` keeps comments. Two details
  only a test showed: replacing a key with `insert` drops the comment above
  it (change the value in place), and comments after the last setting are
  the document's trailing text, so a new section would land above them
  (move them into the new section's prefix). Reset renames the file aside
  instead of deleting it. `keys_cover_every_config_field` destructures each
  config struct without `..`, so a new field can't be added without a key.
  The starter config is `config.example.toml`, now at the built-in
  defaults (it used to pick monokai), checked to load to exactly them.
- **The menu at 80×24 with two more options.** `daily` and `settings`
  pushed the list past a standard terminal again. Instead of shrinking the
  banner, `stats`, `settings` and `quit` share one headless row — they are
  actions, not modes — which also reads better. `whole_menu_fits_80x24_at_launch`
  caught it. After using it, the order changed to categories first (time,
  words, code, symbols, custom), then the single runs (daily, quote, zen):
  the "keep old rows in place for ↑/↓" rule gave way to a menu that reads
  in groups. `menu_offers_new_languages_and_symbols` pins the order.
- **Footers have a width.** Stats gained Enter replay and e export; at
  80 columns something had to go, and `↑/↓` (obvious from the highlighted
  row, and in help) did. Count the footer's cells when adding a hint.
- **A notice modal** next to the error one: "exported 3 sessions to …" is
  not an error. Paths on screen write the home folder as `~`
  (`text::screen_path`), and modal text is inset with the box's padding so
  a wrapped line keeps its inset.

---

## v0.4.1 — polish, speed and stats by category

### Bugs

- **Cursor ran off the bottom of the typing box.** Symptom: in a long time
  or words run, after about one screenful (≈180 words at 80×24) you typed
  blind. Cause: the words paragraph was laid out from the first word and
  never scrolled. Fix: lay out line numbers first, keep the cursor's line
  second from the top, and style only the visible lines. Review found the
  first version still counted characters, not cells, and let the widget
  wrap over-long words behind its back — so CJK text or a long URL could
  still push the cursor out. Now the layout measures cells, breaks long
  words itself, and one laid-out line is exactly one screen row. Guards:
  `cursor_stays_visible_deep_into_a_long_run`,
  `cursor_stays_visible_with_wide_and_overlong_words` (`ui::typing`).
  Cost, measured: a full frame takes ~0.08 ms at 100 words and ~0.29 ms at
  10,000.
- **Time and zen runs ran out of words.** Symptom: past 300 (time) or 500
  (zen) words every key was ignored until the clock ran out. Cause: a fixed
  word list sized by guess. Fix: `App::top_up_words` keeps 100 words ahead
  of the cursor. Guard: `app::tests::endless_modes_top_up_words`.
- **`--file /dev/zero` hung; a huge file filled memory.** Cause:
  `read_to_string` on whatever path was given. Fix: only regular files, at
  most 1 MiB, read through `take` so a growing file is still bounded.
  Guard: `words::tests::file_words_refuse_non_files_and_huge_files`.
- **`--words 1000000000000` tried to build a trillion words.** Fix: run
  lengths are bounded (time ≤ 3600 s, words/symbols ≤ 10,000) by one rule,
  `config::load::check_count`, used by the CLI parser *and* the config —
  the first version only bounded the CLI, so `word_count = 0` in the config
  slipped through. Guards: `tests::out_of_range_counts_are_rejected`,
  `out_of_range_config_counts_warn_and_use_defaults`.
- **A file growing past the 1 MiB limit mid-read could be cut inside a
  character** and reported as unreadable. Fix: only a character cut by the
  limit itself is dropped (a second review caught that the first fix also
  silently shortened small files ending in half a character — those are
  broken files and are now refused as "not UTF-8"); the opened handle is
  checked again for being a regular file. Guard:
  `file_words_drop_a_cut_character_and_refuse_non_utf8`.
- **Two-way footer hints were clickable one way** ("←/→ category" clicked
  as →). Rule restated: navigation hints carry no click; the title's ‹ ›
  are the mouse targets. Guard:
  `tests::clicking_stats_arrows_changes_category_both_ways`.
- **Untyped text was hard to read on monokai and dracula** (≈3.0:1), and the
  gauge label was yellow on cyan. Fix: palettes tuned to ≥ 4.5:1 per text
  color, untyped text kept dimmer than typed text, gauge label drawn on the
  theme background. Guard:
  `theme::builtin::tests::rgb_themes_meet_contrast_minimum`.
- **Four dependency advisories in the lockfile** (anyhow unsound downcast,
  two in lru, unmaintained paste). Fix: dependency upgrade. Guard: the
  Audit workflow (`cargo audit`, on dependency changes and weekly) and
  Dependabot.

### Features and changes

- **Stats by category.** `storage::StatsView` holds the category (all,
  bests, or one mode), that category's history indices and summary, every
  mode's best (`bests_by_mode`) and the indices of best-holding sessions for
  the ★. It is built when the Stats screen opens and when the category
  changes — never per frame. The summary helpers take
  `impl IntoIterator<Item = &SessionRecord>` so a category is summarised
  through references, not copies. Scroll lives in a `Cell` so drawing can
  clamp it to the rows that fit.
- **Leaner code.** Duplicate enums removed, theme overrides applied straight
  onto palette fields (one slot list instead of three), dead counters and an
  unused palette slot deleted, `dirs` replaced by `std::env::home_dir`,
  crossterm used through ratatui's re-export, ratatui built with only the
  features used.

---

## v0.4.0 — more content

### Bugs

- **AltGr characters couldn't be typed on Windows.** Cause: Windows reports
  AltGr as Ctrl+Alt and the keymap dropped every Ctrl chord. Fix: Ctrl+Alt
  with a non-ASCII-alphanumeric character is typed (`is_altgr`). Guard:
  `tests::altgr_characters_are_typed`, `tests::ctrl_alt_letter_chords_are_ignored`.
- **A broken `config.toml` dropped the command-line flags.** Fix: CLI
  overrides apply on top of the defaults whatever happened to the file.
- **Escape sequences from file names and custom text reached the terminal.**
  Fix: one rule (`text::is_unsafe` / `is_invisible` / `is_untypeable`),
  `printable` for display, filtering for typed text.
- **A byte-order mark made a custom file's first word impossible.** Fix:
  invisible characters are dropped from typed text. Guard:
  `words::tests::file_words_ignore_byte_order_mark`.
- **Two snippets could get the same menu label.** Fix: labels are computed
  as drawn and de-duplicated, natural names claimed before any " (n)".

### Features

- 10k-word pool (scrubbed of profanity, slurs and sexual terms — a test
  keeps it so), symbols drill, Go/Java/SQL/Shell, punctuation and numbers,
  a snippet library in `~/.typerush/snippets/`, the remembered last custom
  file in `state.json`, a menu that fits 80×24, a best per custom file.

---

## v0.3.0 — smarter stats

### Bugs

- **A session could be saved twice** (help opened from Results and closed).
  Fix: one `session_saved` flag cleared only when a new game starts.
- **A corrupt `stats.json` was silently replaced.** Fix: copied aside to a
  timestamped `.corrupt-…` file first. Writes are atomic and fsynced
  (`storage::write_atomic`), keep symlinks and permissions.
- **Unsaved sessions shifted the "vs last" delta and could flash "new
  best!".** Fix: `ResultsComparison` knows whether the current session was
  saved.
- **Equal-accuracy keys swapped places between frames.** Fix: ties broken
  by key.
- **Menu didn't scroll on short terminals; a mouse drag between options
  clicked the second one.** Fix: menu scrolls; a click only fires when press
  and release land on the same target (WCAG 2.5.2).

### Features

- Per-key accuracy (counted against the expected character), per-mode
  bests, daily streak, 7/30-day averages, mouse support with keyboard
  alternatives for everything, grouped main menu.

---

## v0.2.x — customization and pure character checking

- **Ctrl+Backspace did nothing** — terminals send it as `^H`, seen as
  Ctrl+H. Fix: Ctrl+H and Ctrl+W delete a word.
- **A wrong key on a space piled letters onto the word** ([#8]). Fix: the
  text is one stream of characters; a space is a character like any other.
- **Custom-file mode never finished on the last word.** Fix: finish when
  nothing is left to type.

[#8]: https://github.com/withrvr/typerush/issues/8
[#20]: https://github.com/withrvr/typerush/issues/20
