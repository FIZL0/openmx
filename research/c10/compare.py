"""Two playbacks of the 2p recording side by side (C10): does another PCSX2 build ride the same ride?

    python3 -B research/c10/compare.py A B      # capture/2p/<A>.csv and <B>.csv (any spec with check.toml's names)

Per rider: the largest chassis-centre distance between the runs at the same frame_counter (and where), each
run's wreck onsets, and each run's closest rider-to-rider and rider-to-car approach. Research, not product.
"""
import csv
import math
import os
import sys

SCALARS = ['frame_counter', 'p1_wreck_reason', 'p2_wreck_reason', 'p1_wf_car', 'p2_wf_car']
VECS = ['p1_ch_centre', 'p2_ch_centre'] + ['car%d_pos' % i for i in range(6)]


def vec(v):
    return v


def load(name):
    rows = {}
    with open(os.path.join('context', 'capture', '2p', name + '.csv')) as f:
        for r in csv.DictReader(f):
            try:
                fc = int(r['frame_counter'])
            except ValueError:
                continue
            row = {k: r.get(k, '') for k in SCALARS}
            for k in VECS:
                # 2p.toml calls a car's position (+0x310) car%d_prev
                src = k if k + '.x' in r else k.replace('_pos', '_prev')
                row[k] = tuple(float(r[src + a]) for a in '.x .y .z'.split()) if r.get(src + '.x') else None
            rows[fc] = row
    return rows


def onsets(rows, p):
    out, last = [], '0'
    for fc in sorted(rows):
        w = rows[fc]['%s_wreck_reason' % p]
        if w not in ('0', '') and last in ('0', ''):
            out.append((fc, int(w)))
        last = w
    return out


def approaches(rows):
    rr, rc, cars = (1e9, 0), (1e9, 0, ''), set()
    for fc, r in rows.items():
        a, b = vec(r['p1_ch_centre']), vec(r['p2_ch_centre'])
        if a and b:
            rr = min(rr, (math.dist(a, b), fc))
        for p, v in (('p1', a), ('p2', b)):
            if r['%s_wf_car' % p] not in ('0', ''):
                cars.add((p, fc))
            for i in range(6):
                c = vec(r['car%d_pos' % i])
                if v and c:
                    rc = min(rc, (math.dist(v, c), fc, '%s car%d' % (p, i)))
    return rr, rc, sorted(cars)


runs = [(n, load(n)) for n in sys.argv[1:3]]
common = sorted(set(runs[0][1]) & set(runs[1][1]))
print('common frames %d (%d..%d)' % (len(common), common[0], common[-1]))
for p in ('p1', 'p2'):
    worst, first = (0, 0), None
    for fc in common:
        a, b = vec(runs[0][1][fc]['%s_ch_centre' % p]), vec(runs[1][1][fc]['%s_ch_centre' % p])
        if a and b:
            d = math.dist(a, b)
            worst = max(worst, (d, fc))
            if first is None and d > 5:
                first = fc
    print('%s: largest centre distance %.2f ft at %d; first > 5 ft at %s' % (p, worst[0], worst[1], first))
for n, rows in runs:
    rr, rc, cars = approaches(rows)
    print('%s: P1 wrecks %s' % (n, onsets(rows, 'p1')))
    print('%s: P2 wrecks %s' % (n, onsets(rows, 'p2')))
    print('%s: riders closest %.1f ft at %d; rider-car closest %.1f ft at %d (%s); wheel on a car: %d rows %s'
          % (n, rr[0], rr[1], rc[0], rc[1], rc[2], len(cars), cars[:4]))
