"""C12: both riders' route and the six path cars, every N rows of a 2p capture.

    python3 -B research/c12/route.py CAPTURE [EVERY]   # capture/2p/<CAPTURE>.csv

Prints frame, each rider's centre (x, z), speed, wreck reason, and the nearest car's distance and index.
Research, not product.
"""
import csv
import math
import os
import sys

every = int(sys.argv[2]) if len(sys.argv) > 2 else 120
with open(os.path.join('context', 'capture', '2p', sys.argv[1] + '.csv')) as f:
    for i, r in enumerate(csv.DictReader(f)):
        if i % every:
            continue
        try:
            fc = int(r['frame_counter'])
        except ValueError:
            continue
        cars = []
        for c in range(6):
            try:
                cars.append((float(r['car%d_xform.m30' % c]), float(r['car%d_xform.m32' % c])))
            except (KeyError, ValueError):
                pass
        out = ['%6d' % fc]
        for p in ('p1', 'p2'):
            x, z = float(r[p + '_ch_centre.x']), float(r[p + '_ch_centre.z'])
            near = min((math.hypot(x - cx, z - cz), k) for k, (cx, cz) in enumerate(cars)) if cars else (0, -1)
            out.append('%s (%8.1f %8.1f) v%5.1f w%-3s car%d %6.0f' % (p, x, z, float(r[p + '_ch_speed']),
                                                                    r.get(p + '_wreck_reason', '?'), near[1], near[0]))
        out.append('cars ' + ' '.join('(%.0f %.0f)' % c for c in cars))
        print('  '.join(out))
