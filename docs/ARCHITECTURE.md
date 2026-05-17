# TypeRush — Architecture

This is the map of the source tree. Read it before making non-trivial changes.

---

## Big picture

TypeRush is a single-process TUI app with one event loop, one mutable `App`
state object, and a stateless renderer. The flow per frame:

```
       ┌──────────────────────────────┐
       │       main.rs::run_app       │
       │                              │
       │   1. terminal.draw(...)      │
       │   2. event::poll  (100ms)    │
       │   3. handle_key(...)         │
       │   4. app.tick()              │
       │   5. save session on enter   │
       │      to Results screen       │
       └──────────────┬───────────────┘
                      │  reads & mutates
                      ▼
              ┌──────────────┐
              │     App      │  ← the single source of truth
              │ (src/app.rs) │
              └──────┬───────┘
                     │  read-only
                     ▼
              ┌──────────────┐
              │   ui::*      │  ← pure renderer, no state of its own
              │ (src/ui/...) │
              └──────────────┘
```

---

## State machine

The `Screen` enum in `src/app.rs` is the application's high-level state:

```
[Menu] ──Enter──► [Typing] ──finish / Esc──► [Results]
   ▲                                            │
   │                                            │
   │ ◄────── Esc / 'm' ─────────────────────────┘
   │
   ├──── Tab ───► [Stats] ──── Esc ────► [Menu]
   │
   └──── ? ─────► [Help]   (overlay; remembers prev screen)
```

Every state transition happens by setting `app.screen` from a keymap handler in
`main.rs`.

---

## Data flow per keystroke

```
crossterm::event::read()
    │
    ▼
main.rs::handle_key  (matches on app.screen → per-screen handler)
    │
    ▼  (e.g. for Screen::Typing)
app.rs::handle_char / handle_backspace
    │
    │   mutates:
    │     - words[current].typed
    │     - correct_chars
    │     - total_typed_chars
    │     - current_word (on space)
    │     - screen → Results (when mode completes)
    ▼
back to main loop ──► next frame is drawn from the new state
```

---

## Module responsibilities

```
src/
├── main.rs              Entry point. Owns the terminal, the event loop,
│                        the panic hook, and the CLI parser (clap).
│
├── app.rs               The App state machine. All fields, all transitions,
│                        WPM / accuracy math, mode-switching, time/word
│                        completion logic.
│
├── game.rs              Pure function: get_char_states(target, typed) — the
│                        character-by-character matcher that drives all
│                        colored feedback. Unit-tested here.
│
├── storage.rs           SessionRecord struct + read/write of
│                        ~/.typerush/stats.json. Personal best & average
│                        accuracy helpers.
│
├── ui/
│   ├── mod.rs           render() dispatcher (matches on app.screen)
│   ├── menu.rs          Main menu with the ASCII banner
│   ├── typing.rs        The typing screen: header, progress gauge, words
│   ├── results.rs       Post-session screen with PB delta + sparkline
│   ├── stats.rs         History view: summary, sparkline, recent table
│   └── help.rs          Floating ? overlay and error modal
│
└── words/
    ├── mod.rs           Word source picker (random, quote, code, file)
    ├── english.rs       Built-in 200- and 1000-word English pools
    └── quotes.rs        Programming quotes + Rust/Python/JS snippets
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
every 500ms. The current design renders a **steady** cursor:

- on a character → REVERSED style (block fill)
- past the last character → an underlined trailing space (plain bar)

The trailing space is always part of the layout, so toggling its style never
changes width.

### Stats persistence
JSON, flat array, no schema. The file is small enough (one session ≈ 200B)
that we rewrite it whole on every save. Zen-mode sessions are deliberately
excluded.

### Cross-platform
crossterm handles Windows Console API, ANSI escape codes, and raw mode in one
crate. We don't directly use any platform-specific code, so the binary is a
"just build it" affair on any of Linux, macOS, Windows, or WSL.

### Error handling
- Setup / teardown errors propagate (`anyhow::Result`).
- A `panic::set_hook` resets the terminal before the panic message prints, so a
  crash never leaves the user's shell broken.
- Disk I/O for stats is best-effort — losing a record is never worth crashing.

---

## Performance notes

- Render loop runs at 10 fps (100ms tick). The actual `terminal.draw` cost is
  dominated by ratatui's diff algorithm — only changed cells are repainted.
- `get_char_states` is O(n) in word length and is called once per visible word
  per frame. For a 100-word session that's still a microsecond-level cost.
- No allocations in the hot path beyond what ratatui itself does for spans.

---

## When you add a new mode

1. Add a variant to `Mode` in `src/app.rs`.
2. Pattern-match it inside `App::start_game` to pick a word source.
3. Update `Mode::label` so it persists nicely in stats.
4. Add a row to `default_menu()`.
5. If it has a unique completion condition, handle it in `submit_word()` /
   `tick()`.
6. (Optional) Add a CLI flag in `main.rs::Cli`.
