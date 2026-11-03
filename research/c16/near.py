"""C16: what in the flying tape's capture is near plane 0 on one row: every xyz column (riders' chassis,
bike actors, the path cars' transforms' position rows, plane 1) within RADIUS ft of plane 0's body_pos.

usage: python3 -B research/c16/near.py FRAME [RADIUS] [CSV]
"""
import csv
import math
import sys

frame = int(sys.argv[1])
radius = float(sys.argv[2]) if len(sys.argv) > 2 else 60.0
path = sys.argv[3] if len(sys.argv) > 3 else 'context/capture/c16/fly.csv'
r = next(r for r in csv.DictReader(open(path)) if int(r['frame_counter']) == frame)
p = tuple(float(r['body_pos.' + k]) for k in 'xyz')
print('plane 0', p)
cols = sorted({k.rsplit('.', 1)[0] for k in r if k.endswith('.x')})
for c in cols:
    if c == 'body_pos':
        continue
    try:
        q = tuple(float(r[c + '.' + k]) for k in 'xyz')
    except (KeyError, ValueError):
        continue
    d = math.dist(p, q)
    if d < radius:
        print('%-28s %7.1f ft  (%.1f, %.1f, %.1f)' % (c, d, *q))
xf = sorted({k.rsplit('.', 1)[0] for k in r if 'xform' in k})
for c in xf:
    keys = [k for k in r if k.startswith(c + '.')]
    vals = [float(r[k]) for k in keys]
    if len(vals) == 16:
        q = tuple(vals[12:15])
        d = math.dist(p, q)
        if d < radius:
            print('%-28s %7.1f ft  (%.1f, %.1f, %.1f) [xform row 3]' % (c, d, *q))
