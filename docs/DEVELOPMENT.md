# TypeRush — Development Guide

Practical local-dev workflow for iterating on TypeRush. For project layout,
coding conventions, commit style and PR flow see
[`CONTRIBUTING.md`](../CONTRIBUTING.md). For runtime behavior and the config
schema see [`USAGE.md`](USAGE.md).

---

## Running the app from source

```bash
cargo run                       # debug build, opens the menu
```

Pass CLI flags **after a `--` separator** — that's what tells cargo to forward
the rest to typerush instead of consuming them itself:

```bash
cargo run -- --help
cargo run -- --time 30
cargo run -- --theme monokai
cargo run -- --list-themes
cargo run -- --file ./snippets/lorem.txt
```

### Debug vs release

| Profile | Compile speed | Runtime speed | When to use |
|---------|---------------|---------------|-------------|
| `cargo run` (debug) | fast (<1s incremental) | slower | day-to-day code edits |
| `cargo run --release` | slower (~30s clean) | optimized | feels like the real installed app, screenshots, perf checks |

```bash
cargo run --release -- --theme dracula
```

### Running the compiled binary directly

After a build, the binary sits at one of these paths — you can invoke it
without going through cargo:

```bash
./target/debug/typerush --theme monokai
./target/release/typerush --theme monokai       # after `cargo build --release`
```

---

## Testing the config-file flow safely

The loader reads `~/.typerush/config.toml`. To experiment without touching
your real config, **redirect `$HOME` for one command** — typerush will then
look at a sandbox directory you control:

```bash
mkdir -p /tmp/tr-test/.typerush
cp config.example.toml /tmp/tr-test/.typerush/config.toml
# edit /tmp/tr-test/.typerush/config.toml as you like, then:
HOME=/tmp/tr-test cargo run
```

Use the release binary the same way:

```bash
HOME=/tmp/tr-test ./target/release/typerush --theme monokai
```

Clean up afterwards:

```bash
rm -rf /tmp/tr-test
```

Your real `~/.typerush/` (stats + any config you keep) stays untouched.

### Quickly testing malformed configs

The loader is supposed to fall back gracefully instead of crashing. To verify:

```bash
echo "this is not valid toml = {{" > /tmp/tr-test/.typerush/config.toml
HOME=/tmp/tr-test cargo run
```

You should land on the menu with an error modal you can dismiss with any key.

---

## Running tests

```bash
cargo test                      # every test
cargo test theme                # only theme::* tests (substring filter)
cargo test game::tests::all_correct   # one specific test
cargo test -- --nocapture       # show println! output from tests
cargo test -- --test-threads=1  # run sequentially (rare, for flaky debug)
```

Filters are case-sensitive substrings against `module::test_name`.

---

## Quality gates (mirror CI)

Run these before committing — CI runs the exact same set on Linux, macOS,
and Windows:

```bash
cargo build                     # type-check
cargo test                      # unit tests
cargo clippy -- -D warnings     # lints (warnings → errors)
cargo fmt --check               # formatting
```

To auto-fix formatting:

```bash
cargo fmt
```

---

## Auto-rebuild on save (optional)

[`cargo-watch`](https://crates.io/crates/cargo-watch) re-runs a command every
time a tracked file changes. Install it once:

```bash
cargo install cargo-watch       # global tool — not a project dep
```

### Patterns that work well

```bash
cargo watch -x test                                     # rerun tests on save
cargo watch -x check                                    # type-check only
cargo watch -x test -x 'clippy -- -D warnings'          # chained
cargo watch -x 'run -- --theme monokai'                 # rerun the app (see caveat)
```

### Caveat for TUI apps

`cargo watch -x run` keeps **re-launching** the binary each time you save —
which fights you for the terminal and kills any session in progress. For
TypeRush you almost always want to keep `cargo watch` on tests / lints and
launch the app **manually** in another terminal.

### The recommended two-terminal loop

```bash
# Terminal 1 — keep gates green on every save
cargo watch -x test -x 'clippy -- -D warnings'

# Terminal 2 — run the app yourself when you want to try a change
cargo run -- --theme monokai
# Ctrl+C to exit, ↑ + Enter in the shell to re-run
```

This separation lets you keep the test feedback loop tight without losing the
typing session every time you save a UI tweak.

---

## A practical loop for a few common tasks

### Tweaking a theme palette

```bash
# Terminal 1
cargo watch -x test

# Terminal 2
cargo run --release -- --theme monokai
# edit src/theme/builtin.rs, Ctrl+C, re-run
```

Release profile makes the colors look the same as the installed app.

### Testing config + theme together

```bash
HOME=/tmp/tr-test cargo run --release
# edit /tmp/tr-test/.typerush/config.toml in a third terminal, Ctrl+C, re-run
```

### Debugging a wrong-character behavior

```bash
cargo test game::                  # the matcher tests
cargo test -- --nocapture          # if you've added eprintln!s
```

---

## Cleaning up

```bash
cargo clean                     # nuke ./target — frees several hundred MB
rm -rf /tmp/tr-test             # any sandbox config you created
```

You almost never need `cargo clean` — incremental builds handle dependency
changes fine. Run it if a strange compile error refuses to go away.
