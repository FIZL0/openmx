"""C44: compare recordings of the same ride, row by row on frame_counter, and say which columns differ.

usage: python3 -B research/c44/agree.py A.csv B.csv [C.csv ...]

For each frame every file holds, the files' rows are compared on every column but the recorder's own
`frame`; prints the frames where any differ and which column groups.
"""
import csv
import sys

runs = []
for path in sys.argv[1:]:
    rows = {int(r['frame_counter']): r for r in csv.DictReader(open(path))}
    runs.append(rows)
common = sorted(set.intersection(*(set(r) for r in runs)))
bad = 0
for f in common:
    rs = [r[f] for r in runs]
    cols = [c for c in rs[0] if c not in ('frame', 'time') and any(r[c] != rs[0][c] for r in rs[1:])]
    if cols:
        bad += 1
        groups = sorted({c.split('.')[0].split('_c')[0] if c[:2] in ('a0', 'a1', 'a3') else c.split('.')[0] for c in cols})
        print(f, len(cols), 'columns differ:', ' '.join(groups[:12]))
print('%d frames in common (%d..%d), %d differ' % (len(common), common[0], common[-1], bad))
