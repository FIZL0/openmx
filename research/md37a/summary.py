"""MD37a: one line per sampled row of an ATV capture (decoded columns only).

  python3 research/md37a/summary.py NAME [EVERY]     reads context/capture/md37a/NAME.csv
"""
import collections
import csv
import os
import sys

csv.field_size_limit(1 << 24)
name = sys.argv[1]
every = int(sys.argv[2]) if len(sys.argv) > 2 else 10
path = os.path.join('context', 'capture', 'md37a', name + '.' + 'csv')
rows = list(csv.DictReader(open(path)))
fr = [int(r['frame']) for r in rows]
print(name, len(rows), 'rows, frames', fr[0], '..', fr[-1], 'steps', dict(collections.Counter(b - a for a, b in zip(fr, fr[1:]))))


def g(r, k):
    return float(r[k])


for i, r in enumerate(rows):
    if i % every and i != len(rows) - 1:
        continue
    t = ''.join(r[f'wheel{w}.touch'] for w in range(4))
    b = ''.join(r[f'wheel{w}.flag_b4'] for w in range(4))
    print(f"{r['frame']:>7} pos {g(r,'pos.x'):8.2f} {g(r,'pos.y'):7.2f} {g(r,'pos.z'):8.2f} spd {g(r,'speed'):6.2f} "
          f"eul {g(r,'euler.x'):6.3f} {g(r,'euler.y'):6.3f} {g(r,'euler.z'):6.3f} gnd {r['ground']} touch {t} b4 {b} "
          f"stick {g(r,'stick.x'):5.2f} {g(r,'stick.y'):5.2f} ctl {g(r,'ctl.x'):.2f} {g(r,'ctl.y'):.2f} {g(r,'ctl.z'):.2f} {g(r,'ctl.w'):.2f} wreck {r['wreck']}")
