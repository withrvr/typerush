# TypeRush — Usage Guide

Everything you can do with TypeRush, in one place.

---

## Running it

Open the interactive menu:

```bash
typerush
```

Or skip the menu and start a session directly:

```bash
typerush --time 30            # 30-second timed test (15 / 30 / 60 / 120)
typerush --words 50           # type exactly N words
typerush --quote              # one programming quote
typerush --code rust          # code-typing: rust | python | js | go | java | sql | shell
typerush --zen                # zen mode (no timer, no stats)
typerush --symbols 25         # symbols drill (N tokens; standard rows: 25 / 50)
typerush --file <path>        # type any text file you have
typerush --big                # use the larger 10,000-word English pool
typerush --punctuation        # mix punctuation marks into random words
typerush --numbers            # mix random number tokens into random words
typerush --theme monokai      # one-shot theme override
typerush --list-themes        # print available theme names and exit
typerush --list-snippets      # print every snippet found in ~/.typerush/snippets/
```

You can pass `--file <path>` alone to use the file inside the menu's "Custom"
entry. TypeRush remembers the last path you typed against in
`~/.typerush/state.json`, so the "Custom" menu row stays useful even after the
flag is gone.

To build a personal snippet library, drop `.txt` files into
`~/.typerush/snippets/`. Each file appears as a `Snippet · <name>` row in the
main menu and is loaded as the typing source when selected.

---

## Modes in detail

| Mode    | Description                                                          | Ends when…                       |
| ------- | -------------------------------------------------------------------- | -------------------------------- |
| Time    | Sprint — type as many words as possible in N seconds                  | the timer hits zero              |
| Words   | Type a fixed number of random words (10 / 25 / 50 / 100)              | you complete the last word       |
| Quote   | A randomly chosen famous programming quote                            | you complete the last word       |
| Code    | A real short code snippet (Rust, Python, JavaScript, Go, Java, SQL, Shell) | you complete the last word  |
| Symbols | Programming punctuation drill — type N short tokens like `=>` `(){};` | you complete the last token      |
| Zen     | Soft, monochrome UI. No timer, no WPM, no score saved.                | you press `Esc`                  |
| Custom  | Any whitespace-separated text file passed via `--file` or picked from `~/.typerush/snippets/` | you complete the last word |

---

## Keybindings

### Everywhere

| Key      | Action                          |
| -------- | ------------------------------- |
| `?`      | Toggle keybindings overlay      |
| `Ctrl+C` | Quit immediately                |

### Main menu

| Key                | Action                                                          |
| ------------------ | --------------------------------------------------------------- |
| `↑ / ↓` or `j / k` | Move highlight (decorative section headers are skipped)         |
| `Enter`            | Start the selected mode / open the selected snippet             |
| `Tab`              | Jump to the historical stats view                               |
| `q`                | Quit                                                            |

The menu is grouped into visual sections — Time, Words, Quote, Code, Symbols,
Zen, Custom, More — separated by muted `── Section ──` headers. Arrow-key
navigation hops over headers so you always land on a selectable row.

### While typing

| Key              | Action                                          |
| ---------------- | ----------------------------------------------- |
| any printable    | Type that character                             |
| `Space`          | Submit the current word, advance to the next    |
| `Backspace`      | Delete the previous character                   |
| `Ctrl+Backspace` | Delete the entire current word                  |
| `Ctrl+W`         | Same — delete the entire current word            |
| `Ctrl+H`         | Same — most terminals send this when you press `Ctrl+Backspace` |
| `Ctrl+R`         | Restart the same mode with a new word list      |
| `Esc`            | End the session and go to the results screen    |

### Results screen

| Key             | Action                            |
| --------------- | --------------------------------- |
| `Enter` / `r`   | Restart the same mode             |
| `Tab` / `s`     | Open the stats history            |
| `m` / `Esc`     | Back to the menu                  |
| `q`             | Quit                              |

### Stats history

| Key                | Action            |
| ------------------ | ----------------- |
| `m` / `Esc` / `Tab`| Back to the menu  |
| `q`                | Quit              |

---

## How WPM is calculated

TypeRush uses the standard "5 characters = 1 word" formula:

```
wpm      = (correct_chars / 5) / minutes_elapsed
accuracy = (correct_chars / total_typed_chars) * 100
```

Only characters you typed **at the correct position** count as `correct_chars`.
Typing extra characters past the end of a word counts toward total typed (so it
hurts accuracy) but never toward correct chars.

---

## Where your stats live

`~/.typerush/stats.json` — a plain JSON array. Back it up if you care about your
history; delete it if you want a fresh start.

`~/.typerush/aggregate.json` — a small running tally of per-key accuracy totals,
used to render the key-accuracy heatmap quickly without rescanning your whole
history. It's derived data: delete it and TypeRush rebuilds it from
`stats.json` on the next launch.

Zen-mode sessions are intentionally **not** saved.

Alongside the stats file, TypeRush keeps:

| File                            | Purpose                                                  |
| ------------------------------- | -------------------------------------------------------- |
| `~/.typerush/stats.json`        | One JSON record per completed session.                   |
| `~/.typerush/aggregate.json`    | Pre-computed per-key hit/miss totals — O(1) Stats reads. |
| `~/.typerush/state.json`        | Tiny UI state: the last `--file` path you typed against. |
| `~/.typerush/config.toml`       | Optional user configuration (themes, defaults, words).   |
| `~/.typerush/snippets/*.txt`    | Personal snippet library (each `.txt` is one menu row).  |

### What the Stats screen shows (v0.3.0+)

Open the Stats screen from the menu (`Tab`) or results screen (`s`).

| Section | What it shows |
| ------- | ------------- |
| **Summary (left)** | All-time best WPM · Average accuracy · Session count · Last WPM · **Daily streak** · **7-day avg WPM** · **30-day avg WPM** |
| **Key accuracy (right)** | Up to 5 of your worst keys (≥ 3 presses). Colour-coded: red < 80%, amber < 93%, green otherwise. Shows `no key data yet` until enough data is collected. |
| **WPM trend** | Sparkline of your last 20 sessions |
| **Recent sessions** | Last 10 sessions with date, mode, WPM, accuracy, and time |

### Per-mode personal bests

On the **Results screen**, the "best" line now shows your personal best
specifically for the mode you just finished (e.g. `time-30s best: 78.4 wpm`).
A `★ new best!` badge appears when you beat your previous record for that mode.

---

## Configuration — `~/.typerush/config.toml`

The config file is **entirely optional**. Without it, TypeRush boots with the
`dark` theme and a 15-second time-mode pre-selected.

A complete example lives at [`config.example.toml`](../config.example.toml) at
the project root — copy it to `~/.typerush/config.toml` and edit.

### Themes

Pick a built-in palette by name:

```toml
theme = "monokai"
```

Built-in themes (run `typerush --list-themes` to print them):

| Name      | Style                                              |
| --------- | -------------------------------------------------- |
| `dark`    | Default — cyan / green / yellow on a dark terminal |
| `light`   | Softer palette for light-background terminals      |
| `monokai` | Classic Sublime/TextMate — pink/green/yellow       |
| `dracula` | Purple/pink/cyan on `#282a36`                      |

### Per-slot color overrides

Override any slot of the chosen theme:

```toml
theme = "dracula"

[colors]
accent = "#FF00FF"      # hex
correct = "green"       # or ANSI name
```

The 10 slots: `accent`, `secondary`, `correct`, `incorrect`, `pending`,
`extra`, `mode_tag`, `error`, `neutral`, `background`. Each maps to a
specific UI element — see `config.example.toml` for inline documentation.

> **Background.** The `dark` theme uses `Color::Reset` for `background` so it
> picks up whatever your terminal's native background is — same look as v0.1.
> The `light`, `monokai`, and `dracula` themes paint their canonical
> backgrounds so the theme looks the same regardless of terminal. Set
> `background = "reset"` in `[colors]` if you'd rather one of those themes
> use your terminal background too.

### Defaults

Pre-select a menu row and starting mode:

```toml
[defaults]
mode = "time"            # time | words | quote | code | zen | symbols
time_seconds = 15        # 15 / 30 / 60 / 120 (matches menu rows)
word_count = 25          # 10 / 25 / 50 / 100
code_lang = "rust"       # rust | python | js | go | java | sql | shell
symbol_count = 25        # 25 / 50 (matches Symbols menu rows)
```

### Words section (v0.4.0+)

Tune the word source for Time / Words / Zen modes:

```toml
[words]
pool = "common"          # "common" (≈1k, default) | "extended" (10k)
punctuation = false      # attach commas, periods, quotes to ~25% of words
numbers = false          # replace ~12% of slots with random 1–4 digit numbers
```

The `--big`, `--punctuation`, and `--numbers` CLI flags override these
per-launch. The decoration toggles never apply in Zen mode (the screen stays
calm by design).

### Snippets library (v0.4.0+)

Drop `.txt` files into `~/.typerush/snippets/` to build your own typing
library. Each file appears as a `Snippet · <name>` row in the main menu and is
sourced as the typing target when selected. Files are discovered in
alphabetical order so menu positions are stable across launches. Use
`typerush --list-snippets` to print every snippet TypeRush has found.

### Precedence

```
CLI flag (--theme, --time, --big, …) > config.toml > built-in default
```

A malformed config file does not crash TypeRush — it falls back to defaults
and shows one error modal you can dismiss with any key.

---

## Terminal compatibility

| Terminal        | Status   |
| --------------- | -------- |
| WSL (Ubuntu)    | ✅ Great |
| Windows CMD     | ✅ Works |
| PowerShell      | ✅ Great |
| macOS Terminal  | ✅ Works |
| iTerm2          | ✅ Great |
| Alacritty       | ✅ Great |
| Warp            | ✅ Great |

If colors look off on Linux/WSL, set:

```bash
export TERM=xterm-256color
export COLORTERM=truecolor
```
