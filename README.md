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

<p align="center">
  <video src="assets/demo.mp4" autoplay loop muted playsinline width="700"></video>
</p>

TypeRush is a beautiful, fast typing-speed trainer that runs **entirely in your terminal**.
No browser. No internet. No installer. Just one tiny binary, ready in a single command.

Whether you want to break your personal best, practise code typing, or just unwind
with a quote — TypeRush meets you where you already work: the command line.

---

## ✨ Why TypeRush

- 🚀 **Instant feedback** — see your WPM and accuracy climb keystroke by keystroke
- 🎨 **Beautiful TUI** — clean colors, smooth layout, no distractions
- ⏱ **Six built-in modes** — Time, Words, Quote, Code, Zen, and Custom-file
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
typerush --time 30           # 30-second timed sprint
typerush --words 50          # 50 random English words
typerush --quote             # one programming quote
typerush --code rust         # a real Rust snippet
typerush --zen               # zen mode — no timer, no pressure
typerush --file my_text.txt  # type anything you want
```

---

## 🎯 Modes at a glance

| Mode    | What it's for                                          |
| ------- | ------------------------------------------------------ |
| Time    | Sprint! Type as many words as you can in 15/30/60/120s |
| Words   | Hit a fixed target — 10, 25, 50, or 100 words          |
| Quote   | Famous programming quote, one round                    |
| Code    | Real Rust / Python / JavaScript snippets               |
| Zen     | No timer, no score. Just type and breathe.             |
| Custom  | Use any text file you have                             |

---

## ⌨️ A few keys to know

| Key        | Action                          |
| ---------- | ------------------------------- |
| `Enter`    | start / restart a session       |
| `Esc`      | back to menu / end session      |
| `Ctrl+R`   | restart the current mode        |
| `Tab`      | jump to your stats history      |
| `?`        | open the keybindings overlay    |
| `Ctrl+C`   | quit                            |

Full keymap: see [`docs/USAGE.md`](docs/USAGE.md).

---

## 📈 Track your progress

Every (non-zen) session is saved to `~/.typerush/stats.json`. The Stats screen
shows your personal best, average accuracy, a trend sparkline, and your last
10 runs.

---

## 📚 More documentation

- 📖 [USAGE](docs/USAGE.md) — all keybindings, modes, CLI flags, config
- 🏛 [ARCHITECTURE](docs/ARCHITECTURE.md) — how the code is laid out
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
