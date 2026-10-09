# Launch video

`brag.mp4` is a 23-second launch video for TypeRush (1920×1080, 30fps, with sound). It was made with [`/brag`](https://github.com/latent-spaces/brag), using its `/brag-slim` workflow. The other files here:

- `brag.gif`: a silent, looping 960px copy that plays inline in the main README.
- `brag.jpg`: the poster frame, which is also baked in as frame 0 of the mp4.
- `share-copy.txt`: a caption you can post as-is.
- `brag-plan.md`: the storyboard.

## The footage is the real app

Every terminal frame is the real `typerush` binary, built from this repo. It ran in a pseudo-terminal, and a headless emulator ([pyte](https://github.com/selectel/pyte)) recorded it cell by cell. A script plays the typist: it reads the words off the screen and types them at about 100 WPM, with one corrected typo. All the WPM, accuracy, results and stats figures are what the app itself printed.

Five shorter practice runs were recorded first, so the trend chart shows a real climb instead of a single bar. ANSI colours are drawn with the Catppuccin Mocha palette, the same theme as `assets/vhs_auto_made_gif/demo.tape`. The `monokai`, `dracula` and `light` scenes use those themes' own hex colours.

## Rebuild it

`source/` holds the pipeline:

| File | What it does |
|---|---|
| `termrec.py` | Runs `target/release/typerush` in a PTY and records every frame to JSON. Scenes: `menu`, `typing`, `partial`, `stats`, `probe`. |
| `prep.py` | Bundles the recordings into `data.js` and extracts keystroke times into `keys.json`. |
| `brag.html` | Draws each video frame as a pure function of time, rendering the terminal recordings on a canvas. |
| `capture.js` | Captures the frames with Playwright/Chromium and pipes them to FFmpeg. |
| `audio.py` | Synthesizes the music and effects with numpy. Key clicks follow `keys.json`. |

```bash
cargo build --release
cd docs/brag/source
pip install pyte numpy             # plus Playwright (npm i -g playwright), FFmpeg and the Inter font

export REC_HOME=/tmp/typerush-brag  # a throwaway $HOME, so your real stats stay untouched
# five practice runs, each faster, so the trend chart has a history
for d in 0.16,0.22 0.14,0.19 0.12,0.17 0.11,0.15 0.10,0.14; do
  REC_DELAY=$d REC_COLS=90 REC_ROWS=14 python3 termrec.py /dev/null typing -- --words 10
done
REC_DELAY=0.085,0.125 REC_COLS=90 REC_ROWS=14 python3 termrec.py typing.json typing -- --words 10
REC_COLS=90 REC_ROWS=24 python3 termrec.py menu.json menu --
REC_COLS=90 REC_ROWS=24 python3 termrec.py stats.json stats --
for th in dark monokai dracula light; do
  REC_DELAY=0.085,0.125 REC_COLS=90 REC_ROWS=14 python3 termrec.py theme-$th.json partial -- --words 25 --theme $th
done

python3 prep.py                    # -> data.js, keys.json
node capture.js stills 9.0 21.6    # spot-check -> stills/
node capture.js video              # -> video-noaudio.mp4
python3 audio.py                   # -> music.wav
cp stills/t-21.60.png poster.png
ffmpeg -y -i video-noaudio.mp4 -loop 1 -i poster.png -i music.wav \
  -filter_complex "[0:v][1:v]overlay=0:0:enable='eq(n,0)':shortest=1,format=yuv420p[v];[2:a]loudnorm=I=-16:TP=-1.5:LRA=7[a]" \
  -map "[v]" -map "[a]" -c:v libx264 -preset slow -crf 20 -c:a aac -b:a 192k -movflags +faststart -t 23 ../brag.mp4
ffmpeg -y -i poster.png -q:v 2 ../brag.jpg

# README GIF: render with the background held still (?freeze), so it stays around 6 MB
QUERY="?freeze" OUT=video-frozen.mp4 node capture.js video
ffmpeg -y -i video-frozen.mp4 -vf "fps=15,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" -loop 0 ../brag.gif
```

Because the words are random, every take differs a little, and so do the WPM figures in it.
