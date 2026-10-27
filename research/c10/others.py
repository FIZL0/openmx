"""Every record a 2p capture's contact lists name, with its rows (C10). Research, not product.

    python3 -B research/c10/others.py [csv-name]
"""
import csv, os, sys
from collections import defaultdict
name = sys.argv[1] if len(sys.argv) > 1 else '2p'
seen = defaultdict(list)
cars = defaultdict(list)
for r in csv.DictReader(open(os.path.join('context', 'capture', '2p', name + '.csv'))):
    if None in r.values():
        break
    for p in (1, 2):
        for k in range(11):
            o = r['p%d_c%d.other' % (p, k)]
            if o != '0':
                seen[(p, int(o))].append(int(r['frame_counter']))
        for w in ('wf', 'wr'):
            for f in ('car', 'owner'):
                v = r['p%d_%s_%s' % (p, w, f)]
                if v != '0':
                    cars[(p, w, f, int(v))].append(int(r['frame_counter']))
for (p, o), fs in sorted(seen.items()):
    print('P%d names %08x in %d rows, %d..%d' % (p, o, len(fs), fs[0], fs[-1]))
for (p, w, f, v), fs in sorted(cars.items()):
    print('P%d %s %s %08x in %d rows, %d..%d' % (p, w, f, v, len(fs), fs[0], fs[-1]))
