"""C34: a ball capture at a glance: gaps, the car's body, and each ball's waking and travel.

usage: balls.py CSV [EVERY]"""
import csv, sys
csv.field_size_limit(1 << 26)
rows = list(csv.DictReader(open(sys.argv[1])))
every = int(sys.argv[2]) if len(sys.argv) > 2 else 10
fc = [int(r['frame_counter']) for r in rows]
print(len(rows), 'rows', fc[0], '..', fc[-1], 'gaps', [(a, b) for a, b in zip(fc, fc[1:]) if b != a + 1][:20])
v = lambda r, n: [round(float(r[f'{n}.{l}']), 2) for l in 'xyz']
for k in range(6):
    woke = next((r['frame_counter'] for r in rows if r[f'ball{k}_enabled'] != '0'), None)
    print(f'ball{k}: woke at {woke}, first {v(rows[0], f"ball{k}_pos")}, last {v(rows[-1], f"ball{k}_pos")}')
for i, r in enumerate(rows):
    if i % every == 0:
        print(r['frame_counter'], 'car', v(r, 'body_pos'), v(r, 'body_vel'), 'awake', [k for k in range(6) if r[f'ball{k}_enabled'] != '0'])
