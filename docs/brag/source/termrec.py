"""Record the real typerush TUI in a pseudo-terminal, cell by cell.

Usage: termrec.py <out.json> <scene> -- <typerush args...>

Scenes:
  probe   start, wait 1.5s, print the screen as text and exit
  menu    show the menu, move down/up through the rows (j/k)
  typing  read the words off the screen and type them (per-char delay from
          REC_DELAY="lo,hi" seconds) with one corrected typo, then stay on the
          results screen
  partial same, but stop after 5 words (for theme stills)
  stats   open the menu and press Tab for the stats screen
  still   start and hold (for theme stills)

Writes {"cols","rows","fps","frames":[{"t","cur":[x,y],"rows":[[[text,fg,bg,bold,ul,rev],...],...]}]}.
"""
import fcntl
import json
import os
import pty
import random
import select
import struct
import sys
import termios
import time

import pyte

COLS = int(os.environ.get("REC_COLS", 100))
ROWS = int(os.environ.get("REC_ROWS", 28))
FPS = 30
BIN = os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../../target/release/typerush")


def spawn(args, home):
    pid, fd = pty.fork()
    if pid == 0:
        env = {"HOME": home, "TERM": "xterm-256color", "COLORTERM": "truecolor",
               "PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"}
        os.execve(BIN, [BIN] + args, env)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
    return pid, fd


class Rec:
    def __init__(self, fd):
        self.fd = fd
        self.screen = pyte.Screen(COLS, ROWS)
        self.stream = pyte.ByteStream(self.screen)
        self.frames = []
        self.t0 = time.monotonic()
        self.next_frame = 0.0

    def pump(self, until):
        """Feed pty output into the emulator until `until` (seconds since start), snapshotting at FPS."""
        while True:
            now = time.monotonic() - self.t0
            if now >= until:
                break
            r, _, _ = select.select([self.fd], [], [], min(0.005, until - now))
            if r:
                try:
                    data = os.read(self.fd, 65536)
                except OSError:
                    data = b""
                if data:
                    self.stream.feed(data)
            now = time.monotonic() - self.t0
            while now >= self.next_frame:
                self.frames.append(self.snapshot(self.next_frame))
                self.next_frame += 1 / FPS

    def send(self, b):
        os.write(self.fd, b)

    def now(self):
        return time.monotonic() - self.t0

    def text(self):
        return "\n".join(self.screen.display)

    def snapshot(self, t):
        rows = []
        for y in range(ROWS):
            line = self.screen.buffer[y]
            runs, cur = [], None
            for x in range(COLS):
                c = line[x]
                key = (c.fg, c.bg, c.bold, c.underscore, c.reverse)
                if cur and cur[1:] == list(key):
                    cur[0] += c.data
                else:
                    cur = [c.data, *key]
                    runs.append(cur)
            rows.append(runs)
        return {"t": round(t, 4), "cur": [self.screen.cursor.x, self.screen.cursor.y],
                "rows": rows}


def box_words(rec):
    """The words inside the typing box: rows between the box's top and bottom border."""
    lines = rec.screen.display
    top = next(i for i, l in enumerate(lines) if "┌" in l or "╭" in l)
    bot = next(i for i in range(top + 1, ROWS) if "└" in lines[i] or "╰" in lines[i])
    words = []
    for l in lines[top + 1:bot]:
        inner = l.strip().strip("│").strip()
        words += inner.split()
    return words


def main():
    out, scene = sys.argv[1], sys.argv[2]
    args = sys.argv[sys.argv.index("--") + 1:]
    home = os.environ.get("REC_HOME", "/tmp/typerush-home")
    os.makedirs(home, exist_ok=True)
    pid, fd = spawn(args, home)
    rec = Rec(fd)
    rng = random.Random(11)

    if scene == "probe":
        rec.pump(1.5)
        print(rec.text())
    elif scene == "still":
        rec.pump(1.5)
    elif scene == "menu":
        rec.pump(1.2)
        for k in [b"j", b"j", b"j", b"k", b"k", b"k"]:
            rec.send(k)
            rec.pump(rec.now() + 0.28)
        rec.pump(rec.now() + 0.6)
    elif scene == "stats":
        rec.pump(1.0)
        rec.send(b"\t")
        rec.pump(rec.now() + 2.0)
    elif scene in ("typing", "partial"):
        lo, hi = map(float, os.environ.get("REC_DELAY", "0.08,0.13").split(","))
        rec.pump(1.0)
        words = box_words(rec)
        if scene == "partial":
            words = words[:5]
        print("words:", words, file=sys.stderr)
        typo_at = (2, 1)  # word index, char index: type a wrong char, then backspace
        for wi, w in enumerate(words):
            for ci, ch in enumerate(w):
                if (wi, ci) == typo_at:
                    rec.send(b"x" if ch != "x" else b"z")
                    rec.pump(rec.now() + 0.16)
                    rec.send(b"\x7f")
                    rec.pump(rec.now() + 0.12)
                rec.send(ch.encode())
                rec.pump(rec.now() + rng.uniform(lo, hi))
            if scene == "partial" and wi == len(words) - 1:
                break  # stop mid-session, leave the last word unsubmitted
            rec.send(b" ")  # space submits the word, including the last one
            rec.pump(rec.now() + rng.uniform(lo, hi) * 1.3)
        rec.pump(rec.now() + 2.5)
    print(rec.text(), file=sys.stderr)
    try:
        os.kill(pid, 9)
    except OSError:
        pass
    with open(out, "w") as f:
        json.dump({"cols": COLS, "rows": ROWS, "fps": FPS, "frames": rec.frames}, f)


if __name__ == "__main__":
    main()
