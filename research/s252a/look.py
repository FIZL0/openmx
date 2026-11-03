"""S252a: summarise a context/capture/s252a csv round the truck's wake.

usage: python3 -B research/s252a/look.py NAME [FIRST LAST]

Per row: the frame, the wreck reason, the truck's physical flag, clock and position, the wreck bike's
distance from the truck's centre and its velocity, the nearest bone (index, distance, velocity) and the
bone collisions' touched mask."""
import csv
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
rows = list(csv.DictReader(open(ROOT / 'context' / 'capture' / 's252a' / (sys.argv[1] + '.csv'))))
lo = int(sys.argv[2]) if len(sys.argv) > 2 else 0
hi = int(sys.argv[3]) if len(sys.argv) > 3 else 1 << 30


def v(r, k):
    return tuple(float(r['%s.%s' % (k, c)]) for c in 'xyz')


for r in rows:
    f = int(r['frame_counter'])
    if not lo <= f <= hi:
        continue
    t = v(r, 'truck_pos')
    w = v(r, 'wreck_pos')
    bones = sorted((math.dist(v(r, 'bone%d_pos' % i), t), i) for i in range(18))
    d, i = bones[0]
    touch = sum(1 << k for k in range(9) if int(r['touch%d' % k]))
    print('%d reason %s phys %s clock %.4f truck %.3f %.3f %.3f | wreck %.2f ft vel %s | bone%d %.2f ft vel %s | touch %03x' % (
        f, r['reason'], r['truck_physical'], float(r['truck_clock']), *t, math.dist(w, t),
        ' '.join('%.2f' % x for x in v(r, 'wreck_vel')), i, d, ' '.join('%.2f' % x for x in v(r, 'bone%d_vel' % i)),
        touch))
