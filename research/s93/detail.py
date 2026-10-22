"""Print both riders around a tick of a split capture (S93): capture first last.

    python3 -B research/s93/detail.py split/splitscreen 3398 3410

Per row: each rider's position (bike xform row 3), chassis velocity, heading (the bike's forward
axis, xform row 2, as a compass angle), on-ground, wreck reason, contact count and the records its
actor hits name; then the gap between the two and the angle between their velocities.
"""
import csv, math, os, sys

rows = list(csv.DictReader(open(os.path.join('context', 'capture', sys.argv[1] + '.csv'))))
lo, hi = int(sys.argv[2]), int(sys.argv[3])


def vec(r, name):
    return [float(r[name + '.' + c]) for c in 'xyz']


def row3(r, p, i):
    return [float(r['p%d_bike_xform.m%d%d' % (p, i, k)]) for k in range(3)]


def ang(v):
    return math.degrees(math.atan2(v[0], v[2]))


for r in rows:
    t = int(r['frame_counter'])
    if not lo <= t <= hi:
        continue
    parts = []
    for p in (1, 2):
        pos, vel, fwd = row3(r, p, 3), vec(r, 'p%d_ch_vel' % p), row3(r, p, 2)
        n = int(r['p%d_rec_count' % p])
        others = sorted({r['p%d_c%d.other' % (p, k)] for k in range(min(n, 11))} - {'0'})
        parts.append('p%d (%.1f %.1f %.1f) v (%.1f %.1f %.1f) hd %4.0f gnd %s wr %s n %d %s' % (
            p, *pos, *vel, ang(fwd), r['p%d_on_ground' % p], r['p%d_wreck_reason' % p], n,
            ','.join('%x' % int(o) for o in others)))
    d = [a - b for a, b in zip(row3(r, 2, 3), row3(r, 1, 3))]
    v1, v2 = vec(r, 'p1_ch_vel'), vec(r, 'p2_ch_vel')
    print('%d %s | gap %.2f (p2-p1 dir %.0f, dy %.2f) vel angle %.0f' % (
        t, ' | '.join(parts), math.hypot(d[0], d[2]), ang(d), d[1], (ang(v2) - ang(v1) + 540) % 360 - 180))
