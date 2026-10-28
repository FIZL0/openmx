"""S162: summarise context/capture/s162/NAME.csv — the bike's place, heading, speed along its heading and
the wreck reason, every EVERY rows, and the rows round the first wreck.

usage: python3 -B research/s162/look.py NAME [EVERY]"""
import csv
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
rows = list(csv.DictReader(open(ROOT / 'context' / 'capture' / 's162' / (sys.argv[1] + '.csv'))))
every = int(sys.argv[2]) if len(sys.argv) > 2 else 25
print(len(rows), 'rows; ctl_src', sorted({r['rider_ctl_src'] for r in rows}))


def f(r, k):
    return float(r[k])


first = next((i for i, r in enumerate(rows) if int(float(r['wreck_reason']))), None)
low = 0.0
for i, r in enumerate(rows):
    fwd = [f(r, 'bike_xform.m2%d' % k) for k in range(3)]
    v = [f(r, 'ch_vel.' + c) for c in 'xyz']
    signed = v[0] * fwd[0] + v[2] * fwd[2]
    low = min(low, signed)
    near = first is not None and first - 3 <= i <= first + 1
    if i % every == 0 or near or i == len(rows) - 1:
        print('%4d %6s pos %8.2f %7.2f %8.2f heading %7.2f signed %7.2f vy %7.2f reason %s%s' % (
            i, r['frame_counter'], f(r, 'bike_xform.m30'), f(r, 'bike_xform.m31'), f(r, 'bike_xform.m32'),
            math.degrees(math.atan2(fwd[0], fwd[2])), signed, v[1], r['wreck_reason'], '  <- first wreck' if i == first else ''))
print('first wreck row', first, 'lowest signed speed %.2f' % low)
