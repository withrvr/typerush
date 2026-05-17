# TypeRush

A fast, cross-platform **typing speed trainer** that lives entirely in your terminal.
Like no browser, no internet, no Electron. Just one Rust binary.

```
████████ ██    ██ ██████  ███████ ██████  ██    ██ ███████ ██   ██
   ██     ██  ██  ██   ██ ██      ██   ██ ██    ██ ██      ██   ██
   ██      ████   ██████  █████   ██████  ██    ██ ███████ ███████
   ██       ██    ██      ██      ██   ██ ██    ██      ██ ██   ██
   ██       ██    ██      ███████ ██   ██  ██████  ███████ ██   ██
```

## Features

- **Live WPM + accuracy** updates every 100ms as you type
- **Character-by-character feedback** — green = correct, red = wrong, gray = pending
- **Six modes**: Time (15/30/60/120s), Words (10/25/50/100), Quote, Code (Rust/Python/JS), Zen, Custom file
- **Persistent stats** — every session saved to `~/.typerush/stats.json`
- **History view** — recent sessions, personal best, WPM trend sparkline
- **Cross-platform** — Linux, macOS, Windows (CMD/PowerShell), WSL, Git Bash
- **Single binary** — no runtime deps, no GUI toolkit
- **Fast** — sub-5ms render loop, zero lag between keystroke and feedback

## Install

```bash
cargo install typerush
```

Or build from source:

```bash
git clone https://github.com/withrvr/typerush
cd typerush
cargo install --path .
```

## Usage

```bash
# Open the interactive menu
typerush

# Or skip the menu and jump straight in
typerush --time 30           # 30-second timed test
typerush --words 50          # type exactly 50 random words
typerush --quote             # one programming quote
typerush --code rust         # a Rust code snippet
typerush --zen               # zen mode — no timer, no stats
typerush --file my_text.txt  # use a custom text file
```

## Keybindings

| Key                | Action                                |
|--------------------|---------------------------------------|
| `↑/↓` or `j/k`     | navigate menu                         |
| `Enter`            | start / restart                       |
| `Esc`              | back to menu / end session            |
| `Ctrl+R`           | restart current mode                  |
| `Backspace`        | delete previous character             |
| `Ctrl+Backspace`   | delete previous word                  |
| `Tab`              | jump to stats screen                  |
| `Ctrl+C`           | quit immediately                      |
| `?`                | toggle keybindings overlay            |

## Modes

| Mode    | Description                                       |
|---------|---------------------------------------------------|
| Time    | Type as many words as possible in 15/30/60/120s   |
| Words   | Type exactly 10/25/50/100 random English words    |
| Quote   | A single famous programming quote                 |
| Code    | A real Rust / Python / JavaScript snippet         |
| Zen     | No timer, no WPM. Just type. Esc when done.       |
| Custom  | Pipe in any file: `typerush --file path.txt`      |

## Stats

Every non-zen session is appended to `~/.typerush/stats.json`. The Stats screen
(reachable via `Tab` from the menu or `s` from the results screen) shows:

- All-time best WPM
- Average accuracy across all sessions
- WPM trend sparkline (last 20 sessions)
- Table of your 10 most recent runs

## WPM Formula

TypeRush uses this standard formula:

```
wpm = (correct_chars / 5) / elapsed_minutes
accuracy = correct_chars / total_typed_chars * 100
```

Where "1 word = 5 characters". Live WPM updates every 100ms.

## Development

```bash
cargo run                 # debug build
cargo run -- --time 30    # with args
cargo test                # run tests
cargo clippy              # lint
cargo build --release     # optimized binary
```

### Project structure

```
src/
├── main.rs          entry point, event loop, CLI args
├── app.rs           App state, game flow, mode logic
├── game.rs          character matching engine
├── storage.rs       JSON persistence for sessions
├── ui/
│   ├── mod.rs       screen dispatcher
│   ├── menu.rs      main menu
│   ├── typing.rs    the typing game
│   ├── results.rs   end-of-session screen
│   ├── stats.rs     history + sparkline
│   └── help.rs      keybindings overlay
└── words/
    ├── mod.rs       word selection
    ├── english.rs   built-in English word lists
    └── quotes.rs    programming quotes + code snippets
```

## License

MIT
