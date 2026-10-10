---
name: typerush-dev
description: How to work on the TypeRush repo — branch/commit/PR flow per version, quality gates, end-to-end testing of the TUI, doc rules, and the learnings log to read before and update after every fix or feature. Use for any code, docs or release change in this repository.
---

# Working on TypeRush

TypeRush is a terminal typing trainer in Rust (ratatui + crossterm). Speed
is the point of the app: every keystroke redraws one frame, so the render
path stays cheap and never touches the disk.

## Before you start

1. Read `docs/LEARNINGS.md` — past bugs, their causes and the tests that
   guard them. Most new bugs are a sibling of an old one.
2. Skim `docs/ARCHITECTURE.md` for the module you are changing, and
   `docs/ROADMAP.md` for what the current version plans.

## Flow for a version

- One branch per version from `main` (`feature/vX.Y.Z`), one pull request.
- Plan first: an ordered list of steps in `docs/ROADMAP.md` under the
  version's heading. Anything that belongs to a later version goes to that
  version's section, not into this branch.
- One commit (or a few) per step, Conventional Commits: `feat(scope):`,
  `fix(scope):`, `refactor:`, `chore(deps):`, `docs(scope):`, `test:`.
  The body says what was wrong and why the change fixes it.
- Bug fixes start with a failing test that reproduces the bug.

## Quality gates (same as CI)

```bash
cargo fmt --all -- --check
cargo check --all-targets --locked
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
cargo +1.88.0 check --locked      # MSRV
cargo audit                       # RustSec advisories
```

## End-to-end check

Unit tests don't catch TUI regressions. Drive the release binary in tmux
with a throwaway `HOME` — never the real `~/.typerush` — following
"End-to-end testing in tmux" in `docs/DEVELOPMENT.md` (keyboard and mouse).

## Rules that are easy to miss

- User text (file names, custom files, `stats.json`, error messages) is
  drawn through `text::printable`; typed text drops `text::is_untypeable`.
- Every input gets a bound: file size, CLI counts, lists that grow.
- New theme colors must pass `rgb_themes_meet_contrast_minimum` (4.5:1).
- Every mouse action has a keyboard equivalent; clicks fire on release.
- Docs: diagrams in `.md` files are mermaid; README.md has no mermaid
  (crates.io renders it). Never name other products or companies in the
  repo — describe features in our own words.

## When you finish a fix or feature

- Add an entry to `docs/LEARNINGS.md` (bug: symptom → cause → fix → guard;
  feature: what and how) under the current version.
- Update `CHANGELOG.md` `[Unreleased]`, `docs/USAGE.md` for anything a user
  sees, `docs/ARCHITECTURE.md` for structure, the ROADMAP step's status.
- Update the PR description to match what the branch now contains.
