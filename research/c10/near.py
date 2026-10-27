"""How close the two riders and the path cars come in a 2p capture, and the record counts (C10).

    python3 -B research/c10/near.py [csv-name]
Research, not product.
"""
import csv, math, os, sys
from collections import Counter
name = sys.argv[1] if len(sys.argv) > 1 else '2p'
cnt = Counter()
best = []
carbest = {}
for r in csv.DictReader(open(os.path.join('context', 'capture', '2p', name + '.csv'))):
    if None in r.values():
        break
    f = int(r['frame_counter'])
    P = {p: [float(r['p%d_bike_xform.m3%d' % (p, k)]) for k in range(3)] for p in (1, 2)}
    best.append((math.dist(P[1], P[2]), f))
    for p in (1, 2):
        cnt[(p, r['p%d_rec_count' % p])] += 1
        for c in range(6):
            C = [float(r['car%d_xform.m3%d' % (c, k)]) for k in range(3)]
            d = math.dist(P[p], C)
            if (p, c) not in carbest or d < carbest[(p, c)][0]:
                carbest[(p, c)] = (d, f, P[p][1] - C[1])
print(sorted(cnt.items()))
best.sort()
print('closest riders', [(round(d, 1), f) for d, f in best[:5]])
for k, v in sorted(carbest.items()):
    print('P%d car%d closest %.1f at %d (dy %.1f)' % (k[0], k[1], v[0], v[1], v[2]))
