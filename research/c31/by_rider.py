"""C31: turn a list-ordered capture (spec.py without BASE) into one column group per rider.

The original re-sorts its card list by place, so sx01-ai4.csv's `cardK_0` is whoever was K-th in the
list on that tick. This rewrites every row with `riderN_0.*` columns keyed by each card's id (its +0
word), plus `order` (the ids in list order), and leaves the other columns as they are.

usage: python3 -I research/c31/by_rider.py IN.csv OUT.csv
"""
import csv, sys

csv.field_size_limit(1 << 24)
src, dst = sys.argv[1:]
rows = csv.DictReader(open(src))
cols = rows.fieldnames
groups = sorted({c.split('.', 1)[0] for c in cols if c.startswith('card')})
fields = [c.split('.', 1)[1] for c in cols if c.startswith(groups[0] + '.')]
plain = [c for c in cols if not c.startswith('card')]
n = len(groups)
out = csv.writer(open(dst, 'w', newline=''))
out.writerow(plain + ['order'] + ['rider%d_0.%s' % (i, f) for i in range(n) for f in fields])
for r in rows:
    by = {}
    order = []
    for g in groups:
        i = r[g + '.id']
        order.append(i)
        if i != '':
            by[int(i)] = [r[g + '.' + f] for f in fields]
    out.writerow([r[c] for c in plain] + [' '.join(order)]
                 + [x for i in range(n) for x in by.get(i, [''] * len(fields))])
