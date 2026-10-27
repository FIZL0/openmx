"""Which tape field each capture row's pad buffers hold (C10): the offset and the port mapping.

    python3 -B research/c10/padcheck.py CAPTURE [FRAME...]   # capture/2p/<CAPTURE>.csv

For every row, takes the captured pad buffers (pad0_a, pad1_a: the bytes the game polled) and finds the tape
fields within +-40 of 2 * (frame - FIRST) whose port bytes 0..5 (buttons and sticks) equal them. Prints the
histogram of (buffer, port, field - 2 * tick) over rows where the match is unique, then the rows asked for.
Research, not product.
"""
import collections
import csv
import os
import sys

FIRST = 127882
HEAD = 570
tape = open(os.path.join('context', 'recordings', 'split', '2p.p2m2'), 'rb').read()
n = (len(tape) - HEAD) // 36


def pad(field, port):
    o = HEAD + 36 * field + 18 * port
    return tape[o:o + 6]


hist = collections.Counter()
want = set(int(a) for a in sys.argv[2:])
with open(os.path.join('context', 'capture', '2p', sys.argv[1] + '.csv')) as f:
    for r in csv.DictReader(f):
        try:
            fc = int(r['frame_counter'])
        except ValueError:
            continue
        t = fc - FIRST
        for buf in ('pad0_a', 'pad1_a', 'pad0_b', 'pad1_b'):
            raw = bytes.fromhex(r[buf].replace(' ', ''))
            # the game's buffer: a header of 2 bytes (status, 0x73/0x79) before the buttons
            for skip in (2, 3, 4):
                got = raw[skip:skip + 6]
                hits = [(port, fld - 2 * t) for fld in range(max(0, 2 * t - 40), min(n, 2 * t + 40))
                        for port in (0, 1) if pad(fld, port) == got]
                ports = set(h[0] for h in hits)
                if len(ports) == 1 and got != b'\xff\xff\x7f\x7f\x7f\x7f':
                    hist[(buf, skip, hits[0][0], min(h[1] for h in hits), max(h[1] for h in hits))] += 1
            if fc in want:
                print(fc, buf, raw.hex())
        if fc in want:
            for d in range(-6, 3):
                print('   field 2t%+d  p0 %s  p1 %s' % (d, pad(2 * t + d, 0).hex(), pad(2 * t + d, 1).hex()))
for k, v in hist.most_common(25):
    print(v, k)
