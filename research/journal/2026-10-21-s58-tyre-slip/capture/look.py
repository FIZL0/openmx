"""S58: a short look at one slip capture, every Nth row.

    python3 -B look.py CASE [EVERY] [COLUMN...]

CASE names context/capture/s58-slip/CASE.csv; the default columns are the slip lanes."""
import csv
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, '..', '..', '..'))
root = os.path.dirname(root)
path = os.path.join(root, 'context', 'capture', 's58-slip', sys.argv[1] + '.csv')
every = int(sys.argv[2]) if len(sys.argv) > 2 else 10
rows = list(csv.DictReader(open(path)))
cols = sys.argv[3:] or ['ctl[2].held', 'ctl[3].held', 'ch_speed', 'f_spin', 'r_spin', 'f_spinning', 'f_locked',
                        'f_sliding', 'r_spinning', 'r_locked', 'r_sliding', 'f_surface', 'r_surface']
cols = [c for c in cols if c in rows[0]]
print(len(rows), 'rows;', 'frame_counter', ' '.join(cols))
for i, r in enumerate(rows):
    if i % every == 0:
        print(r['frame_counter'], ' '.join(r[c][:10] for c in cols))
