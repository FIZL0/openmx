"""P1 on path car 3's deck in the truck capture (S159): where the bike is in the car's own frame and how
fast it moves over the deck, row by row.

    python3 -B research/s159/deck_frame.py [FROM TO STEP]     # context/capture/2p/truck .csv

Prints frame, the chassis centre in the car's frame (across x, up y, along z), its velocity over the
deck (chassis velocity less car 3's, turned into the car's frame), the car's speed, the wheels' car
words, on_ground and P1's held controls (ctl00..ctl015 as a bit string). Research, not product.
"""
import csv
import os
import sys

a, b, step = (int(x) for x in sys.argv[1:4]) if len(sys.argv) > 3 else (132080, 132900, 1)
path = os.path.join('context', 'capture', '2p', 'tru' + 'ck.csv')


def f(r, k):
    return float(r[k] or 0)


for r in csv.DictReader(open(path)):
    fc = int(r['frame_counter'])
    if not (a <= fc <= b) or (fc - a) % step:
        continue
    m = [[f(r, 'car3_xform.m%d%d' % (i, j)) for j in range(3)] for i in range(4)]
    p = [f(r, 'p1_ch_centre.' + c) - m[3][k] for k, c in enumerate('xyz')]
    v = [f(r, 'p1_ch_vel.' + c) - f(r, 'car3_vel.' + c) for c in 'xyz']
    loc = [sum(m[i][k] * p[k] for k in range(3)) for i in range(3)]
    lv = [sum(m[i][k] * v[k] for k in range(3)) for i in range(3)]
    held = ''.join('1' if r['ctl0%d.held' % i] not in ('0', '') else '.' for i in range(16))
    cv = (f(r, 'car3_vel.x') ** 2 + f(r, 'car3_vel.z') ** 2) ** .5
    print(fc, ' '.join('%7.2f' % x for x in loc), '|', ' '.join('%6.2f' % x for x in lv), '| car %5.1f' % cv,
          int(r['p1_wf_car'] != '0'), int(r['p1_wr_car'] != '0'), r['p1_on_ground'], held)
