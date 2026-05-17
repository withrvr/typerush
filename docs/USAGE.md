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
