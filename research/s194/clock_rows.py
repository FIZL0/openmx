"""S194: each row of a watch/s129-train.toml capture numbered by its frame and by the bike's clock.

usage: python3 -B research/s194/clock_rows.py CAPTURE.csv

The bike's clock (`bike_clock`, rider +0x13fc) gains the frame's dt at the end of every player update,
so the number of f32 `clock + dt` additions from row 0 to a row is how many ticks its bike is past row
0's. Prints the rows whose bike step is not one tick, the rows that repeat the row before (every
column but the frame counter and the pad's), and the held controls' column sums.
"""

import csv
import struct
import sys


def f32(x):
    return struct.unpack('<f', struct.pack('<f', float(x)))[0]


rows = list(csv.DictReader(open(sys.argv[1])))
cols = list(rows[0].keys())
bike = [c for c in cols if c != 'frame_counter' and c != 'frame' and not c.startswith(('stamp_', 'pad_'))]
dt = f32(rows[0]['dt'])
c0 = f32(rows[0]['bike_clock'])
steps = []
for r in rows:
    c, k = c0, 0
    want = f32(r['bike_clock'])
    while c < want and k < 100000:
        c, k = f32(c + dt), k + 1
    steps.append(k if c == want else None)
odd = []
for i in range(1, len(rows)):
    frame = int(rows[i]['frame_counter']) - int(rows[i - 1]['frame_counter'])
    tick = None if steps[i] is None or steps[i - 1] is None else steps[i] - steps[i - 1]
    same = all(rows[i][c] == rows[i - 1][c] for c in bike)
    if tick != frame or same:
        odd.append((i, rows[i]['frame_counter'], frame, tick, same))
print(len(rows), 'rows; dt', dt, '; clock off the f32 sum on', sum(s is None for s in steps), 'rows')
for o in odd:
    print('row %d frame %s: frame step %d, bike step %s, repeats the row before: %s' % o)
for c in cols:
    if c.startswith('ctl') and c.endswith('.held'):
        n = sum(r[c] == '1' for r in rows)
        if n:
            print(c, 'held on', n, 'rows')
print('wreck rows', sum(r['wreck_reason'] != '0' for r in rows))
