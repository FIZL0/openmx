"""S240: print a capture's rows: the frame, ball 1 (enabled, position, velocity, its record's touch bytes)
and the nearest bone (its number, distance from the ball, position, velocity).

usage: python3 -B research/s240/show.py NAME [FIRST [COUNT]]"""
import csv
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
rows = list(csv.DictReader(open(ROOT / 'context' / 'capture' / 's240' / (sys.argv[1] + '.csv'))))
first = int(sys.argv[2]) if len(sys.argv) > 2 else 0
count = int(sys.argv[3]) if len(sys.argv) > 3 else 60


def v(r, k):
    try:
        return tuple(float(r[k + '.' + a]) for a in 'xyz')
    except (KeyError, ValueError):
        return None


fmt = lambda t: ' '.join('%8.2f' % x for x in t) if t else '-'
for r in rows[first:first + count]:
    ball = v(r, 'b1_pos')
    d, k = min((math.dist(v(r, 'bone%d_pos' % k), ball), k) for k in range(9))
    print(r['frame_counter'], '| ball', r['b1_en'], fmt(ball), fmt(v(r, 'b1_vel')), r['b1_touch'][:16],
          '| bone %d d %.2f' % (k, d), fmt(v(r, 'bone%d_pos' % k)), fmt(v(r, 'bone%d_vel' % k)))
