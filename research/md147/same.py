"""MD147 (research/md164/same.py at md147's captures): does capture B (from a later state) replay capture A from row OFFSET on? Prints the first rows that
differ on the decoded columns (or the columns given) and the count of equal rows.

  python3 -I research/md147/same.py A B OFFSET [COL,COL..]
"""
import csv
import os
import sys

csv.field_size_limit(1 << 26)
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
load = lambda n: list(csv.DictReader(open(os.path.join(ROOT, 'context', 'capture', 'md147', n + '.csv'))))
a, b, off = load(sys.argv[1]), load(sys.argv[2]), int(sys.argv[3])
cols = (sys.argv[4] if len(sys.argv) > 4 else 'pos.x,pos.y,pos.z,quat.x,quat.y,quat.z,quat.w,rates.x,rates.y,rates.z,speed,euler.x,euler.y,euler.z').split(',')
same, diff = 0, []
for i, y in enumerate(b):
    if off + i >= len(a):
        break
    x = a[off + i]
    if all(x[c] == y[c] for c in cols):
        same += 1
    else:
        diff.append(i)
print(f'{same} equal, {len(diff)} differ; first differing rows of B: {diff[:10]}')
