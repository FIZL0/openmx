"""S117: how steep the ground under each wheel is and how much of it falls across the bike, every
Nth row of a context/capture/s58-slip case.

    python3 -B slope.py CASE [EVERY]

Per row: the chassis' position, speed, each wheel's slope (acos of its contact normal's y, degrees)
and the normal's share along the bike's right (the node's right row, ch_xform m00..m02): 0.72 is a
46-degree face side on. A wheel off the ground (ray_hit 0) shows '-'."""
import csv
import math
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(here))))
path = os.path.join(root, 'context', 'capture', 's58-slip', sys.argv[1] + '.csv')
every = int(sys.argv[2]) if len(sys.argv) > 2 else 10
rows = list(csv.DictReader(open(path)))
print('frame   x       y       z      speed  f:deg across  r:deg across  slip f/r  stick-ish')
for i, r in enumerate(rows):
    if i % every:
        continue
    g = lambda k: float(r[k])
    right = [g('ch_xform.m0%d' % k) for k in range(3)]
    out = []
    for w in 'fr':
        if r[w + '_ray_hit'] in ('0', '0.0'):
            out.append('    -      - ')
            continue
        n = [g('%s_normal.%s' % (w, c)) for c in 'xyz']
        deg = math.degrees(math.acos(max(-1, min(1, n[1]))))
        out.append('%5.1f %6.2f ' % (deg, sum(a * b for a, b in zip(n, right))))
    flags = ''.join(r[w + k] for w in 'fr' for k in ('_spinning', '_locked', '_sliding'))
    print('%5s %7.1f %6.1f %7.1f %6.1f  %s %s  %s' % (r['frame_counter'], g('ch_cofg.x'), g('ch_cofg.y'),
          g('ch_cofg.z'), g('ch_speed'), out[0], out[1], flags))
