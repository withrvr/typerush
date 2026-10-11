# TypeRush — Architecture

This is the map of the source tree. Read it before making non-trivial changes.

---

## Big picture

TypeRush is a single-process TUI app with one event loop, one mutable `App`
state object, and a stateless renderer. The flow per frame:

```mermaid
flowchart TD
    loop["main.rs::run_app — every frame
    1. terminal.draw(...)
    2. event::poll (100 ms)
    3. handle_key / handle_mouse
    4. app.tick()
    5. after_input: save a finished game once,
    load the history cache, compute summaries"]
    app["App (src/app.rs)
    the single source of truth"]
    ui["ui::* (src/ui/...)
    pure renderer, no state of its own
    (only registers click targets)"]
    loop -- "reads & mutates" --> app
    app -- "read-only" --> ui
```

---

## State machine

The `Screen` enum in `src/app.rs` is the application's high-level state:

```mermaid
stateDiagram-v2
    [*] --> Menu
    [*] --> Typing: mode flag (--time, --words, --quote, --code, --zen, --symbols, --daily, --replay, --file)
    Menu --> Typing: Enter / Space / click an option
    Typing --> Typing: Ctrl+R / F5 restart, Ctrl+P / F2 pause and resume
    Typing --> Results: mode finishes / Esc
    Results --> Typing: Enter / F5 / Ctrl+R restart
    Results --> Menu: Esc
    Results --> Stats: Tab
    Menu --> Stats: Tab / s
    Stats --> Typing: Enter replays the selected session
    Stats --> Menu: Esc / m / Tab
    Menu --> Settings: c / click settings
    Settings --> Menu: Esc / m / Tab
    Menu --> Profile: u / click profile
    Profile --> Menu: Esc / m / Tab
    Menu --> Help: ? / F1
    Results --> Help: F1
    Stats --> Help: ? / F1
    Settings --> Help: ? / F1
    Profile --> Help: ? / F1
    Help --> [*]: Esc / ? / F1 / q / click
    note right of Help
        Overlay: remembers the previous screen
        and returns to it when closed.
        Not reachable while typing (? is a character there).
    end note
```

Every state transition happens by setting `app.screen` from a keymap handler in
`main.rs`.

---

## Data flow per keystroke

```mermaid
flowchart TD
    read["crossterm::event::read()"] --> key["main.rs::handle_key
    matches on app.screen, calls the per-screen handler"]
    read --> mouse["main.rs::handle_mouse
    press + release on the same click target"]
    mouse -- "acts like its key" --> key
    key -- "e.g. Screen::Typing" --> char["app.rs::handle_char / handle_backspace"]
    char --> check["pure character check: the key is compared with the
    character under the cursor (the space between words included).
    Match = correct, anything else = wrong; the cursor
    moves on one slot either way."]
    check --> mutate["mutates: words[current].typed, correct_chars,
    total_typed_chars, key_hits / key_misses, current_word
    (past a word's trailing space), screen = Results (mode done)"]
    mutate --> back["back to the main loop: after_input, then the next
    frame is drawn from the new state"]
```

Mouse events take a short detour into the same path. While drawing, the UI
registers clickable regions in `App::click_targets` (menu options and footer
hints, rebuilt every frame). `main.rs::handle_mouse` looks up the region
under the pointer. A left-button press remembers that target
(`App::pressed_target`); the release fires its `ClickAction` only if it lands
on the same target: a footer hint calls `handle_key` with its key, a menu option
selects that option and presses Enter. Nothing is reachable by mouse that
isn't reachable by keyboard.

---

## Module responsibilities

```mermaid
flowchart LR
    src["src/"]
    src --> main["main.rs
    Entry point: terminal, event loop, panic hook,
    CLI parser (clap), key + mouse dispatch,
    after_input (save once, caches)"]
    src --> commands["commands.rs
    subcommands that don't open the app:
    export (CSV), config path/show/get/set/reset/init"]
    src --> settings["settings.rs
    Settings screen state (SettingsView) and actions:
    change, save through config::edit, apply at once"]
    src --> progress["progress.rs
    XP and levels (Level::of), Profile: totals,
    days typed, streaks, standard bests, calendar day"]
    src --> app["app.rs
    App state machine: all fields and transitions,
    WPM / raw WPM / accuracy math (paused time left out),
    the per-second timeline, consistency, missed chars,
    runs started (saved to state.json), pause,
    mode switching, completion, per-key tracking,
    replay (Replay, Mode::from_label), session text,
    menu navigation, click targets"]
    src --> game["game.rs
    get_char_states(target, typed): the matcher
    behind all colored feedback"]
    src --> storage["storage.rs
    SessionRecord + ~/.typerush/stats.json (atomic writes).
    personal_best, personal_best_for_mode, average_accuracy,
    streak, avg_wpm_last_n_days, key_accuracy,
    bests_by_mode / bests_among, StatsSummary,
    StatsView (selection, Filters: dates, words, sort),
    ResultsComparison (incl. XP), to_csv / export_csv"]
    src --> state["state.rs
    ~/.typerush/state.json: the last custom file, runs
    started and since when (atomic writes, path passed in)"]
    src --> text["text.rs
    printable / shorten / screen_path: safe,
    width-aware display of user-supplied names,
    paths and file text"]
    src --> theme["theme/"]
    theme --> theme_mod["mod.rs
    ThemePalette (10 color slots) + per-slot overrides"]
    theme --> builtin["builtin.rs
    dark, light, monokai, dracula
    (new theme = one row in ALL)"]
    theme --> color["color.rs
    #RRGGBB and 16 ANSI color names: parse_color,
    color_name (the other way)"]
    src --> config["config/"]
    config --> config_mod["mod.rs
    Config / Defaults / Colors / Words from config.toml (serde)"]
    config --> load["load.rs
    load_or_default_with(CliOverrides): never errors,
    bad files become one human-readable warning;
    read_config (regular file, 1 MiB at most);
    code_lang / word_pool / MODE_NAMES / defaults:
    one table each for the loader, CLI and edit"]
    config --> edit["edit.rs
    KEYS (every setting), check (the loader's rules),
    effective values, set / reset / reset_all / init
    with toml_edit: comments and layout kept"]
    src --> ui["ui/"]
    ui --> ui_mod["mod.rs
    render() dispatcher, background paint,
    one clickable footer on the last row for every
    screen (each render returns its hints)"]
    ui --> menu["menu.rs
    title row, then one full-width box: banner on top,
    modes grouped by category, sized to fit (big or
    compact banner, category gaps all or none,
    then scrolling with more hints); word settings on
    the bottom border; long rows wrap"]
    ui --> typing["typing.rs
    header, progress gauge, words"]
    ui --> results["results.rs
    WPM delta, raw, consistency, breakdown, mode best,
    XP in the title, the run's chart (second by second)"]
    ui --> stats["stats.rs
    categories, summary, key heatmap, sparkline,
    sessions table (★ bests, raw, consistency),
    bests table, filter chips on the bottom border"]
    ui --> profile_ui["profile.rs
    you / bests boxes, activity calendar (a year of
    days, shaded by time typed; a day to pick)"]
    ui --> settings_ui["settings.rs
    the saved settings as ‹ value › rows,
    reset to defaults (asks first)"]
    ui --> help["help.rs
    help overlay, error and notice modals"]
    src --> words["words/"]
    words --> words_mod["mod.rs
    word source picker (random, quote, code, symbols, file);
    WordPool + WordDecor (punctuation / numbers);
    daily_words (SplitMix64 seeded by the date)"]
    words --> english["english.rs
    ENGLISH_COMMON (~430 common words) and ENGLISH_10000"]
    words --> quotes["quotes.rs
    programming quotes + Rust / Python / JS /
    Go / Java / SQL / Shell snippets"]
    words --> symbols["symbols.rs
    programming-symbol tokens for Mode::Symbols"]
    words --> snippets["snippets.rs
    *.txt discovery in ~/.typerush/snippets/"]
```

---

## Key design choices

### Immediate-mode rendering
ratatui re-renders the whole UI every frame from `&App`. There is no retained
widget state. This makes reasoning about the UI trivial: whatever's in `App` is
what you see.

### Steady (no-blink) cursor
Earlier versions blinked a phantom `▏` character past the end of the typed
word. That toggled the rendered word width, shifting the whole line left/right
every 500ms. The current design renders a **steady** cursor as an
underlined character in `theme.accent`:

- on a character → the character itself, restyled with the cursor style
- on the space after a word → an underlined trailing space, same style

Both cases use the same `fg = theme.accent, modifier = UNDERLINED`. The
trailing space is always part of the layout, so toggling its style never
changes width.

### Theme-aware background paint
`ui::render` calls `frame.buffer_mut().set_style(area, ...)` with the
active theme's `background` color before any screen renders. This is what
makes the `light` theme actually look light on a dark terminal, and what
gives `monokai` / `dracula` their canonical backgrounds regardless of
terminal config. The `dark` theme uses `Color::Reset` so the user's
terminal background shines through exactly as it did in v0.1 — there is
no forced override.

Widgets that include plain `Line::from(string)` or `Cell::from(string)`
entries (no explicit fg) need a widget-level `.style(fg=theme.neutral)`
so they don't fall through to the terminal's default foreground — which
becomes invisible the moment the light theme paints a white background
over a dark terminal. The menu list, stats date column, and help overlay
all follow this pattern.

### Stats persistence
JSON, flat array, no schema. The file is small enough (one session ≈ 200B
before v0.3.0, slightly larger with per-key maps) that we rewrite it whole on
every save. Zen-mode sessions are deliberately excluded.

v0.3.0 added two optional fields to `SessionRecord` (and v0.5.0 a third,
`text` — see [Replay](#replay-and-the-daily-challenge-v050)):
- `key_hits: HashMap<String, u64>` — per expected character, times typed correctly
- `key_misses: HashMap<String, u64>` — per expected character, times something
  else was typed instead (the space between words counts as a character)

Both fields use `#[serde(default)]` so old records without them load cleanly
as empty maps. The aggregation helpers (`streak`, `avg_wpm_last_n_days`,
`personal_best_for_mode`, `key_accuracy`) all live in `storage.rs` and are pure functions over
`&[SessionRecord]`.

### Read-path performance (v0.3.0)

The Stats and Results screens never touch disk on a frame (the render loop
redraws every 100 ms tick and on every input event, mouse motion included).

- **`App::stats_cache: Option<Vec<SessionRecord>>`** holds the full history.
  It's read from `stats.json` the first time the Stats or Results screen needs
  it. `storage::save_session_to_path` re-reads the file, appends, writes, and
  returns the updated list, which replaces the cache — so `stats.json` stays
  the single source of truth and an outside edit shows up after the next save.
- **`App::stats_view: Option<StatsView>`** (v0.4.1) — the Stats screen's
  category (all, bests, or one mode), the history indices of that
  category's sessions, its `StatsSummary` (PB, averages, streak, time typed,
  key accuracy), every mode's best (`bests_by_mode`) and the set of
  best-holding sessions for the ★. Built from the cache when the screen is
  entered (except when coming back from Help) and when the category
  changes; never per frame. The summary helpers take any iterator of
  `&SessionRecord`, so a category is summarised by reference. The list's
  scroll is a `Cell` so drawing can clamp it to the rows that fit.

All of this bookkeeping lives in `main.rs::after_input`, which also saves each
finished game exactly once: the "saved" flag is cleared only when a new game
starts, so visiting Help from Results can't save the session again.

### Custom files and `state.json` (v0.4.0)

The menu's `custom` row is built by `app::build_menu(snippets, last_file)`:
the last custom file first (skipped when it is one of the snippets), then each
snippet, or a single placeholder option when there is neither. Its labels come
from `app::custom_labels`, already terminal-safe and shortened, and all
distinct as drawn: snippets are labelled from the snippet set alone (so the
remembered file never changes them), the remembered file falls back to
`folder/name`, and repeats get the first free " (n)". `App::new`
never touches disk; `main.rs` discovers snippets and loads `state.json`, then
hands both to `App::load_custom_sources` together with the path to save to.
Tests leave that path `None`, so they can never write to a real home
directory.

Every custom start goes through `App::start_custom(path)`: it sets
`custom_file`, calls `start_game`, and only then saves the file to
`state.json` and the menu. If the file fails to load, `custom_file` is put
back, so neither a restart nor the remembered file ever points at the file
that failed. `start_game` builds the word list before changing any state, so
the screen, mode and words are untouched too. The saved path is made absolute
so it works from any directory.

`App::session_label` names a custom session after its file
(`custom-notes`), so each file has its own personal best; time and words
labels get the `+10k` / `+p` / `+n` suffixes of the word settings, which the
menu's `p` / `n` / `b` keys toggle on `App::word_pool` / `App::word_decor`.

### Pause (v0.5.0)

`App::paused_at` is when the current pause began and `App::paused_for` the
time spent in earlier pauses. `elapsed()` stops at `paused_at` while paused
and subtracts `paused_for`, so WPM, a time run's clock and the saved
duration all leave paused time out without any of them knowing about
pauses. `finish_game` resumes first, so a run finished while paused ends at
the moment it was paused. `handle_char` / `handle_backspace` ignore keys
while paused (one guard where every caller goes through), and the typing
screen draws the box without its words.

### Replay and the daily challenge (v0.5.0)

`SessionRecord::text` keeps a session's words, space-separated: every word
of a fixed text, the words reached in a time run (`App::session_text`),
nothing past `MAX_TEXT_BYTES` (64 KiB). `#[serde(default,
skip_serializing_if = "String::is_empty")]` keeps old files loading and
text-less records unchanged on disk.

`Replay::of(record)` turns a record back into a run: its words (held to the
custom-file rules, since stats.json can be hand-edited), its mode from the
label (`Mode::from_label`, the reverse of `Mode::label`), the original's
word settings from the label's `+10k` / `+p` / `+n` for a time run's fresh
words, and its WPM and date for Results. `App::start_replay` sets
`App::replay` and calls `begin`, the half of `start_game` that resets the
session; `restart` replays again while `replay` is set, and `start_game`
clears it. `session_label` is the original's plus `+replay`.

The Stats lists have a selected row (`StatsView::selected`, scrolled into
view when drawn); `selected_session` names the history index to replay —
on the bests table, the mode's best run.

`words::daily_words(date)` picks 40 words with a SplitMix64 written out in
the source, seeded by the date's day number: `rand`'s generators may change
between versions, and the daily words must depend on the date alone (a
test pins today's). `ResultsComparison::new` takes `same_text`, so the
daily challenge's best is only over earlier runs of today's words
(`App::best_text`).

### Editing `config.toml` (v0.5.0)

`config::edit` is shared by the `typerush config` subcommands and the
Settings screen. `KEYS` lists every setting with its `Kind`; `check` holds a
value to the loader's own rules (the same `check_count`, `code_lang`,
`word_pool`, `builtin::by_name`, `parse_color`) and writes it one canonical
way. `set` and `reset` edit the file as a `toml_edit` document — a value
changed in place keeps the comments above its key and after its value, a
new section goes after the file's trailing comments — and write it with
`write_atomic`. A file that isn't valid TOML is never written. `reset_all`
renames the file aside rather than deleting it. `effective` gives each
setting's value as the app will use it, and where it comes from (file,
default, or a file value the loader ignores, with the reason).

The Settings screen keeps a `SettingsView` read from the file on the way in
(not per frame) and after each change; every change is saved, then applied
to the running app (theme with the file's color overrides, start mode as
the menu's highlight, word settings).

### Deeper results (v0.6.0)

`App::timeline` tallies the run per whole second of typing (keys typed,
right, wrong), indexed by `elapsed()` — so paused time is left out like
everywhere else. From it come `consistency()` (100 minus the coefficient of
variation of keys per second, idle seconds included) and the Results chart
(`ui::results::chart_points`: the running WPM, each second's raw speed, the
seconds with mistakes; whole seconds only). `raw_wpm()` is every key over
the minutes; `SessionRecord::raw_wpm` works it out from the saved key
count and duration, so it exists for every old record. `missed` (text left
when a fixed run ends early) is worked out once in `finish_game` — it walks
the rest of the text, which can be big — and `end_early` (Esc) marks a run
as not completed. Records gain `consistency`, `missed` and `ended_early`,
each left out of the file when empty.

### History filters (v0.6.0)

`storage::Filters` (date range, word settings, sort and direction) lives in
`StatsView` and is carried over when the view is rebuilt. `select` filters
the category's sessions into `indices` (in order of time: the summary and
trend), sorts a copy into `order` (the list) and recomputes
`table_bests` with `bests_among(history, indices)`; the categories and the
★ still come from the unfiltered `bests`. Sorting puts a run without the
figure last whichever way round, ties newest first.

### Levels, XP and the Profile (v0.6.0)

`progress` is pure: one XP per whole second of a saved session,
`Level::of(xp)` walks levels of 300 + 150 × (level − 1). `ResultsComparison`
carries the XP before and with the run, worked out on Results entry with
the rest. `progress::Profile` adds up the history once when the Profile
opens (per-day totals in a `BTreeMap`, streaks, completed runs, the
standard bests via `bests_by_mode`) and holds the calendar's picked day.

Runs *started* are counted on a run's first key (not zen) into
`App::runs_started_unsaved` — no disk write in the key path — and added to
`state.json` by `save_run_count` when a run ends and on quit, read-modify-
write so two windows add up. `counting_since` lets `App::runs_started`
count saved sessions from before counting began as started too.

The calendar (`ui::profile::render_calendar`) draws as many weeks as fit
(up to 53), Monday on top and the current week last, each day's shade its
share of the busiest day shown in quarters, as glyphs that get denser; each
day cell is a `ClickAction::Select(days_back)` target.

### Config loading

`config::load::load_from(path, cli)` never fails: a missing file means
defaults, and an unreadable or unparseable one means defaults plus a warning
(shown in the error modal). Either way the CLI overrides (`--theme`, `--big`,
`--no-big`, `--punctuation`, …) are applied on top, so a typo in
`config.toml` never silently drops a flag.

### Cross-platform
crossterm handles Windows Console API, ANSI escape codes, and raw mode in one
crate. We don't directly use any platform-specific code, so the binary is a
"just build it" affair on any of Linux, macOS, Windows, or WSL.

### Error handling
- Setup / teardown errors propagate (`anyhow::Result`).
- A `panic::set_hook` resets the terminal before the panic message prints, so a
  crash never leaves the user's shell broken.
- Disk I/O for stats is best-effort — losing a record is never worth crashing.
- In the app, a failed action shows the error modal (`App::show_error`) and a
  finished one that needs saying (an export) the notice modal (`App::notice`);
  any key or click closes either.
- Subcommands (`export`, `config …`) run before the terminal is touched and
  report errors like `main` does (`print_error_and_exit`, status 1).

---

## Performance notes

- Render loop runs at 10 fps (100ms tick). The actual `terminal.draw` cost is
  dominated by ratatui's diff algorithm — only changed cells are repainted.
- The typing screen lays out line numbers for every word (a character
  count each), then styles only the words on visible lines: the box scrolls
  so the cursor's line stays second from the top. A keystroke's redraw costs
  the same at word 10 and word 1000.
- `get_char_states` is O(n) in word length, walks both strings with
  iterators and is called once per visible word per frame.
- Time and zen runs keep 100 words ahead of the cursor (`App::top_up_words`)
  instead of generating a large list up front.

---

## When you add a new mode

1. Add a variant to `Mode` in `src/app.rs`.
2. Pattern-match it inside `App::start_game` to pick a word source.
3. Update `Mode::label` so it persists nicely in stats.
4. Add an entry to `build_menu()` — items sharing a `group` render under one
   heading, options side by side (wrapping when the row is too wide). Rows
   with a choice of options go with the other categories (time … custom),
   a single run with daily, quote and zen; the actions row stays last.
5. If it has a unique completion condition, handle it in `advance_word()` /
   `tick()`.
6. (Optional) Add a CLI flag in `main.rs::Cli`.

For v0.5 and later also: give it a name in `Mode::label` that
`Mode::from_label` reads back (so its sessions can be replayed — a test
round-trips every menu mode), and if it can be a start mode, a name in
`config::load::MODE_NAMES`, `resolve_default_mode` and
`settings::start_settings`.

## When you add a setting

1. Add the field to the `Config` structs in `src/config/mod.rs` and resolve
   it in `src/config/load.rs`.
2. Add a `Key` to `config::edit::KEYS`, and its arms in `file_value` and
   `default_value`. `keys_cover_every_config_field` destructures every
   struct without `..`, so it won't compile until you look at it.
3. Document it in `config.example.toml` at its default value (a test checks
   the starter file loads to exactly the defaults) and in USAGE's table of
   setting names.

## When you add a code language

1. Add a variant to `CodeLang` (`src/words/mod.rs`) and a snippet pool in
   `src/words/quotes.rs`; match it in `random_code_snippet`.
2. Add its names to `code_lang` in `src/config/load.rs` (this one table
   serves both `code_lang` in the config and `--code`).
3. Add a `start("code", …)` entry to `build_menu()`.

---

## When you add a new theme

1. Declare a `pub const` of type `ThemePalette` in `src/theme/builtin.rs`.
   Fill every slot (`accent`, `secondary`, `correct`, `incorrect`, `pending`,
   `mode_tag`, `error`, `neutral`, `background`). Every text color needs at
   least 4.5:1 contrast against the background, and `pending` (untyped text)
   must stay dimmer than `correct` and `neutral` — the
   `rgb_themes_meet_contrast_minimum` test checks both.
2. Append `("name", YOUR_THEME)` to `ALL` in the same file.
3. That's it. The CLI loader (`--theme <name>`), the config loader (`theme =
   "..."`), and `--list-themes` all iterate `ALL` — no other wiring needed.

If you want the theme to look identical regardless of terminal, set
`background` to a concrete `Color::Rgb(...)`. If you'd rather it inherit
the user's terminal background, set `background = Color::Reset`.
