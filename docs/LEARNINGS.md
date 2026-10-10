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
- **Read the terminal's actual bytes.** Ctrl+Backspace arrives as `^H`,
  AltGr arrives as Ctrl+Alt on Windows, `Escape Down` in tmux arrives as
  Alt+Down. Check what the key really sends before writing the keymap.

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
