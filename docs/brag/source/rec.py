"""Record the real typerush binary in a PTY through pyte, cell by cell.

usage: rec.py <out.json|-> <cols> <rows> <script.json> -- <typerush args>
script: list of steps
  ["wait", s]                 pump for s seconds
  ["key", "text"]             send raw text (escape sequences allowed)
  ["type", n, lo, hi, typos]  type the first n words read off the screen
                              typos: list of [word, char, fix] (fix=false leaves it wrong)
  ["mark", name]              store the current time under name
  ["show"]                    print the screen to stderr
Env: REC_HOME (throwaway HOME), REC_SEED.
"""
import fcntl, json, os, pty, random, select, struct, sys, termios, time
import pyte

BIN = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../../target/release/typerush"))
FPS = 30


def main():
    out, cols, rows, script = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
    args = sys.argv[sys.argv.index("--") + 1:]
    steps = json.load(open(script)) if script.endswith(".json") else json.loads(script)
    home = os.environ["REC_HOME"]
    rng = random.Random(int(os.environ.get("REC_SEED", "7")))
    pid, fd = pty.fork()
    if pid == 0:
        env = {"HOME": home, "TERM": "xterm-256color", "COLORTERM": "truecolor",
               "PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"}
        os.execve(BIN, [BIN] + args, env)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    screen = pyte.Screen(cols, rows)
    stream = pyte.ByteStream(screen)
    frames, marks = [], {}
    t0 = time.monotonic()
    nxt = [0.0]

    def now():
        return time.monotonic() - t0

    def snap(t):
        out_rows = []
        for y in range(rows):
            line, runs, cur = screen.buffer[y], [], None
            for x in range(cols):
                c = line[x]
                key = [c.fg, c.bg, c.bold, c.underscore, c.reverse]
                if cur and cur[1:] == key:
                    cur[0] += c.data
                else:
                    cur = [c.data, *key]
                    runs.append(cur)
            out_rows.append(runs)
        return {"t": round(t, 4), "cur": [screen.cursor.x, screen.cursor.y],
                "hid": screen.cursor.hidden, "rows": out_rows}

    def pump(dt):
        until = now() + dt
        while True:
            n = now()
            if n >= until:
                break
            r, _, _ = select.select([fd], [], [], min(0.004, until - n))
            if r:
                try:
                    data = os.read(fd, 65536)
                except OSError:
                    data = b""
                if data:
                    stream.feed(data)
            n = now()
            while out != "-" and n >= nxt[0]:
                frames.append(snap(nxt[0]))
                nxt[0] += 1 / FPS

    def send(s):
        os.write(fd, s.encode())

    def text():
        return "\n".join(screen.display)

    def words_on_screen():
        # typing text = the lines between the first box's borders that hold words
        lines = screen.display
        tops = [i for i, l in enumerate(lines) if "╭" in l or "┌" in l]
        for top in tops:
            bot = next((i for i in range(top + 1, rows) if "╰" in lines[i] or "└" in lines[i]), None)
            if bot is None:
                continue
            ws = []
            for l in lines[top + 1:bot]:
                ws += l.strip().strip("│").strip().split()
            if len(ws) >= 3:
                return ws
        return []

    for st in steps:
        op = st[0]
        if op == "wait":
            pump(st[1])
        elif op == "key":
            send(st[1]); pump(0.02)
        elif op == "mark":
            marks[st[1]] = round(now(), 4)
        elif op == "show":
            print(text(), file=sys.stderr)
        elif op == "type":
            n, lo, hi = st[1], st[2], st[3]
            typos = {(a, b): f for a, b, f in (st[4] if len(st) > 4 else [])}
            words = words_on_screen()[:n]
            print("words:", words, file=sys.stderr)
            for wi, w in enumerate(words):
                for ci, ch in enumerate(w):
                    if (wi, ci) in typos:
                        wrong = "x" if ch != "x" else "z"
                        send(wrong); pump(rng.uniform(lo, hi) * 1.6)
                        if typos[(wi, ci)]:
                            send("\x7f"); pump(rng.uniform(lo, hi))
                            send(ch)
                        pump(rng.uniform(lo, hi))
                        continue
                    send(ch); pump(rng.uniform(lo, hi))
                if wi < len(words) - 1 or st[1] >= len(words_on_screen() or words):
                    send(" "); pump(rng.uniform(lo, hi) * 1.2)
    try:
        os.kill(pid, 9)
    except OSError:
        pass
    if out != "-":
        json.dump({"cols": cols, "rows": rows, "fps": FPS, "marks": marks, "frames": frames},
                  open(out, "w"))


if __name__ == "__main__":
    main()
