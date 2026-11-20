"""C44: print a c44-plane-bike capture's plane and AI bikes every STEP ticks, with each bike's distance
to the plane's body.

usage: python3 -B research/c44/paths.py CSV [STEP]
"""
import csv
import math
import sys

STEP = int(sys.argv[2]) if len(sys.argv) > 2 else 30
rows = list(csv.DictReader(open(sys.argv[1])))
for k, r in enumerate(rows):
    if k % STEP and k != len(rows) - 1:
        continue
    p = [float(r['plane_pos.' + a]) for a in 'xyz']
    s = '%6s crash %s plane %8.2f %7.2f %8.2f' % (r['frame_counter'], r['crash'], *p)
    for pre in ('a0', 'a1', 'a3'):
        c = [float(r['%s_ch_centre.%s' % (pre, a)]) for a in 'xyz']
        v = [float(r['%s_ch_vel.%s' % (pre, a)]) for a in 'xyz']
        d = math.dist(c, p)
        s += ' | %s %8.2f %7.2f %8.2f v%5.1f d%6.1f n%s' % (pre, *c, math.hypot(v[0], v[2]), d, r[pre + '_rec_count'])
    print(s)
