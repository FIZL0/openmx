"""Row -> tape field for a 2p capture (C10): which field's two pads each tick polled.

    python3 -B research/c10/align.py CAPTURE       # capture/2p/<CAPTURE>.csv

Each row's polled pads (pad0_a, pad1_a bytes 2..7: buttons and sticks) against the tape's two ports. Per
block of 200 rows, every offset d = field - tick from -200 to 13000 is scored by the summed distance of
the block's pads to fields t + d (button bits counted 64 each, stick bytes by their difference); prints the
best offset and its mean distance, so a playback that consumes a field a tick shows a near-constant d with
a small distance. Research, not product.
"""
import csv
import os
import sys

import numpy as np

FIRST = 127882
HEAD = 570
raw = open(os.path.join('context', 'recordings', 'split', '2p.p2m2'), 'rb').read()
n = (len(raw) - HEAD) // 36
tape = np.frombuffer(raw[HEAD:HEAD + 36 * n], np.uint8).reshape(n, 36)
tape = np.concatenate([tape[:, 0:6], tape[:, 18:24]], 1).astype(np.int32)

ticks, pads = [], []
with open(os.path.join('context', 'capture', '2p', sys.argv[1] + '.csv')) as f:
    for r in csv.DictReader(f):
        try:
            fc = int(r['frame_counter'])
        except ValueError:
            continue
        ticks.append(fc - FIRST)
        pads.append(list(bytes.fromhex(r['pad0_a'].replace(' ', ''))[2:8]) +
                    list(bytes.fromhex(r['pad1_a'].replace(' ', ''))[2:8]))
ticks = np.array(ticks)
pads = np.array(pads, np.int32)


def dist(a, b):
    bits = np.unpackbits((a[..., [0, 1, 6, 7]] ^ b[..., [0, 1, 6, 7]]).astype(np.uint8)[..., None], -1).sum((-1, -2))
    sticks = np.abs(a[..., [2, 3, 4, 5, 8, 9, 10, 11]] - b[..., [2, 3, 4, 5, 8, 9, 10, 11]]).sum(-1)
    return 64 * bits + sticks


for s in range(0, len(ticks), 200):
    t = ticks[s:s + 200]
    p = pads[s:s + 200]
    best = None
    for d in range(-200, n):
        f = t + d
        ok = (f >= 0) & (f < n)
        if ok.sum() < len(t) // 2:
            continue
        score = dist(p[ok], tape[f[ok]]).mean()
        if best is None or score < best[0]:
            best = (score, d)
    print('rows %5d.. tick %5d  best d %+6d (field %5d)  mean distance %.1f' % (s, t[0], best[1], t[0] + best[1],
                                                                               best[0]))
