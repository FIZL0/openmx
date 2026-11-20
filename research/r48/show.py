"""R48: print a r48-truck capture every STEP rows (default 10), and its gaps and wake rows.

usage: python3 -B research/r48/show.py CSV [STEP]
"""
import csv, sys

rows = list(csv.DictReader(open(sys.argv[1])))
step = int(sys.argv[2]) if len(sys.argv) > 2 else 10


def v(r, k):
    return '(%8.2f %7.2f %8.2f)' % tuple(float(r['%s.%s' % (k, c)]) for c in 'xyz')


prev = None
for r in rows:
    f = int(r['frame_counter'])
    if prev is not None and f != prev + 1:
        print('gap', prev, f)
    prev = f
awake = None
for i, r in enumerate(rows):
    if i % step == 0 or r['truck_awake'] != awake:
        print(r['frame_counter'], 'dt', r['dt'][:7], 'crash', r['crash'], 'n', r['plane_contacts'], 'cmd %5.3f' % float(r['cmd_34c']),
              'plane', v(r, 'body_pos'), v(r, 'body_vel'), 'truck', r['truck_awake'], v(r, 'truck_pos'), v(r, 'truck_vel'))
    awake = r['truck_awake']
