# TypeRush

**Type faster. Right in your terminal.**

[![Crates.io](https://img.shields.io/crates/v/typerush)](https://crates.io/crates/typerush)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Build](https://github.com/withrvr/typerush/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/withrvr/typerush/actions/workflows/ci.yml)

<a href="docs/brag/brag.mp4"><img src="docs/brag/brag.gif" alt="TypeRush in action: a fast typing run with live speed, its results chart, the new practice modes, the stats history, and the color themes" width="100%"></a>

**[▶ Watch with sound (23 seconds)](docs/brag/brag.mp4)**

TypeRush is a typing trainer that runs in your terminal, the text window
where you type commands. Pick a test, type, and see your speed and accuracy
change with every key. When you're done you get a chart of the whole run,
and over time you can watch yourself get faster.

It's one small, free program. No browser, no account, no internet, no ads.
Everything it saves stays on your computer.

---

## Why you'll like it

- **See your speed live.** Words per minute (WPM) and accuracy update as you
  type, and every finished run gets a second-by-second chart.
- **Practice the way you want.** Sprints of 15 to 120 seconds, a set number
  of words, famous quotes, real code in seven languages, or zen mode with no
  timer at all.
- **Train your weak spots.** TypeRush remembers the words you get wrong and
  builds a run out of them. It also shows which keys slow you down.
- **Race yourself.** A second cursor moves at your best speed, so you can see
  whether you're ahead or behind.
- **Watch your progress.** Personal bests, streaks, a speed trend, a level
  that grows as you practice, and a year-long activity calendar.
- **A new challenge every day.** The daily challenge gives everyone the same
  words for the day.
- **Make it yours.** Four color themes built in, or make your own with a tiny
  file. Change settings from inside the app, no text editor needed.
- **Works everywhere.** Windows, macOS and Linux, with keyboard or mouse.

### New in 0.7

- **Drill your misses:** a run made of the words you've gotten wrong lately.
- **Race your best:** the pace cursor shows your best run (or any speed you
  pick) while you type.
- **Type blind:** no colors and no numbers until the end, so you stop
  watching the screen.
- **Stop on error:** a mistake has to be fixed before you can go on.
- **Repeat a run:** press `F6` after a run to type the same words again.
- **Your own themes:** drop a color file in `~/.typerush/themes/`.

Everything that changed is in the [changelog](CHANGELOG.md).

---

## Get TypeRush

### Option 1: download it (no setup)

1. Go to the [latest release](https://github.com/withrvr/typerush/releases/latest).
2. Download the file for your computer:

   | Your computer | File to download |
   | --- | --- |
   | Windows | `typerush-…-x86_64-pc-windows-msvc.zip` |
   | Mac with an Apple chip (M1 or newer) | `typerush-…-aarch64-apple-darwin.tar.gz` |
   | Mac with an Intel chip | `typerush-…-x86_64-apple-darwin.tar.gz` |
   | Linux | `typerush-…-x86_64-unknown-linux-gnu.tar.gz` |

3. Unzip it (double-click the file).
4. Start it:
   - **Windows:** double-click `typerush.exe`. If Windows warns you about an
     unknown app, click **More info**, then **Run anyway**.
   - **Mac:** double-click `typerush`. If macOS says it can't check the app,
     open **System Settings → Privacy & Security** and click **Open Anyway**.
     It asks only once, because the app isn't from the App Store.
   - **Linux:** open a terminal in that folder and run `./typerush`.

### Option 2: install with Rust

If you have [Rust](https://rustup.rs) installed, one command builds and
installs it:

```bash
cargo install typerush
```

Then type `typerush` in any terminal to start.

---

## Start typing

Run `typerush` to open the menu. Pick a test with the arrow keys (or the
mouse) and press `Enter`.

| Key | What it does |
| --- | --- |
| `Enter` | start a test |
| `Tab` or `F5` | restart while typing |
| `Esc` | finish the test, or go back to the menu |
| `F6` | type the same words again (after a run) |
| `?` or `F1` | show every key |
| `Ctrl+C` | quit |

You can also start a test straight from the terminal:

```bash
typerush --time 30       # type for 30 seconds
typerush --words 50      # type 50 words
typerush --daily         # today's daily challenge
typerush --missed        # practice the words you got wrong
```

---

## Learn more

- 📖 [User guide](docs/USAGE.md): every mode, key, option and setting
- 📝 [Changelog](CHANGELOG.md): what's new in each version
- 🐞 [Report a bug or ask for a feature](https://github.com/withrvr/typerush/issues/new/choose)
- 🤝 [Help build TypeRush](CONTRIBUTING.md): setup, how the code is laid out, how to send a change

If you like TypeRush, a ⭐ on GitHub helps other people find it.

## License

[MIT](LICENSE). Free to use, change and share.
