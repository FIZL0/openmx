"""C15: one line per sampled row of a bike capture (decoded columns only).

  python3 -I research/c15/summary.py CSV [EVERY]
"""
import collections
import csv
import sys

csv.field_size_limit(1 << 24)
path = sys.argv[1]
every = int(sys.argv[2]) if len(sys.argv) > 2 else 10
rows = list(csv.DictReader(open(path)))
fr = [int(r['vsync']) for r in rows]
print(path, len(rows), 'rows, vsync', fr[0], '..', fr[-1], 'steps', dict(collections.Counter(b - a for a, b in zip(fr, fr[1:]))))


def g(r, k):
    return float(r[k])


for i, r in enumerate(rows):
    if i % every and i != len(rows) - 1:
        continue
    print(f"{i:4} {r['vsync']:>6} pos {g(r,'pos.x'):8.2f} {g(r,'pos.y'):7.2f} {g(r,'pos.z'):8.2f} spd {g(r,'speed'):6.2f} "
          f"eul {g(r,'euler.x'):6.3f} {g(r,'euler.y'):6.3f} {g(r,'euler.z'):6.3f} gnd {r['ground']} "
          f"stick {g(r,'dpad_stick.z'):5.2f} {g(r,'dpad_stick.w'):5.2f} brk {g(r,'ctl.z'):.2f} gas {g(r,'ctl.w'):.2f} "
          f"trick {r['trick']} flip {r['flip'][:16]} wreck {r['wreck']}")
