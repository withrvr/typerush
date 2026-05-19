# Release Guide

Step-by-step process for cutting a new TypeRush release and publishing it to
[crates.io](https://crates.io/crates/typerush). Run through every section in
order — the order matters (see the note at the bottom on tag-before-publish).

Examples below use `v0.2.0`. Substitute the version you are actually shipping.

---

## 1. Pre-flight: working tree sanity

Before doing anything else, confirm:

- `git status` is clean.
- Local `main` is in sync with `origin/main` (`git status -sb` shows no
  ahead/behind).
- `Cargo.toml`'s `version = "..."` matches the version you intend to tag.
- No tag for that version exists yet: `git tag -l`.
- `CHANGELOG.md` has a finalized entry for the version (date filled in, not
  `Unreleased`).

If any of these is wrong, fix it and commit before continuing.

---

## 2. Local verification

These must all pass on the version-bump commit:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
```

Then smoke-test the actual release binary — type checking and unit tests do not
catch TUI regressions:

```bash
./target/release/typerush
```

Exercise the things that changed in this release. For v0.2.0 that means at
minimum:

- Run a short typing test end-to-end.
- Switch themes (the v0.2 background paint work).
- Trigger `Ctrl+Backspace` mid-word (the v0.2 input fix).
- Run with `--help` and with a non-default config to exercise config loading.

---

## 3. Package-shape check (the important one before publish)

`cargo publish --dry-run` alone is not enough — it builds, but it does not show
you the file list. Always run `cargo package --list` first so you can *see*
what will be uploaded.

```bash
cargo package --list      # lists every file that will end up in the .crate
cargo package             # builds the .crate into target/package/
cargo publish --dry-run   # builds from that tarball, no upload
```

Eyeball the file list:

- ✅ Present: `README.md`, `LICENSE`, `CHANGELOG.md`, `Cargo.toml`, `src/**`,
  `config.example.toml`.
- ❌ Absent: `assets/`, `.github/`, `*.tape`, `*.mp4`, `*.gif`, `docs/` (if
  excluded), local dev junk.
- Crate size should be in the hundreds-of-KB range, not multiple MB. A bloated
  tarball usually means the `exclude` list in `Cargo.toml` is missing a path.

If the file list is wrong, update `exclude` in `Cargo.toml`, commit, and
re-run.

---

## 4. Docs sanity

`docs.rs` runs `cargo doc` after publish. A failure there is annoying and
public, so catch it locally first:

```bash
cargo doc --no-deps
```

---

## 5. Tag and create the GitHub release

Only after sections 2–4 are all green.

```bash
git tag -a v0.2.0 -m "v0.2.0"
git push origin v0.2.0

gh release create v0.2.0 \
  --title "v0.2.0" \
  --notes-from-tag
# or: --notes-file with the relevant slice of CHANGELOG.md
```

---

## 6. Publish to crates.io

```bash
cargo login      # only if not already logged in on this machine
cargo publish    # no --dry-run this time
```

---

## 7. Post-publish verification

- `cargo search typerush` — confirm the new version is listed.
- Visit <https://crates.io/crates/typerush> — version, README, and metadata
  render correctly.
- Visit <https://docs.rs/typerush> — the docs build can take a few minutes
  after publish; if it fails, the build log is linked from that page.
- Optional: `cargo install typerush --version 0.2.0` in a scratch directory to
  prove a clean install works end-to-end.

---

## Why tag before publish

Tag first (§5), publish second (§6). The reason:

- A crates.io version is **immutable** once uploaded. You cannot replace,
  yank-and-reupload, or amend it.
- A git tag is cheap to move or delete before it's been pulled by others.

So if anything goes wrong, you want the failure to happen on the side that is
recoverable. If `cargo publish` fails after the tag is pushed, you fix the code,
bump to the next patch version, and re-tag. If you publish first and then
realize the tag points at the wrong commit, you are stuck with a published
crate that does not match any tag.

---

## Quick reference (happy-path commands)

```bash
# 1. sanity
git status && git tag -l

# 2. verify
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
./target/release/typerush

# 3. package shape
cargo package --list
cargo publish --dry-run

# 4. docs
cargo doc --no-deps

# 5. tag + release
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
gh release create vX.Y.Z --title "vX.Y.Z" --notes-from-tag

# 6. publish
cargo publish

# 7. verify
cargo search typerush
```
