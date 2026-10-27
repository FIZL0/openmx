"""Vote for the tape field each captured row's pads came from, over the whole tape (C10).

    python3 -B research/c10/offset.py CAPTURE      # capture/2p/<CAPTURE>.csv

Each row's polled buffer (pad0_a, pad1_a: bytes 2..7, buttons and sticks) is looked up among every tape
field's port bytes 0..5; each hit votes for (buffer, port, field - 2 * tick). A playback that rode the tape
shows one big winner per buffer near 0; the votes per 500-row block say whether and where it moves.
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
where = collections.defaultdict(list)
for fld in range(n):
    for port in (0, 1):
        o = HEAD + 36 * fld + 18 * port
        where[(port, tape[o:o + 6])].append(fld)

total = collections.Counter()
blocks = collections.defaultdict(collections.Counter)
with open(os.path.join('context', 'capture', '2p', sys.argv[1] + '.csv')) as f:
    for r in csv.DictReader(f):
        try:
            fc = int(r['frame_counter'])
        except ValueError:
            continue
        t = fc - FIRST
        for buf in ('pad0_a', 'pad1_a'):
            got = bytes.fromhex(r[buf].replace(' ', ''))[2:8]
            for port in (0, 1):
                flds = where.get((port, got), [])
                if 0 < len(flds) < 40:
                    for fld in flds:
                        total[(buf, port, fld - 2 * t)] += 1
                        blocks[(buf, t // 500)][(port, fld - 2 * t)] += 1
for k, v in total.most_common(12):
    print(v, k)
for k in sorted(blocks):
    print(k, blocks[k].most_common(3))
