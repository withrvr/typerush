"""Bundle the terminal recordings into data.js for brag.html, and keystroke times into keys.json.

Consecutive identical frames are dropped (the renderer shows the last frame at or before t).
Keystroke times come from the typing take: every change of the typing-box row
(the first row after the box's top border), ignoring the timer row.
"""
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
NAMES = ["menu", "typing", "stats", "theme-dark", "theme-monokai", "theme-dracula", "theme-light"]

out = {}
for name in NAMES:
    d = json.load(open(os.path.join(HERE, name + ".json")))
    frames, prev = [], None
    for f in d["frames"]:
        key = json.dumps(f["rows"])
        if key != prev:
            frames.append({"t": f["t"], "rows": f["rows"]})
        prev = key
    out[name] = {"cols": d["cols"], "rows": d["rows"], "frames": frames}

# keystrokes: changes to the box's first content row in the typing take
typing = json.load(open(os.path.join(HERE, "typing.json")))
keys, prev = [], None
for f in typing["frames"]:
    lines = ["".join(r[0] for r in row) for row in f["rows"]]
    tops = [i for i, l in enumerate(lines) if "┌ typerush" in l]
    if not tops:
        continue  # results screen
    row = json.dumps(f["rows"][tops[0] + 1])
    if prev is not None and row != prev:
        keys.append(f["t"])
    prev = row

with open(os.path.join(HERE, "data.js"), "w") as fh:
    fh.write("window.REC = " + json.dumps(out, separators=(",", ":")) + ";\n")
with open(os.path.join(HERE, "keys.json"), "w") as fh:
    json.dump(keys, fh)
print({k: len(v["frames"]) for k, v in out.items()}, "keys:", len(keys), keys[:3], keys[-3:])
