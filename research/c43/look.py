"""C43: a flight's track from a c43.toml csv, every STEP rows: frame, the pad's sticks, the helicopter's
position and velocity, its distance to the prop, the record's entry count, the prop's enabled word,
position and velocity; and every row where the prop's enabled, velocity or the record count changes.

usage: python3 -I research/c43/look.py CSV [STEP]
"""
import csv
import math
import sys

STEP = int(sys.argv[2]) if len(sys.argv) > 2 else 10


def f4(r, k):
    return [float(r[k + '.' + c]) for c in 'xyzw']


rows = list(csv.DictReader(open(sys.argv[1])))
print(len(rows), 'rows')
last = None
for i, r in enumerate(rows):
    p, v = f4(r, 'body_pos'), f4(r, 'body_vel')
    q, qv = f4(r, 'prop_pos'), f4(r, 'prop_vel')
    d = math.dist(p[:3], q[:3])
    x, y, z, w = f4(r, 'body_quat')
    fx, fz = 2 * (x * z + w * y), 1 - 2 * (x * x + y * y)      # the nose, local +z, in the world
    yaw = math.degrees(math.atan2(fx, fz))
    key = (r['prop_enabled'], r['rec_count'], r['prop_rec_count'], tuple(round(x, 3) for x in qv[:3]))
    if i % STEP == 0 or key != last:
        print('%s yaw %6.1f st %s  heli %.1f %.1f %.1f  v %.1f %.1f %.1f  d %.1f  rec %s | prop on %s rec %s at %.2f %.2f %.2f v %.2f %.2f %.2f'
              % (r['frame_counter'], yaw, ' '.join('%.2f' % x for x in f4(r, 'stick')[:3]), *p[:3], *v[:3], d,
                 r['rec_count'], r['prop_enabled'], r['prop_rec_count'], *q[:3], *qv[:3]))
    last = key
