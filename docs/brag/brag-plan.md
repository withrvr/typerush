# /brag plan: TypeRush

**What it is:** a typing-speed trainer that runs entirely in your terminal. It's one Rust binary, installed with `cargo install typerush`.
**Who it's for:** developers who live in the command line and want to type faster without opening a browser.
**What sets it apart:** it needs no browser, no internet and no installer. You get live WPM and accuracy as you type, six modes (including real code snippets), four themes, and local stats history.
**Most impressive claim:** "No browser. No internet. No installer. Just one tiny binary."
**Visual hook:** the headline itself gets typed out, TypeRush-style. It starts as grey pending text, turns green as it's typed, and shows one red typo that gets corrected.
**Share caption:** "Type faster, right in your terminal."

## Angle
The real app, typed for real. Every terminal frame in the video is the actual `typerush` v0.2.0 binary. It ran in a pseudo-terminal and was recorded cell by cell. The WPM, accuracy, results and stats screens are what the app printed.

The typist is a script reading the words off the screen. Five practice runs at increasing speed came first, so the trend chart has an honest climb: 60.0 → 69.2 → 76.9 → 86.0 → 92.8 → 105.2 WPM.

## Tone
`default`: punchy, playful and clean. It uses the Catppuccin Mocha terminal palette (the same theme as the repo's own VHS demo), with Inter for captions and DejaVu Sans Mono for the terminal.

## Storyboard (22.5s, 1920×1080, 30fps)

| # | Time | Scene | On screen |
|---|---|---|---|
| 1 | 0.0–3.2 | **Hook** | "Type faster." is typed out letter by letter, with one red typo that's corrected. Then "Right in your terminal." |
| 2 | 3.2–6.0 | **Reveal** | A terminal window rises in, `$ typerush` is typed, and the real menu appears. The selection moves through the modes. Caption: "A typing trainer that lives in your terminal." |
| 3 | 6.0–13.0 | **Live typing** | The real `--words 10` session: words go green as they're typed, and WPM and accuracy update live, then the results screen shows 105.2 WPM (+12 vs last). Caption: "Live WPM and accuracy, keystroke by keystroke." |
| 4 | 13.0–16.0 | **Themes** | The same mid-session screen in `dark`, `monokai`, `dracula` and `light`, each with its `--theme` flag. Caption: "Four built-in themes. Or bring your own colors." |
| 5 | 16.0–18.8 | **Stats** | The real Stats screen: best WPM, average accuracy, the trend, and recent sessions. Caption: "Every session saved, right on your machine." |
| 6 | 18.8–22.5 | **Outro** | The real TYPERUSH banner, `cargo install typerush`, "One binary. No browser. No internet.", the supported platforms, and the repo URL. |

## Sound
Synthesized in D major at 124 BPM: a bright pluck arpeggio, a warm pad and a soft kick. The typing scene gets soft key clicks, timed to the recorded keystrokes and pitched to the key. There's a chime when results appear, a whoosh on each theme cut, and a bloom on the outro.
