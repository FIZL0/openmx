"""S197b: print a capture's rows: the frame, the wreck reason, the chassis, ball BALL and the wreck bike.

usage: python3 -B research/s197b/show.py NAME BALL [FIRST [COUNT]]"""
import csv
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
rows = list(csv.DictReader(open(ROOT / 'context' / 'capture' / 's197b' / (sys.argv[1] + '.csv'))))
b = 'b%s_' % sys.argv[2]
first = int(sys.argv[3]) if len(sys.argv) > 3 else 0
count = int(sys.argv[4]) if len(sys.argv) > 4 else 40


def v(r, k):
    try:
        return tuple(float(r[k + '.' + a]) for a in 'xyz')
    except (KeyError, ValueError):
        return None


for r in rows[first:first + count]:
    ch, ball, wb = v(r, 'ch_cofg'), v(r, b + 'pos'), v(r, 'wb_pos')
    near = math.dist(wb, ball) if wb and ball and r['wreck_reason'] != '0' else float('nan')
    fmt = lambda t: ' '.join('%8.2f' % x for x in t) if t else '-'
    print(r['frame_counter'], 'wr', r['wreck_reason'], 'ch', fmt(ch), '| ball', r[b + 'en'], fmt(ball), fmt(v(r, b + 'vel')),
          '| wb', fmt(wb), 'd %.2f' % near, r.get('wb_touch', '')[:16])
