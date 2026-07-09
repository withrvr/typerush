# TypeRush — Roadmap

What's done, what's next.

```
KEY:  ✅ done    🚧 in progress    🔲 planned    💡 idea
```

---

## ✅ Shipped (v0.1)

### Core engine
- ✅ Cross-platform terminal setup (raw mode + alt screen + panic hook)
- ✅ 100ms tick event loop
- ✅ Character-by-character matcher (`game.rs`)
- ✅ Live WPM + accuracy with the standard 5-char-per-word formula

### Modes
- ✅ Time mode (15 / 30 / 60 / 120 seconds)
- ✅ Words mode (10 / 25 / 50 / 100 words)
- ✅ Quote mode
- ✅ Code mode (Rust, Python, JavaScript)
- ✅ Zen mode (no timer, no stats)
- ✅ Custom file via `--file <path>`

### UI
- ✅ ASCII banner main menu
- ✅ Live colored typing screen (green/red/dim/extra)
- ✅ **Steady, non-blinking cursor** (no layout jitter)
- ✅ Progress gauge for word/time modes
- ✅ Results screen with WPM delta vs last session
- ✅ Stats history screen (table + WPM sparkline)
- ✅ Floating `?` help overlay
- ✅ Error modal for bad CLI inputs / missing files

### Persistence
- ✅ Auto-save sessions to `~/.typerush/stats.json`
- ✅ Personal best tracking
- ✅ Average accuracy

### Project hygiene
- ✅ Unit tests for the matcher
- ✅ GitHub Actions CI (Linux / macOS / Windows)
- ✅ Clippy clean with `-D warnings`
- ✅ Inline doc comments

---

## ✅ Shipped (v0.2 — Customization)

- ✅ `~/.typerush/config.toml` for themes and defaults
- ✅ Named-color + hex (`#RRGGBB`) theme support
- ✅ Default word count / time / mode picked from config
- ✅ Light theme
- ✅ Built-in themes: `dark`, `light`, `monokai`, `dracula`
- ✅ Per-slot color overrides on top of any built-in theme
- ✅ `--theme <name>` CLI override + `--list-themes` discovery flag

---

## ✅ Shipped (v0.3 — Smarter stats)

- ✅ Per-key accuracy heatmap (find your problem keys)
- ✅ Per-mode personal bests (separate PB for time-30s vs words-50)
- ✅ Daily streak counter
- ✅ Average WPM over the last 7 / 30 days
- ✅ Crash-safe (atomic) stats writes
- ✅ O(1) stats rendering via `aggregate.json` + in-memory session cache

---

## 🚧 In progress

- 🚧 Pre-built binary releases on GitHub
- 🚧 First publish to crates.io

---

## 🔲 Planned

### ✅ v0.4 — More content & menu UX
- ✅ Bigger English word pool (10k) — opt-in via `--big` or `words.pool = "extended"`
- ✅ Programming-symbols mode (focus on `(){};=>` etc.)
- ✅ More languages: Go, Java, SQL, Shell
- ✅ Punctuation / numbers toggle — `--punctuation`, `--numbers`, or the `[words]` section
- ✅ Custom snippet library: drop `.txt` files in `~/.typerush/snippets/`
- ✅ "Custom" menu row + last-picked-file memory in `~/.typerush/state.json`
- ✅ Visual section gaps in the main menu (Time / Words / Quote / Code / Symbols / Zen / Custom groups)

### v0.5 — Quality of life
- 🔲 Pause / resume mid-session
- 🔲 Replay a recent session word-for-word
- 🔲 Export stats as CSV
- 🔲 Daily challenge (deterministic seed-of-the-day)
- 🔲 `typerush config get/set/show/reset/path` CLI subcommands (edit `~/.typerush/config.toml` without opening it)
- 🔲 In-app Settings screen — theme picker, default mode picker, reset-to-defaults
- 🔲 `typerush --init-config` writes a starter `config.toml` to `~/.typerush/`

---

## 💡 Ideas (not committed)

- 💡 Multiplayer race over a LAN
- 💡 Per-finger heatmap (left vs right hand, weak fingers)
- 💡 Adaptive practice: re-roll words containing your worst keys
- 💡 Webhook to post your PB to Discord/Slack
- 💡 Voice-over for accessibility

Got an idea that should be on this list? [Open an issue](https://github.com/withrvr/typerush/issues/new/choose).
