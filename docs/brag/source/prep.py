"""Bundle the recordings into data.js (deduped screens) and keys.json (keystroke times)."""
import json

CLIPS = ["hero", "menu", "stats", "missed", "pace", "blind",
         "theme-monokai", "theme-dracula", "theme-light", "theme-ocean"]
clips, keys = {}, {}
for name in CLIPS:
    d = json.load(open(name + ".json"))
    screens, index, frames = [], {}, []
    last_cur, ks = None, []
    for f in d["frames"]:
        rows = [[[r[0], r[1], r[2], int(r[3]), int(r[4]), int(r[5])] for r in row] for row in f["rows"]]
        k = json.dumps(rows, separators=(",", ":"))
        if k not in index:
            index[k] = len(screens)
            screens.append(rows)
        if not frames or frames[-1][1] != index[k]:
            frames.append([f["t"], index[k]])
        # the typing caret: the cyan underlined cell; it moves once per key
        cur = None
        for y, row in enumerate(rows[:12]):
            x = 0
            for r in row:
                if r[1] == "00cdcd" and r[4] and len(r[0]) == 1:
                    cur = (x, y)
                x += len(r[0])
        if cur and last_cur and cur != last_cur:
            ks.append(f["t"])
        last_cur = cur or last_cur
    clips[name] = {"cols": d["cols"], "rows": d["rows"], "marks": d["marks"],
                   "frames": frames, "screens": screens}
    keys[name] = ks
    print(name, len(d["frames"]), "frames ->", len(screens), "screens,", len(ks), "keys")
open("data.js", "w").write("const DATA = " + json.dumps(clips, separators=(",", ":")) + ";\n")
json.dump(keys, open("keys.json", "w"))
