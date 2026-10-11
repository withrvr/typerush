"""Music + effects for the brag video, synthesized with numpy -> music.wav (44.1 kHz stereo)."""
import json, wave
import numpy as np

SR, DUR, BPM = 44100, 23.0, 120
BEAT = 60 / BPM
N = int(SR * DUR)
L = np.zeros(N); R = np.zeros(N)
rng = np.random.default_rng(3)
T = dict(hook=0, reveal=3.0, results=6.1, v07=9.5, stats=14.8, themes=17.6, outro=19.7)
SUB, TH = [2.0, 1.8, 1.5], 0.42


def note(m): return 440 * 2 ** ((m - 69) / 12)


def add(sig, t, gain=1.0, pan=0.0):
    i = int(t * SR)
    if i >= N: return
    s = sig[: N - i] * gain
    L[i:i + len(s)] += s * (1 - max(0, pan))
    R[i:i + len(s)] += s * (1 + min(0, pan))


def lp(x, a):  # one-pole low-pass
    y = np.empty_like(x); acc = 0.0
    for i, v in enumerate(x):
        acc += a * (v - acc); y[i] = acc
    return y


def env(n, a, d):
    t = np.arange(n) / SR
    return np.minimum(1, t / max(a, 1e-4)) * np.exp(-t / d)


def tone(f, dur, harm=(1,), a=0.005, d=0.3, detune=0.0):
    n = int(dur * SR); t = np.arange(n) / SR
    s = sum(np.sin(2 * np.pi * f * k * t * (1 + detune)) / k for k in harm)
    return s * env(n, a, d)


# chords: Am F C G, one bar (2 s) each, MIDI roots
CH = [(57, 60, 64), (53, 57, 60), (48, 55, 64), (55, 59, 62)]
bars = int(DUR / (4 * BEAT)) + 1

# pad: detuned saws, low-passed, under everything
for b in range(bars):
    t0 = b * 4 * BEAT
    if t0 >= T['outro'] + 1.3: break
    for m in CH[b % 4]:
        for dt in (-0.004, 0.004):
            n = int(4 * BEAT * SR); tt = np.arange(n) / SR
            saw = 2 * ((note(m) * (1 + dt) * tt) % 1) - 1
            e = np.minimum(1, tt / 0.4) * np.minimum(1, (4 * BEAT - tt) / 0.3)
            add(lp(saw, 0.035) * e, t0, 0.035, pan=dt * 60)

# final chord at the wordmark, ringing out
for m in (57, 64, 69, 72, 76):
    add(tone(note(m), 3.0, harm=(1, 2, 3), a=0.01, d=1.1), T['outro'] + 1.3, 0.06, pan=(m - 66) / 30)

# kick + bass from the reveal to the outro; hats from the start
t = 0.0
while t < T['outro'] - 0.01:
    beat = round(t / BEAT)
    if t >= T['reveal'] - 0.01:
        n = int(0.35 * SR); tt = np.arange(n) / SR
        f = 48 + 90 * np.exp(-tt * 32)
        kick = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-tt * 9)
        add(kick, t, 0.3)
    m = CH[int(t / (4 * BEAT)) % 4][0] - 12
    for off in (0, BEAT / 2):   # bass from the first frame, fuller once the kick comes in
        add(lp(tone(note(m), 0.24, harm=(1, 2, 3, 4), a=0.004, d=0.12), 0.25), t + off,
            0.16 if t >= T['reveal'] - 0.01 else 0.11)
    # off-beat hat
    n = int(0.05 * SR)
    hat = np.diff(rng.standard_normal(n + 1)) * env(n, 0.001, 0.012)
    add(hat, t + BEAT / 2, 0.035 if t < T['reveal'] else 0.05, pan=0.3)
    t += BEAT

# pluck on each scene change: chord tones, bright and short
cuts = [T['reveal'], T['results'], T['v07'], T['v07'] + SUB[0], T['v07'] + SUB[0] + SUB[1], T['stats'], T['themes'], T['outro']]
for i, c in enumerate(cuts):
    m = CH[int(c / (4 * BEAT)) % 4]
    for j, mm in enumerate(m):
        add(tone(note(mm + 12), 0.6, harm=(1, 2, 3), a=0.002, d=0.18), c + j * 0.03, 0.05, pan=(j - 1) * 0.3)
    # soft air swell into the cut
    n = int(0.35 * SR); nz = lp(rng.standard_normal(n), 0.08)
    sw = nz * np.linspace(0, 1, n) ** 2
    add(sw, c - 0.33, 0.05)
# theme flips: tiny ticks
for k in range(1, 5):
    add(tone(note(88), 0.08, a=0.001, d=0.02), T['themes'] + k * TH, 0.05)
# "new best" chime as the results land
for j, m in enumerate((81, 88, 93)):
    add(tone(note(m), 1.4, harm=(1, 2.01), a=0.003, d=0.45), T['results'] + 0.35 + j * 0.07, 0.05, pan=0.2 * (j - 1))

# key clicks at the real keystroke times, mapped from clip time to video time
keys = json.load(open('keys.json'))
data = json.load(open('hero.json')); h0 = data['marks']['start']
marks = {k: json.load(open(k + '.json'))['marks'].get('in') for k in ('missed', 'pace', 'blind')}
spans = [('hero', h0 - 0.3, 1.7, T['hook'], T['reveal'])]
s0 = T['v07']
for (name, off), d in zip([('missed', 1.2), ('pace', 1.6), ('blind', 1.2)], SUB):
    spans.append((name, marks[name] + off, 1.2, s0, s0 + d)); s0 += d


def click():
    n = int(0.03 * SR)
    nz = np.diff(lp(rng.standard_normal(n + 1), 0.5)) * env(n, 0.0005, 0.006)
    return nz + tone(2400 + rng.uniform(-200, 200), 0.03, a=0.0005, d=0.004) * 0.3


for name, base, rate, a, b in spans:
    for kt in keys[name]:
        vt = a + (kt - base) / rate
        if a + 0.05 <= vt < b - 0.05:
            add(click(), vt, 0.07 * rng.uniform(0.75, 1.0), pan=rng.uniform(-0.15, 0.15))
# outro: the install command typed at 45 ms a key
for i in range(len('cargo install typerush')):
    add(click(), T['outro'] + 0.3 + i * 0.045, 0.06)

mix = np.stack([L, R], 1)
mix /= np.max(np.abs(mix)) + 1e-9
mix = np.tanh(mix * 1.4) / np.tanh(1.4)
fade = np.ones(N); fn = int(0.6 * SR); fade[-fn:] = np.linspace(1, 0, fn)
mix *= fade[:, None] * 0.9
with wave.open('music.wav', 'wb') as w:
    w.setnchannels(2); w.setsampwidth(2); w.setframerate(SR)
    w.writeframes((mix * 32767).astype('<i2').tobytes())
print('ok', mix.shape)
