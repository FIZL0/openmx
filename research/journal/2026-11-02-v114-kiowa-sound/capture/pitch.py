"""V114: the rate the original plays the Kiowa's engine loop at, window by window, off a Video Capture's
sound. Each window of the capture (48 kHz mono s16) is matched against the loop (its own rate, mono
s16) resampled at a range of pitches; the pitch whose copy correlates best, at any offset, is the one
playing. Music and wind are in the capture too, so the score says how sure each window is.

usage: python3 pitch.py CAPTURE.pcm LOOP.pcm LOOP_RATE [WINDOW_S] [STEP_S]
"""
import sys
import numpy as np

cap = np.frombuffer(open(sys.argv[1], 'rb').read(), dtype=np.int16).astype(np.float64)
loop = np.frombuffer(open(sys.argv[2], 'rb').read(), dtype=np.int16).astype(np.float64)
own = float(sys.argv[3])
win = float(sys.argv[4]) if len(sys.argv) > 4 else 0.5
step = float(sys.argv[5]) if len(sys.argv) > 5 else 0.5
OUT = 48000.0


def played(p, n):
    """The loop played at `p` times its own rate, looping, `n` samples of 48 kHz from its start."""
    t = np.arange(n) * (own * p / OUT)
    i = np.floor(t).astype(np.int64)
    f = t - i
    a = loop[i % len(loop)]
    b = loop[(i + 1) % len(loop)]
    return a + (b - a) * f


def best_corr(seg, ref):
    """The largest normalised cross-correlation of `seg` against `ref` (longer), at any offset."""
    n = len(ref) + len(seg)
    size = 1 << (n - 1).bit_length()
    c = np.fft.irfft(np.fft.rfft(ref, size) * np.conj(np.fft.rfft(seg, size)), size)[: len(ref) - len(seg) + 1]
    # The reference's energy under each offset.
    sq = np.concatenate([[0.0], np.cumsum(ref * ref)])
    e = np.sqrt(sq[len(seg):] - sq[: len(ref) - len(seg) + 1]) * np.sqrt(np.sum(seg * seg))
    e[e == 0] = np.inf
    return float(np.max(c / e))


pitches = np.linspace(0.96, 1.04, 81)
n = int(win * OUT)
# A reference a loop and a window long at each pitch, so every phase of the loop is in it.
refs = {p: played(p, int(len(loop) * OUT / (own * 0.96)) + n) for p in pitches}
t = 0.0
while (t + win) * OUT <= len(cap):
    seg = cap[int(t * OUT): int(t * OUT) + n]
    seg = seg - seg.mean()
    scores = [(best_corr(seg, refs[p] - refs[p].mean()), p) for p in pitches]
    s, p = max(scores)
    # The three rates the port's rule plays: descending, holding and climbing (PitchVariance 400).
    rule = {k: dict((q, c) for c, q in scores)[min(pitches, key=lambda q: abs(q - (1 + k / own)))] for k in (-400, 0, 400)}
    said = "  ".join(f"{k:+d}:{c:.3f}" for k, c in rule.items())
    print(f"{t:6.2f}s  pitch {p:.3f}  ({(p - 1) * own:+6.0f} Hz)  score {s:.3f}   rule {said}")
    t += step
