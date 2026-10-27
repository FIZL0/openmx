"""Everything the 2p riders touch (C10): each distinct wheel owner, wheel vehicle and record `other`, per rider.

    python3 -B research/c10/owners.py CAPTURE       # capture/2p/<CAPTURE>.csv

For each value: the rows it appears on and its first and last frame. Props (crates, signs, the beach ball)
would show here as owners or record others besides scenery's. Research, not product.
"""
import csv
import os
import sys

seen = {}
with open(os.path.join('context', 'capture', '2p', sys.argv[1] + '.csv')) as f:
    rd = csv.DictReader(f)
    cols = [c for c in rd.fieldnames if c.endswith(('_owner', '_wf_car', '_wr_car')) or c.endswith('.other')]
    for r in rd:
        try:
            fc = int(r['frame_counter'])
        except ValueError:
            continue
        for c in cols:
            key = (c[:2], c.split('_', 1)[1] if not c.endswith('.other') else 'rec other', r[c])
            n, a, b = seen.get(key, (0, fc, fc))
            seen[key] = (n + 1, min(a, fc), max(b, fc))
for (p, col, v), (n, a, b) in sorted(seen.items()):
    print('%s %-12s %-10s %6d rows  %d..%d' % (p, col, v, n, a, b))
