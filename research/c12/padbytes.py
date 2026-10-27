"""C12: do the pads the game polled equal the tape's bytes, or a transform of them?

    python3 -B research/c12/padbytes.py CAPTURE     # capture/2p/<CAPTURE>.csv

For each row, the tape fields t-2..t+1 (t = frame_counter - FIRST): which one matches the polled pad bytes (buttons, sticks and
the 12 pressure bytes) exactly, per port; for rows with no exact match, the byte-by-byte pairs (tape, polled) of the nearest field.
Research, not product.
"""
import collections
import csv
import os
import sys

FIRST = 127882
HEAD = 570
raw = open(os.path.join('context', 'recordings', 'split', '2p.p2m2'), 'rb').read()
n = (len(raw) - HEAD) // 36
fields = [raw[HEAD + 36 * i:HEAD + 36 * (i + 1)] for i in range(n)]

exact = [collections.Counter(), collections.Counter()]
pairs = [collections.Counter(), collections.Counter()]
shown = 0
with open(os.path.join('context', 'capture', '2p', sys.argv[1] + '.csv')) as f:
    for r in csv.DictReader(f):
        try:
            t = int(r['frame_counter']) - FIRST
        except ValueError:
            continue
        for port, col in enumerate(('pad0_a', 'pad1_a')):
            got = bytes.fromhex(r[col].replace(' ', ''))
            polled = got[2:20]
            best = None
            for d in (-2, -1, 0, 1):
                if 0 <= t + d < n:
                    tp = fields[t + d][18 * port:18 * port + 18]
                    dist = sum(abs(a - b) for a, b in zip(tp, polled))
                    if best is None or dist < best[0]:
                        best = (dist, d, tp)
            if best[0] == 0:
                exact[port][best[1]] += 1
            else:
                exact[port]['none'] += 1
                for i, (a, b) in enumerate(zip(best[2], polled)):
                    if a != b:
                        pairs[port][(i, a, b)] += 1
                if shown < 12:
                    shown += 1
                    print('t %5d port %d tape %s polled %s full %s' % (t, port, best[2].hex(), polled.hex(), got.hex()))
for port in (0, 1):
    print('port', port, dict(exact[port]))
    print('  commonest differing (byte, tape, polled):', pairs[port].most_common(15))
