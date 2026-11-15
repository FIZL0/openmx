"""MD164 (research/md160/decay.py on md164 captures, with the car state): the body's angular rates row to row, and their ratio, over a row range of a capture.

  python3 -I research/md164/decay.py NAME FIRST LAST

NAME is context/capture/md164/NAME.csv. In the air with no torque, the ratio is the angular damping's
per-tick factor: SLUS-21104's integrate gives d per substep, Untamed's powf(d, 0.333/h).
"""
import csv
import os
import sys

csv.field_size_limit(1 << 26)
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
name, first, last = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
rows = list(csv.DictReader(open(os.path.join(ROOT, 'context', 'capture', 'md164', name + '.csv'))))
for i in range(max(first, 1), last + 1):
    r, q = rows[i], rows[i - 1]
    t = ''.join(r[f'tire{k}.touch'] for k in range(4))
    rate = [float(r[f'rate.{c}']) for c in 'xyz']
    prev = [float(q[f'rate.{c}']) for c in 'xyz']
    ratio = ' '.join(f'{a / b:8.5f}' if b else '       -' for a, b in zip(rate, prev))
    print(f"{i:4d} s {r['carstate']:>2} t {t} rec {int(r['record'][0x34 * 2:0x34 * 2 + 2], 16)} d {float(r['angdamp.x']):.4g} "
          f"rate {' '.join(f'{x:9.5f}' for x in rate)} ratio {ratio}")
