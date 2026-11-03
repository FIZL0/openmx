"""F27a1: where one clip sounds in a recording, and how loud. A normalised cross-correlation of the clip
over the recording (P164's match_clips.py), then at each of the best peaks the least-squares gain of
the clip in each channel (<x, c> / <c, c>), with the window's NCC.

usage: python3 -I research/f27a1/match.py REC_STEREO.pcm CLIP.pcm [PEAKS]

Both s16 little-endian at 22050 Hz: the recording stereo interleaved, the clip mono (the disc's VAG
decoded by the shell's audio::Library). Gains are in the clip's own scale: 1.0 is the clip at full
level in that channel.

With --pair, each peak is also fitted as two copies of the clip, the second D samples after the first
(D searched over ±2 ticks), by least squares on the mono mix: the best D, both gains and how much of
the window's energy is left over, beside the one-copy fit's."""
import sys

import numpy as np

R = 22050
pair = '--pair' in sys.argv
args = [a for a in sys.argv[1:] if a != '--pair']
st = np.fromfile(args[0], dtype='<i2').astype(np.float64).reshape(-1, 2)
c = np.fromfile(args[1], dtype='<i2').astype(np.float64)
peaks = int(args[2]) if len(args) > 2 else 4
x = st.mean(axis=1)


def ncc(x, c):
    n = len(x) + len(c)
    N = 1 << (n - 1).bit_length()
    cc = np.fft.irfft(np.fft.rfft(x, N) * np.conj(np.fft.rfft(c, N)), N)[:len(x) - len(c) + 1]
    e = np.concatenate([[0], np.cumsum(x * x)])
    win = e[len(c):] - e[:-len(c)]
    return cc / (np.sqrt(np.maximum(win, 1e-9)) * np.sqrt((c * c).sum()))


def fit_pair(k):
    """Two copies, at k + a and k + a + d, a in ±2 ticks and d in 1..2 ticks."""
    tick = R // 30
    n = len(c) + 3 * tick
    best = None
    for a in range(-2 * tick, 2 * tick + 1, 8):
        for d in range(8, 2 * tick + 1, 8):
            lo = k + a
            if lo < 0 or lo + n > len(x):
                continue
            w = x[lo:lo + n]
            A = np.zeros((n, 2))
            A[:len(c), 0] = c
            A[d:d + len(c), 1] = c
            g, *_ = np.linalg.lstsq(A, w, rcond=None)
            left = ((w - A @ g) ** 2).sum() / (w * w).sum()
            if best is None or left < best[0]:
                best = (left, a, d, g)
    if best is None:
        return 'pair: no room'
    w = x[k:k + n]
    one = ((w[:len(c)] - c * (w[:len(c)] @ c) / cc) ** 2).sum() / (w[:len(c)] ** 2).sum()
    left, a, d, g = best
    return 'pair: first at %+.4f s, second %.4f s (%.2f ticks) after, gains %.3f %.3f, %.0f%% left (one copy %.0f%%)' % (
        a / R, d / R, d * 30 / R, g[0], g[1], 100 * left, 100 * one)


cc = (c * c).sum()
r = ncc(x, c)
print('recording %.3f s, clip %.4f s' % (len(x) / R, len(c) / R))
for _ in range(peaks):
    k = int(np.argmax(r))
    w = st[k:k + len(c)]
    gl, gr = (w[:, 0] * c).sum() / cc, (w[:, 1] * c).sum() / cc
    rest = w - np.outer(c, [gl, gr])
    print('  %.3f s  ncc %.3f  gain L %.3f R %.3f  (window rms %.0f, left over %.0f)' % (
        k / R, r[k], gl, gr, np.sqrt((w ** 2).mean()), np.sqrt((rest ** 2).mean())))
    if pair:
        print('    ' + fit_pair(k))
    r[max(0, k - R // 4):k + R // 4] = -1
