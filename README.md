# TypeRush

**Type faster. Right in your terminal.**

[![Crates.io](https://img.shields.io/crates/v/typerush)](https://crates.io/crates/typerush)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Build](https://img.shields.io/github/actions/workflow/status/withrvr/typerush/release.yml?label=build)](https://github.com/withrvr/typerush/actions)

```
████████ ██    ██ ██████  ███████ ██████  ██    ██ ███████ ██   ██
   ██     ██  ██  ██   ██ ██      ██   ██ ██    ██ ██      ██   ██
   ██      ████   ██████  █████   ██████  ██    ██ ███████ ███████
   ██       ██    ██      ██      ██   ██ ██    ██      ██ ██   ██
   ██       ██    ██      ███████ ██   ██  ██████  ███████ ██   ██
```

## 👀 Preview

![TypeRush demo](https://raw.githubusercontent.com/withrvr/typerush/main/assets/demo.gif)

TypeRush is a beautiful, fast typing-speed trainer that runs **entirely in your terminal**.
No browser. No internet. No installer. Just one tiny binary, ready in a single command.

Whether you want to break your personal best, practise code typing, or just unwind
with a quote — TypeRush meets you where you already work: the command line.

---

## ✨ Why TypeRush

- 🚀 **Instant feedback** — see your WPM and accuracy climb keystroke by keystroke
- 🎨 **Themes built-in** — `dark`, `light`, `monokai`, `dracula`, or roll your own colors
- ⏱ **Eight built-in modes** — Time, Words, Quote, Code, Symbols, Zen, Custom-file, and a Daily challenge
- 🌅 **Daily challenge** — the same seed-of-the-day word list for everyone, every day
- ⏸ **Pause / resume & replay** — `Ctrl+P` freezes the clock; replay any recent session word-for-word
- 🔤 **Seven code languages** — Rust, Python, JavaScript, Go, Java, SQL, Shell
- 📚 **10k-word pool & punctuation/number drills** — opt in when you want a tougher session
- 🗂  **Personal snippet library** — drop `.txt` files in `~/.typerush/snippets/`
- 💾 **Tracks your progress** — every session saved locally, with charts and a personal best
- 🌍 **Runs everywhere** — Linux · macOS · Windows · WSL · Git Bash
- 📦 **One binary, zero setup** — `cargo install typerush` and you're done

---

## 🚀 Install

```bash
cargo install typerush
```

That's it. Run `typerush` and start typing.

> Don't have Rust? Install it in 30 seconds: [https://rustup.rs](https://rustup.rs)

Pre-built binaries for Linux, macOS, and Windows are also available on the
[Releases page](https://github.com/withrvr/typerush/releases) — download, unzip, run.

---

## 🎮 Quick start

Open the interactive menu:

```bash
typerush
```

Or jump straight into a mode:

```bash
typerush --time 30            # 30-second timed sprint
typerush --words 50           # 50 random English words
typerush --quote              # one programming quote
typerush --code rust          # rust | python | js | go | java | sql | shell
typerush --symbols 25         # programming-punctuation drill
typerush --daily              # today's challenge — same words for everyone
typerush --zen                # zen mode — no timer, no pressure
typerush --file my_text.txt   # type anything you want
typerush --big                # use the 10,000-word English pool
typerush --punctuation        # sprinkle punctuation onto random words
typerush --numbers            # mix number tokens into random words
typerush --theme monokai      # try a different theme
typerush --list-themes        # see all built-in themes
typerush --list-snippets      # see every snippet in ~/.typerush/snippets/
typerush --export-csv out.csv # export your stats history as CSV
typerush --init-config        # write a starter config to ~/.typerush/
typerush config set theme dracula   # edit the config without opening it
```

---

## 🎯 Modes at a glance

| Mode    | What it's for                                                       |
| ------- | ------------------------------------------------------------------- |
| Time    | Sprint! Type as many words as you can in 15/30/60/120s              |
| Words   | Hit a fixed target — 10, 25, 50, or 100 words                       |
| Quote   | Famous programming quote, one round                                 |
| Code    | Real Rust / Python / JS / Go / Java / SQL / Shell snippets          |
| Symbols | Drill punctuation: `(){};=>`, `&&`, `?.`, `<T>` and friends         |
| Zen     | No timer, no score. Just type and breathe.                          |
| Custom  | Any text file via `--file` or `.txt` in `~/.typerush/snippets/`     |
| Daily   | Deterministic seed-of-the-day words — race the same list as everyone |

---

## ⌨️ A few keys to know

| Key        | Action                          |
| ---------- | ------------------------------- |
| `Enter`    | start / restart a session       |
| `Esc`      | back to menu / end session      |
| `Ctrl+P`   | pause / resume mid-session      |
| `Ctrl+R`   | restart the current mode        |
| `p`        | replay the finished session     |
| `Tab`      | jump to your stats history      |
| `?`        | open the keybindings overlay    |
| `Ctrl+C`   | quit                            |

Full keymap: see [`docs/USAGE.md`](docs/USAGE.md).

---

## 📈 Track your progress

Every (non-zen) session is saved to `~/.typerush/stats.json`. The Stats screen
shows your personal best, average accuracy, a trend sparkline, and your last
10 runs — plus, as of v0.3:

- 🔥 **Daily streak** — consecutive days you've practiced
- 📅 **7- and 30-day rolling WPM averages**
- ⌨️ **Per-key accuracy heatmap** — find the keys that slow you down
- 🏅 **Per-mode personal bests** — a separate record for `time-30s`, `words-50`, etc.

And as of v0.5: **replay** any of your recent sessions word-for-word straight
from the Stats screen, and export the whole history with
`typerush --export-csv`.

---

## 🎨 Make it yours

Drop a [`config.toml`](config.example.toml) into `~/.typerush/` to pick a
theme, set your default mode, or override individual colors:

```toml
theme = "monokai"

[defaults]
mode = "time"
time_seconds = 30
```

See [`docs/USAGE.md`](docs/USAGE.md#configuration--typerushconfigtoml) for
the full reference.

---

## 📚 More documentation

- 📖 [USAGE](docs/USAGE.md) — all keybindings, modes, CLI flags, config
- 🏛 [ARCHITECTURE](docs/ARCHITECTURE.md) — how the code is laid out
- 🛠 [DEVELOPMENT](docs/DEVELOPMENT.md) — local dev workflow, cargo-watch, sandbox config
- 🗺 [ROADMAP](docs/ROADMAP.md) — what's done, what's next
- 🤝 [CONTRIBUTING](CONTRIBUTING.md) — dev setup, conventions, how to help
- 📝 [CHANGELOG](CHANGELOG.md) — what changed in every release

---

## 💬 Get involved

- 🐞 Found a bug? [Open an issue](https://github.com/withrvr/typerush/issues/new/choose)
- 💡 Got an idea? [Request a feature](https://github.com/withrvr/typerush/issues/new/choose)
- ❤️ Like the project? Give it a star — it really helps!

---

## License

[MIT](LICENSE)
