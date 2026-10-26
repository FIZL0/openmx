"""P74: print the capture's changes: each entry's flag/on byte and carry moves, against the feed words.

usage: python3 -B look.py <run.csv> [from] [to] [columns...]

With no columns, prints a row whenever any entry's flag or switch byte changes, with the feed words.
With columns, prints those columns for every row in the range.
"""

import csv
import sys

rows = list(csv.DictReader(open(sys.argv[1])))
lo = int(sys.argv[2]) if len(sys.argv) > 2 else 0
hi = int(sys.argv[3]) if len(sys.argv) > 3 else 1 << 30
cols = sys.argv[4:]
rows = [r for r in rows if lo <= int(r['frame_counter']) <= hi]

FEED = ['wreck_reason', 's888', 's88c', 's860', 's91c', 's930', 's870', 's880', 's910', 's911',
        's914', 's918', 's928', 's92c', 't0_flags', 't0_zone', 't1_flags', 't1_zone']


def short(v):
    try:
        f = float(v)
    except ValueError:
        return v
    return v if f == int(f) and abs(f) < 1e6 else f'{f:.3f}'


if cols:
    for r in rows:
        print(r['frame_counter'], ' '.join(f'{c}={short(r[c])}' for c in cols))
    sys.exit()

prev = None
for r in rows:
    state = [(r[f'e{i}_flag'], r[f'e{i}_on']) for i in range(15)]
    if state != prev:
        s = ' '.join(f'{i}:{f}/{int(o):02x}' for i, (f, o) in enumerate(state))
        print(r['frame_counter'], s, '|', ' '.join(f'{c}={short(r[c])}' for c in FEED))
    prev = state
