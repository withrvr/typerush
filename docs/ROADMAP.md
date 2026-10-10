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
- ✅ Live colored typing screen (green/red/dim)
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
- ✅ `Ctrl+Backspace` deletes the current word (terminals that send `^H`, plus `Ctrl+W`)

---

## ✅ Shipped (v0.2.1 — Pure character checking)

- ✅ Typing is one stream of characters; the space between words is a character like any other
- ✅ A wrong key where a space belongs is one wrong character (shown as a red, underlined space) instead of piling up letters on the word ([#8](https://github.com/withrvr/typerush/issues/8))
- ✅ A space typed mid-word is a wrong character — it no longer jumps to the next word
- ✅ The run ends on the last character (no trailing space needed)
- ✅ Backspace walks back over spaces one character at a time

---

## ✅ Shipped (v0.3 — Smarter stats)

- ✅ Per-key accuracy heatmap (find your problem keys)
- ✅ Per-mode personal bests (separate PB for time-30s vs words-50)
- ✅ Daily streak counter
- ✅ Average WPM over the last 7 / 30 days
- ✅ Crash-safe (atomic, fsynced) stats writes; a corrupt `stats.json` is backed up, never wiped
- ✅ Stats screens never read `stats.json` per frame (in-memory session cache)

**Added to v0.3 beyond the original plan:**

- ✅ Main menu grouped by category (heading, options on the line below) with
  2-D arrow navigation — pulled forward from v0.4's "visual section gaps"
- ✅ Mouse support (click menu options and footer hints, wheel scrolls the
  menu) with every action also on the keyboard (WCAG 2.1.1) and
  release-to-activate clicks (WCAG 2.5.2)
- ✅ Alternate keys: `F1` help, `F5` restart, `Space` start, `s` stats

---

## ✅ Shipped (v0.4 — More content)

- ✅ Bigger English word pool (10k) — `--big` or `[words] pool = "extended"`
- ✅ Programming-symbols mode (focus on `(){};=>` etc.) — `symbols` menu row, `--symbols N`
- ✅ More languages: Go, Java, SQL, Shell
- ✅ Punctuation / numbers toggle — `--punctuation`, `--numbers`, or `[words]`;
  `p` / `n` / `b` in the menu, `--no-…` switches to turn them off
- ✅ Menu fits a standard 80×24 terminal (compact banner, `▲/▼ more` when it
  can't)
- ✅ A personal best per custom file (`custom-notes`)
- ✅ Custom snippet library: drop `.txt` files in `~/.typerush/snippets/` (`--list-snippets`)
- ✅ "custom" menu row + last-picked-file memory in `~/.typerush/state.json`

(The original v0.4 item "visual section gaps in the main menu" shipped early,
in v0.3, as the category-grouped menu.)

---

## 🚧 In progress

- 🚧 Pre-built binary releases on GitHub
- 🚧 First publish to crates.io

---

## 🔲 Planned

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
