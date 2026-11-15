"""MD147: rows of a capture whose body block equals the row before (a late read: the recorder misses rows
under load), and vsync steps that are not 2.

  python3 -I research/md147/dups.py NAME [NAME ...]
"""
import csv
import os
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')
csv.field_size_limit(1 << 24)
for name in sys.argv[1:]:
    rows = list(csv.DictReader(open(os.path.join(ROOT, 'context/capture/md147', name + '.csv'))))
    same = [i for i in range(1, len(rows)) if rows[i]['body'] == rows[i - 1]['body']]
    steps = [i for i in range(1, len(rows)) if int(rows[i]['vsync']) - int(rows[i - 1]['vsync']) != 2]
    print(f'{name}: {len(rows)} rows, {len(same)} repeat the row before {same[:12]}, vsync steps not 2: {steps[:12]}')
