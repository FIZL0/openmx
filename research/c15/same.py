"""C15: compare two captures of the same tape row by row, every column (raw blocks byte for byte).

  python3 -I research/c15/same.py A.csv B.csv
Prints the first differing row per column (vsync-aligned) and the count of differing rows.
"""
import csv
import sys

csv.field_size_limit(1 << 24)
a, b = ({r['vsync']: r for r in csv.DictReader(open(p))} for p in sys.argv[1:3])
both = sorted(set(a) & set(b), key=int)
print(len(a), len(b), 'rows,', len(both), 'shared vsyncs', both[0], '..', both[-1])
skip = {'frame'}
bad = {}
for v in both:
    for k in a[v]:
        if k not in skip and a[v][k] != b[v][k]:
            bad.setdefault(k, []).append(v)
for k, vs in bad.items():
    x, y = a[vs[0]][k], b[vs[0]][k]
    if len(x) > 40:
        i = next(i for i in range(len(x)) if x[i] != y[i])
        x, y = f'byte ~{i // 2:#x}: {x[i:i + 16]}', y[i:i + 16]
    print(f'{k}: {len(vs)} rows differ, first at vsync {vs[0]}: {x} vs {y}')
print('identical' if not bad else f'{len(bad)} columns differ')
