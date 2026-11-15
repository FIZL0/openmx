"""MD164 (research/md164/rows.py on md164): which rows two md164 captures differ on, for some decoded columns, and whether a row of one
equals the other's next or previous row (a late or early read) rather than a different ride.

  python3 -I research/md164/rows.py A B [COL,COL..]
"""
import csv
import os
import sys

csv.field_size_limit(1 << 26)
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
load = lambda n: list(csv.DictReader(open(os.path.join(ROOT, 'context', 'capture', 'md164', n + '.csv'))))
a, b = load(sys.argv[1]), load(sys.argv[2])
cols = (sys.argv[3] if len(sys.argv) > 3 else 'pos.x,pos.y,pos.z,vel.x,vel.y,vel.z,rate.x,rate.y,rate.z,quat.w').split(',')
key = lambda r: tuple(r[c] for c in cols)
for i, (x, y) in enumerate(zip(a, b)):
    if key(x) != key(y):
        how = []
        if i + 1 < len(b) and key(x) == key(b[i + 1]):
            how.append('A = B next')
        if i + 1 < len(a) and key(y) == key(a[i + 1]):
            how.append('B = A next')
        if i and key(x) == key(b[i - 1]):
            how.append('A = B prev')
        if i and key(y) == key(a[i - 1]):
            how.append('B = A prev')
        print(i, x['vsync'], y['vsync'], ', '.join(how) or 'neither')
