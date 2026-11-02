"""S207: summarise context/capture/s207/NAME.csv: the bike's place, its velocity and the wreck reason every
EVERY rows, and the rows round the first wreck.

usage: python3 -B research/s207/look.py NAME [EVERY]"""
import csv
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
rows = list(csv.DictReader(open(ROOT / 'context' / 'capture' / 's207' / (sys.argv[1] + '.csv'))))
every = int(sys.argv[2]) if len(sys.argv) > 2 else 50
print(len(rows), 'rows; ctl_src', sorted({r['rider_ctl_src'] for r in rows}), 'rider_type', sorted({r['rider_type'] for r in rows}))


def f(r, k):
    return float(r[k])


first = next((i for i, r in enumerate(rows) if int(float(r['wreck_reason']))), None)
for i, r in enumerate(rows):
    if i % every == 0 or (first is not None and first - 4 <= i <= first + 1) or i == len(rows) - 1:
        print('%4d %6s pos %8.2f %7.2f %8.2f vel %7.2f %7.2f %7.2f reason %s%s' % (
            i, r['frame_counter'], *(f(r, 'bike_xform.m3%d' % k) for k in range(3)),
            *(f(r, 'ch_vel.' + c) for c in 'xyz'), r['wreck_reason'], '  <- first wreck' if i == first else ''))
print('first wreck row', first)
