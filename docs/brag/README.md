# Launch video

`brag.mp4` is the 23-second launch video for TypeRush 0.7.0 (1920×1080,
30 fps, with sound). It was made with
[`/brag`](https://github.com/latent-spaces/brag), using its `/brag-slim`
workflow. The other files here:

- `brag.gif`: a silent, looping 960px copy that plays in the main README.
- `brag.jpg`: the poster frame, which is also frame 0 of the mp4.
- `share-copy.txt`: a caption you can post as-is.
- `brag-plan.md`: the storyboard.

## The footage is the real app

Every terminal frame is the real `typerush` 0.7.0 binary built from this
repo. It ran in a pseudo-terminal, and a headless terminal emulator
([pyte](https://github.com/selectel/pyte)) recorded it cell by cell. A script
plays the typist: it reads the words off the screen and types them. All the
WPM, accuracy, chart and stats figures are what the app printed.

Eight practice runs were typed first, each a little faster, so the Stats
screen shows a real climb (50 → 93 wpm) before the 116 wpm run in the video.
All of them ran in a throwaway `$HOME`, never in a real `~/.typerush`.

## Rebuild it

`source/` holds the pipeline:

| File | What it does |
|---|---|
| `rec.py` | Runs `target/release/typerush` in a PTY and records every frame to JSON. A small step list types words, presses keys and marks times. |
| `prep.py` | Bundles the recordings into `data.js` and finds the keystroke times (`keys.json`). |
| `brag.html` | Draws each video frame as a pure function of time on a canvas. |
| `capture.mjs` | Captures the frames in headless Chromium and pipes them to FFmpeg. |
| `audio.py` | Synthesizes the music and effects with numpy; key clicks follow `keys.json`. |

```bash
cargo build --release
cd docs/brag/source
python3 -m venv .venv && .venv/bin/pip install pyte numpy
npm i playwright@1.56.0 && npx playwright install --only-shell chromium
export REC_HOME=$PWD/home; mkdir -p $REC_HOME    # throwaway $HOME

# eight practice runs, each faster, so Stats has a history
i=0; for d in 0.19,0.26 0.17,0.23 0.15,0.21 0.14,0.19 0.13,0.17 0.12,0.16 0.11,0.15 0.10,0.14; do
  i=$((i+1)); lo=${d%,*}; hi=${d#*,}
  REC_SEED=$i .venv/bin/python rec.py - 90 22 "[[\"wait\",0.8],[\"type\",25,$lo,$hi,[[3,1,false],[12,2,true],[20,0,false]]],[\"wait\",1.0]]" -- --words 25
done
# the run in the video, then the clips
REC_SEED=42 .venv/bin/python rec.py hero.json 90 26 '[["wait",0.8],["mark","start"],["type",25,0.078,0.122,[[6,2,true]]],["mark","done"],["wait",4.0]]' -- --words 25
D='\u001b[B'; .venv/bin/python rec.py menu.json 90 26 "[[\"wait\",1.0],[\"mark\",\"in\"]$(for _ in 1 2 3 4 5 6; do printf ',["key","%s"],["wait",0.35]' "$D"; done),[\"wait\",0.9]]" --
.venv/bin/python rec.py stats.json 90 26 '[["wait",1.0],["key","s"],["wait",0.2],["mark","in"],["wait",2.5]]' --
.venv/bin/python rec.py missed.json 90 26 '[["wait",1.0],["mark","in"],["type",6,0.085,0.13],["wait",0.6]]' -- --missed
REC_SEED=5 .venv/bin/python rec.py pace.json 90 26 '[["wait",1.0],["mark","in"],["type",9,0.098,0.15],["wait",0.3]]' -- --pace best --words 25
.venv/bin/python rec.py blind.json 90 26 '[["wait",1.0],["mark","in"],["type",8,0.085,0.13,[[2,1,false]]],["wait",0.3]]' -- --blind --words 25
mkdir -p $REC_HOME/.typerush/themes
printf 'base = "dark"\nbackground = "#0B1D2A"\nneutral    = "#D8E6F0"\naccent     = "#4FC3F7"\npending    = "#7A8C99"\n' > $REC_HOME/.typerush/themes/ocean.toml
for th in monokai dracula light ocean; do
  REC_SEED=3 .venv/bin/python rec.py theme-$th.json 90 26 '[["wait",0.9],["mark","in"],["type",5,0.085,0.13],["wait",0.3]]' -- --words 25 --theme $th
done

.venv/bin/python prep.py               # -> data.js, keys.json
node capture.mjs stills 1.5 5.5 8.9    # spot-check -> stills/
node capture.mjs video                 # -> video-noaudio.mp4
.venv/bin/python audio.py              # -> music.wav
cp stills/t-5.50.png poster.png
ffmpeg -y -i video-noaudio.mp4 -loop 1 -i poster.png -i music.wav \
  -filter_complex "[1:v]format=yuv420p[p];[0:v][p]overlay=0:0:enable='eq(n,0)':shortest=1,format=yuv420p[v];[2:a]loudnorm=I=-16:TP=-1.5:LRA=7[a]" \
  -map "[v]" -map "[a]" -c:v libx264 -preset slow -crf 20 -c:a aac -b:a 192k -movflags +faststart -t 23 ../brag.mp4
ffmpeg -y -i poster.png -q:v 2 ../brag.jpg

# README GIF: the background held still (?freeze) keeps it under 7 MB
QUERY="?freeze" OUT=video-frozen.mp4 node capture.mjs video
ffmpeg -y -i video-frozen.mp4 -vf "fps=12,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" -loop 0 ../brag.gif
```

The words are random, so every take differs a little, and so do the figures
in it. The scene times in `brag.html` and `audio.py` (`T`, `SUB`, `TH`) must
match each other.
