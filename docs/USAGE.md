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
typerush --code rust          # code-typing: rust | python | js
typerush --zen                # zen mode (no timer, no stats)
typerush --file <path>        # type any text file you have
typerush --theme monokai      # one-shot theme override
typerush --list-themes        # print available theme names and exit
```

You can also pass `--file` alone to use the file inside the menu's "Custom" entry.

---

## Modes in detail

| Mode    | Description                                                          | Ends when…                       |
| ------- | -------------------------------------------------------------------- | -------------------------------- |
| Time    | Sprint — type as many words as possible in N seconds                  | the timer hits zero              |
| Words   | Type a fixed number of random words (10 / 25 / 50 / 100)              | you complete the last word       |
| Quote   | A randomly chosen famous programming quote                            | you complete the last word       |
| Code    | A real short code snippet (Rust, Python, or JavaScript)               | you complete the last word       |
| Zen     | Soft, monochrome UI. No timer, no WPM, no score saved.                | you press `Esc`                  |
| Custom  | Any whitespace-separated text file passed via `--file`                | you complete the last word       |

---

## Keybindings

### Everywhere

| Key      | Action                          |
| -------- | ------------------------------- |
| `?`      | Toggle keybindings overlay      |
| `Ctrl+C` | Quit immediately                |

### Main menu

| Key                | Action                            |
| ------------------ | --------------------------------- |
| `↑ / ↓` or `j / k` | Move highlight                    |
| `Enter`            | Start the selected mode           |
| `Tab`              | Jump to the historical stats view |
| `q`                | Quit                              |

### While typing

| Key              | Action                                          |
| ---------------- | ----------------------------------------------- |
| any printable    | Type that character                             |
| `Space`          | Submit the current word, advance to the next    |
| `Backspace`      | Delete the previous character                   |
| `Ctrl+Backspace` | Delete the entire current word                  |
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

Zen-mode sessions are intentionally **not** saved.

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

The 9 slots: `accent`, `secondary`, `correct`, `incorrect`, `pending`,
`extra`, `mode_tag`, `error`, `neutral`. Each maps to a specific UI element —
see `config.example.toml` for inline documentation.

### Defaults

Pre-select a menu row and starting mode:

```toml
[defaults]
mode = "time"            # time | words | quote | code | zen
time_seconds = 15        # 15 / 30 / 60 / 120 (matches menu rows)
word_count = 25          # 10 / 25 / 50 / 100
code_lang = "rust"       # rust | python | js
```

### Precedence

```
CLI flag (--theme, --time, …) > config.toml > built-in default
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
