"""C34: a car capture at a glance: frame_counter gaps, and the body every EVERY rows.

usage: look.py CSV [EVERY]"""
import csv, sys
csv.field_size_limit(1 << 26)
rows = list(csv.DictReader(open(sys.argv[1])))
every = int(sys.argv[2]) if len(sys.argv) > 2 else 10
fc = [int(r['frame_counter']) for r in rows]
gaps = [(a, b) for a, b in zip(fc, fc[1:]) if b != a + 1]
print(len(rows), 'rows', fc[0], '..', fc[-1], 'gaps', len(gaps), gaps[:20])
jumps = []
for i, r in enumerate(rows):
    v = lambda n: [round(float(r[f'{n}.{l}']), 3) for l in 'xyzw']
    if i and abs(float(r['body_pos.y']) - float(rows[i - 1]['body_pos.y'])) > 0.5:
        jumps.append(r['frame_counter'])
    if i % every and i not in (0, len(rows) - 1):
        continue
    print(r['frame_counter'], 'pos', v('body_pos'), 'vel', v('body_vel'), 'quat', v('body_quat'))
print('rows where y moved > 0.5 ft:', jumps)
