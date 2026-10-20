"""Integrity of a biplane capture: tick range and gaps, empty cells, the identity columns, and whether
each row's pad half is the tape's (C6's test: row k's buttons equal p2m2 field 2k-1).

usage: python3 check.py CAPTURE.csv [RECORDING.p2m2]
"""
import csv, sys, collections

rows = list(csv.DictReader(open(sys.argv[1])))
cols = list(rows[0].keys())
fc = [int(r['frame_counter']) for r in rows]
print(f'{len(rows)} rows x {len(cols)} columns, ticks {fc[0]}..{fc[-1]}')
steps = collections.Counter(b - a for a, b in zip(fc, fc[1:]))
print('tick steps', dict(steps))
empty = collections.Counter(c for r in rows for c in cols if r[c] == '')
print('empty cells', dict(empty) or 0)
for c in ('plane_type', 'plane_ctl_src', 'plane_flown', 'plane_class', 'plane_player', 'rider_type',
          'rider_player', 'bike_type', 'bike_ctl_src', 'bike_player', 'dt', 'crash'):
    print(f'{c:14}', dict(collections.Counter(r[c] for r in rows)))
# bike-style repeats: a moving row whose body position equals the previous row's
rep = sum(1 for a, b in zip(rows, rows[1:])
          if b['body_pos.x'] == a['body_pos.x'] and b['body_pos.z'] == a['body_pos.z'] and float(b['speed']) > 1)
print('moving rows repeating the previous position', rep)

if len(sys.argv) > 2:
    # The pad half the game read is the one with the smaller stamp (C5); bytes 2..7 vs the tape.
    data = open(sys.argv[2], 'rb').read()
    def pad_of(r):
        a, b = int(r['stamp_a']), int(r['stamp_b'])
        h = r['pad_a'] if a < b else r['pad_b']
        return bytes.fromhex(h.replace(' ', ''))[2:8]
    print('row 0 pad', pad_of(rows[0]).hex(), 'row 1 pad', pad_of(rows[1]).hex())
    H, F = 570, 36
    nf = (len(data) - H) // F
    field = lambda k: data[H + k * F: H + k * F + 6]   # pad 0: both button bytes, then the four axes
    best = []
    for off in range(-6, 7):
        ok = sum(1 for k, r in enumerate(rows) if 1 <= 2 * k + off < nf and field(2 * k + off) == pad_of(r))
        n = sum(1 for k in range(len(rows)) if 1 <= 2 * k + off < nf)
        best.append((ok, n, off))
    for ok, n, off in sorted(best, reverse=True)[:4]:
        print(f'row k = field 2k{off:+d}: {ok} of {n} rows match')
    print('fields in the file', nf - 1)
