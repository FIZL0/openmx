"""S242: print a capture's rows: the frame, and balls A and B (active, enabled, position, velocity, rates,
the record's touch bytes), with the distance between their centres.

usage: python3 -B research/s242/show.py NAME A B [FIRST [COUNT]]"""
import csv
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
rows = list(csv.DictReader(open(ROOT / 'context' / 'capture' / 's242' / (sys.argv[1] + '.csv'))))
A, B = sys.argv[2], sys.argv[3]
first = int(sys.argv[4]) if len(sys.argv) > 4 else 0
count = int(sys.argv[5]) if len(sys.argv) > 5 else 60


def v(r, k):
    try:
        return tuple(float(r[k + '.' + a]) for a in 'xyz')
    except (KeyError, ValueError):
        return None


fmt = lambda t: ' '.join('%8.2f' % x for x in t) if t else '-'
for r in rows[first:first + count]:
    pa, pb = v(r, 'b%s_pos' % A), v(r, 'b%s_pos' % B)
    print(r['frame_counter'], 'd %.2f' % math.dist(pa, pb), *(
        '| %s %s%s %s %s %s %s' % (k, r['b%s_active' % k], r['b%s_en' % k], fmt(v(r, 'b%s_pos' % k)),
                                  fmt(v(r, 'b%s_vel' % k)), fmt(v(r, 'b%s_rates' % k)), r['b%s_touch' % k][:8])
        for k in (A, B)))
