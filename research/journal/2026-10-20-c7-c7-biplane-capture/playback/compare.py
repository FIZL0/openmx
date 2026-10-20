"""Two captures of the same playback, cell by cell: which columns differ, on how many rows, first where.

usage: python3 compare.py A.csv B.csv
"""
import csv, sys

a = list(csv.DictReader(open(sys.argv[1])))
b = list(csv.DictReader(open(sys.argv[2])))
print(f'{len(a)} vs {len(b)} rows; ticks {a[0]["frame_counter"]}..{a[-1]["frame_counter"]} vs {b[0]["frame_counter"]}..{b[-1]["frame_counter"]}')
diff = {}
for k, (x, y) in enumerate(zip(a, b)):
    for c in x:
        if x[c] != y.get(c):
            diff.setdefault(c, []).append(k)
same = [c for c in a[0] if c not in diff]
print(f'{len(same)} of {len(a[0])} columns identical on every row')
for c, ks in diff.items():
    print(f'  {c}: {len(ks)} rows differ, first row {ks[0]} ({a[ks[0]][c]} vs {b[ks[0]][c]})')
rows_diff = sorted({k for c, ks in diff.items() if c != 'rng_state' for k in ks})
print('rows with a non-rng difference', rows_diff)
for k in rows_diff:
    for s, r in (('A', a), ('B', b)):
        same_prev = r[k]['body_pos.z'] == r[k - 1]['body_pos.z']
        same_next = r[k]['body_pos.z'] == r[k + 1]['body_pos.z']
        print(f'  {s} row {k}: body_pos.z {r[k - 1]["body_pos.z"][:9]} | {r[k]["body_pos.z"][:9]} | {r[k + 1]["body_pos.z"][:9]}'
              f'  (equals previous {same_prev}, next {same_next})')
if 'rng_state' in diff:
    ks = diff['rng_state']
    print('rng_state differs on rows', ks[:8], '...', ks[-3:])
