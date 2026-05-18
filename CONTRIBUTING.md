# Contributing to TypeRush

Thanks for your interest in TypeRush! This document covers everything you need
to know to make a change — from cloning the repo to opening a pull request.

---

## TL;DR

```bash
git clone https://github.com/withrvr/typerush
cd typerush
cargo run                       # try it
cargo test                      # run the tests
cargo clippy -- -D warnings     # lint
```

If your change builds, passes tests, and is clippy-clean — open a PR. We'll
take it from there.

---

## Project layout

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the full breakdown.
The TL;DR:

- `src/main.rs` — entry point + event loop
- `src/app.rs` — application state machine
- `src/game.rs` — character matcher
- `src/storage.rs` — JSON stats persistence
- `src/ui/*.rs` — one file per screen
- `src/words/*.rs` — word sources

---

## Development setup

### Prerequisites

You need:

- **Rust 1.75 or newer.** Install via [rustup](https://rustup.rs).
- A terminal that supports 256 colors / UTF-8 (anything modern works).

Optional but useful:

```bash
cargo install cargo-watch       # auto-rerun on save
cargo install cargo-expand      # expand macros, occasionally handy
```

### First build

```bash
git clone https://github.com/withrvr/typerush
cd typerush
cargo build
```

The first build pulls in ~40 crates and takes ~60 seconds. Subsequent builds
are incremental and finish in under a second.

### Running during development

For the full local-dev workflow — running with CLI args, debug vs release,
testing the config file safely, `cargo-watch` patterns for TUI apps, the
recommended two-terminal loop — see [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).

Quick reference:

```bash
cargo run                       # debug build, opens the menu
cargo run -- --time 30          # pass any CLI flag through
cargo build --release && ./target/release/typerush
```

---

## Coding conventions

### Style
- **rustfmt:** run `cargo fmt` before committing.
- **clippy:** keep it clean. CI fails on warnings.
- **Comments:** explain *why*, not *what*. Variable and function names should
  already explain *what*.
- **Doc comments (`///`)** on every public item (struct, fn, enum). Module-
  level `//!` docs on every file.

### Naming
- Variables: `snake_case`, full words. `active_word`, not `aw`.
- Functions: verb phrases. `start_game`, `handle_backspace`.
- Booleans: `is_…`, `has_…`. `is_active_word`, `cursor_past_word_end`.

### Tests
- Unit tests live in the same file as the code they cover, inside `#[cfg(test)] mod tests`.
- Prefer cheap deterministic tests (the matcher in `game.rs` is a good
  example).

### Error handling
- Use `anyhow::Result` for fallible operations that bubble up to `main`.
- For things that "should never fail" (e.g. dirs::home_dir), fall back to a
  sensible default — never panic in user-facing code.
- File I/O in the stats path is best-effort: losing a stat row is fine; never
  crash the typing app.

---

## Commits

We follow a light flavour of [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>: <imperative summary>

<optional body>
```

Common types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`.

Examples:

```
feat: add per-key accuracy heatmap to stats screen
fix: cursor no longer shifts layout at end of word
docs: clarify zen mode behaviour in usage guide
```

Keep one logical change per commit when you can — it makes review easier.

---

## Submitting a pull request

1. Fork the repo and create a branch off `main`.
2. Make your change. Add or update tests if relevant.
3. Run the full check:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   ```
4. Push and [open a PR](https://github.com/withrvr/typerush/pulls).
5. Use the PR template — it asks the questions reviewers care about.

CI runs the same checks on Linux, macOS, and Windows. A green CI is required
to merge.

---

## Reporting bugs / requesting features

Use the [Issues](https://github.com/withrvr/typerush/issues/new/choose) page.
Templates exist for:

- 🐞 **Bug report** — something broken
- 💡 **Feature request** — something missing
- ❓ **Question** — how do I…?

Please search existing issues before opening a new one.

---

## Adding a new mode

The architecture doc has a step-by-step recipe — see "When you add a new mode"
in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

Quick version: add a `Mode` variant, plug it into `App::start_game`, add a
menu row, optionally add a CLI flag.

---

## Adding a new theme

(Once theming lands in v0.2.) Each theme is a `Theme` struct in
`src/config.rs` mapping the four character states (Correct / Incorrect /
Pending / Extra) to ratatui colors. The plan is to read these from
`~/.typerush/config.toml`. Until then, hard-coded constants live in
`src/ui/typing.rs::style_for_char`.

---

## Code of Conduct

Be kind. Assume good faith. Disagreement is fine, hostility isn't. Maintainers
reserve the right to lock or remove anything that crosses the line.

---

## License

By contributing you agree that your work is licensed under the same MIT
license that covers the rest of the project.
