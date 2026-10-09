"""Synthesizes the soundtrack for brag.mp4: music and effects written as one piece.

D major, 124 BPM, chords D - Bm - G - A (one bar each). Key clicks follow the real
keystrokes recorded from the typing take (keys.json), so the sound is in sync with
the screen. Effects share the pluck's reverb so they sit in one space.
Writes music.wav (stereo, 48 kHz).
"""
import json
import os
import wave

import numpy as np

SR = 48000
DUR = 23.0
N = int(SR * DUR)
t_all = np.arange(N) / SR
rng = np.random.default_rng(5)
HERE = os.path.dirname(os.path.abspath(__file__))

BEAT = 60 / 124
BAR = 4 * BEAT


def hz(midi):
    return 440.0 * 2 ** ((midi - 69) / 12)


# D(D F# A) Bm(B D F#) G(G B D) A(A C# E)
CHORDS = [[62, 66, 69], [59, 62, 66], [55, 59, 62], [57, 61, 64]]
ROOTS = [38, 35, 43, 45]


def chord_at(t):
    return int(t // BAR) % 4


def env_adsr(n, a, d, s, r, sustain_len):
    """Linear ADSR, lengths in seconds; returns array of length n."""
    e = np.zeros(n)
    A, D, S, R = int(a * SR), int(d * SR), int(sustain_len * SR), int(r * SR)
    i = 0
    seg = [
        np.linspace(0, 1, max(A, 1)),
        np.linspace(1, s, max(D, 1)),
        np.full(max(S, 1), s),
        np.linspace(s, 0, max(R, 1)),
    ]
    for part in seg:
        k = min(len(part), n - i)
        if k <= 0:
            break
        e[i : i + k] = part[:k]
        i += k
    return e


def lowpass_fft(x, cutoff, order=2):
    X = np.fft.rfft(x)
    f = np.fft.rfftfreq(len(x), 1 / SR)
    X *= 1 / np.sqrt(1 + (f / cutoff) ** (2 * order))
    return np.fft.irfft(X, len(x))


def highpass_fft(x, cutoff, order=2):
    X = np.fft.rfft(x)
    f = np.fft.rfftfreq(len(x), 1 / SR)
    X *= 1 - 1 / np.sqrt(1 + (f / cutoff) ** (2 * order))
    return np.fft.irfft(X, len(x))


def add(buf, start, sig, gain=1.0):
    i = int(start * SR)
    if i >= len(buf):
        return
    k = min(len(sig), len(buf) - i)
    buf[i : i + k] += sig[:k] * gain


def automation(points):
    """Piecewise-linear gain curve from [(time, gain), ...]."""
    ts, gs = zip(*points)
    return np.interp(t_all, ts, gs)


# ------------------------------------------------------------------ music
pad = np.zeros(N)
for b in range(int(DUR / BAR) + 1):
    notes = CHORDS[b % 4]
    ln = BAR + 0.6
    n = int(ln * SR)
    tt = np.arange(n) / SR
    e = env_adsr(n, 0.4, 0.4, 0.75, 0.6, ln - 1.4)
    sig = np.zeros(n)
    for m in notes + [notes[0] + 12]:
        for det in (-0.1, 0.0, 0.1):
            ph = 2 * np.pi * hz(m + det) * tt + rng.uniform(0, 6.28)
            sig += np.sin(ph) + 0.3 * np.sin(2 * ph) + 0.12 * np.sin(3 * ph)
    add(pad, b * BAR, sig * e / 16)
pad = lowpass_fft(pad, 1800)

bass = np.zeros(N)
for k in range(int(DUR / BEAT)):
    st = k * BEAT
    if st < 3.3 or st > 22.2:
        continue
    f = hz(ROOTS[chord_at(st)])
    n = int(0.42 * SR)
    tt = np.arange(n) / SR
    e = np.exp(-tt * 6) * np.minimum(1, tt / 0.008)
    add(bass, st, np.tanh(1.5 * np.sin(2 * np.pi * f * tt)) * e * 0.3)
bass = lowpass_fft(bass, 520)

pluck = np.zeros(N)
ARP = [0, 1, 2, 3, 1, 2, 3, 2]
for k in range(int(DUR / (BEAT / 2))):
    st = k * BEAT / 2
    if st < 3.3 or st > 22.0:
        continue
    ch = CHORDS[chord_at(st)] + [CHORDS[chord_at(st)][0] + 12]
    f = hz(ch[ARP[k % 8]] + 12)
    n = int(0.3 * SR)
    tt = np.arange(n) / SR
    e = np.exp(-tt * 15) * np.minimum(1, tt / 0.003)
    sig = (np.sin(2 * np.pi * f * tt) + 0.3 * np.sin(4 * np.pi * f * tt) * np.exp(-tt * 30)) * e
    add(pluck, st, sig * (0.10 if k % 2 == 0 else 0.07))

kick = np.zeros(N)
hats = np.zeros(N)
for k in range(int(DUR / BEAT)):
    st = k * BEAT
    if not (6.1 <= st < 22.0):
        continue
    n = int(0.3 * SR)
    tt = np.arange(n) / SR
    ph = 2 * np.pi * np.cumsum(48 + 90 * np.exp(-tt * 30)) / SR
    add(kick, st, np.sin(ph) * np.exp(-tt * 9) * 0.34)
    nh = int(0.05 * SR)
    add(hats, st + BEAT / 2, rng.standard_normal(nh) * np.exp(-np.arange(nh) / SR * 80) * 0.02)
hats = lowpass_fft(highpass_fft(hats, 7000), 12000)

# ------------------------------------------------------------------ effects (same key, same space)
fx = np.zeros(N)


def blip(f, length=0.18, decay=28, bright=0.25):
    n = int(length * SR)
    tt = np.arange(n) / SR
    e = np.exp(-tt * decay) * np.minimum(1, tt / 0.002)
    return (np.sin(2 * np.pi * f * tt) + bright * np.sin(2 * np.pi * 2 * f * tt)) * e


def whoosh(length=0.6, peak=0.75):
    n = int(length * SR)
    tt = np.arange(n) / SR
    e = np.sin(np.pi * np.clip(tt / length, 0, 1)) ** 2
    e *= np.where(tt / length < peak, (tt / length) / peak, 1)
    return lowpass_fft(rng.standard_normal(n), 1800) * e


k_rng = np.random.default_rng(9)
PENTA = [74, 76, 78, 81, 83, 86]  # D major pentatonic


def keyclick(start, gain):
    """A soft key: short high-passed noise plus a quiet pitched tick from the key."""
    n = int(0.03 * SR)
    noise = k_rng.standard_normal(n) * np.exp(-np.arange(n) / SR * 260)
    click = highpass_fft(np.pad(noise, (0, 1500)), 2200)
    add(fx, start, click, gain)
    add(fx, start, blip(hz(PENTA[k_rng.integers(len(PENTA))] + 12), 0.06, 70, 0.1), gain * 0.35)


# hook: the headline being typed (line 1 at 0.25 + 0.075k; line 2 is fast, so every other key)
for k in range(14):
    keyclick(0.25 + 0.075 * k, 0.05)
for k in range(0, 23, 2):
    keyclick(1.45 + 0.032 * k, 0.035)

# transitions
for c in (3.3, 6.1, 13.6, 16.6, 19.4):
    add(fx, c - 0.45, whoosh(0.7), 0.03)

# reveal: `typerush` typed at the prompt, Enter, then menu moves (j j j k k k)
for k in range(8):
    keyclick(3.75 + 0.06 * k, 0.045)
add(fx, 4.32, blip(hz(74), 0.12, 40, 0.5), 0.06)
for k, rt in enumerate((1.23, 1.5, 1.77, 2.07, 2.33, 2.63)):
    add(fx, rt + 3.45, blip(hz([81, 83, 86, 83, 81, 78][k]), 0.1, 45), 0.035)

# typing take: every recorded keystroke (recording time -> video time)
for kt in json.load(open(os.path.join(HERE, "keys.json"))):
    keyclick(kt - 0.95 + 6.1, 0.045)
# results screen: a bright D major chime
for m in (74, 78, 81, 86):
    add(fx, 12.22, blip(hz(m), 0.9, 4.5), 0.035)

# theme cuts: a soft tick on each cut, rising
for k in range(1, 4):
    add(fx, 13.72 + 0.72 * k, blip(hz([78, 81, 86][k - 1]), 0.25, 18), 0.05)

# outro: swell into a D major bloom
for m in (50, 57, 62, 66, 69, 74):
    nn = int(3.4 * SR)
    tt = np.arange(nn) / SR
    e = np.minimum(1, tt / 0.04) * np.exp(-tt * 1.0)
    add(fx, 19.5, np.sin(2 * np.pi * hz(m) * tt) * e, 0.04)


# ------------------------------------------------------------------ shared reverb on pluck + fx
def reverb(x, seconds=1.6, wet=0.26):
    n = int(seconds * SR)
    tt = np.arange(n) / SR
    ir = rng.standard_normal(n) * np.exp(-tt * 3.4)
    ir = lowpass_fft(ir, 5000)
    ir /= np.sqrt(np.sum(ir**2))
    L = len(x) + n
    nfft = 1 << (L - 1).bit_length()
    y = np.fft.irfft(np.fft.rfft(x, nfft) * np.fft.rfft(ir, nfft), nfft)[: len(x)]
    return x * (1 - wet) + y * wet * 0.9


bus = reverb(pluck + fx)
pad_r = reverb(pad, 2.4, 0.35)

# ------------------------------------------------------------------ arrangement + mix
pad_g = automation([(0, 0.4), (3.0, 0.75), (19.2, 0.8), (19.5, 1.0), (DUR, 1.0)])
master = automation([(0, 0), (0.05, 1), (21.9, 1), (DUR, 0)])

mix = (pad_r * pad_g + bass + kick + hats + bus) * master

left = mix + 0.004 * np.roll(mix, 240)
right = mix + 0.004 * np.roll(mix, 410)
st = np.stack([left, right], 1)
st = np.tanh(st * 1.2) / 1.2
st *= 0.89 / np.max(np.abs(st))

with wave.open(os.path.join(HERE, "music.wav"), "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes((st * 32767).astype("<i2").tobytes())
print("wrote music.wav", st.shape[0] / SR, "s")
