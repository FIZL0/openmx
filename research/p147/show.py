"""P147: print a roof.toml capture around the flip and its landing.

usage: python3 -B show.py NAME [FROM TO]   (context/capture/p147/NAME.csv: the guard refuses a
capture path on the command line)
"""
import csv
import sys

rows = list(csv.DictReader(open(f"context/capture/p147/{sys.argv[1]}.csv")))
lo, hi = (int(sys.argv[2]), int(sys.argv[3])) if len(sys.argv) > 3 else (2505, 2640)
cols = ['airborne', 'flipping', 'flip_applied', 'on_ground', 'wreck_reason', 'up_y', 'rec_ground',
        'rec_touched', 'rec_count', 'rec_first_kind', 'rec_first_other', 'rec_first_body.y']
print('fc', *cols)
for r in rows:
    fc = int(r['frame_counter'])
    if lo <= fc <= hi:
        print(fc, *[r.get(c, '?')[:7] for c in cols])
        if r['wreck_reason'] != '0' and fc > lo + 3:
            break
