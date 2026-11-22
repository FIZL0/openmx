"""S158: the rider-vs-rider rows of a 2p capture: every row where an entry of either rider's collision
record names a record (`other` non-zero), with the riders' wreck reasons, speeds and distance.

    python3 -I research/s158/contacts.py CAPTURE.csv [PAD]

PAD rows of context either side of each run of contact rows. Research, not product.
"""
import csv
import math
import sys

rows = list(csv.DictReader(open(sys.argv[1])))
pad = int(sys.argv[2]) if len(sys.argv) > 2 else 2


def entries(row, p):
    n = int(row[f'p{p}_rec_count'] or 0)
    out = []
    for k in range(min(n, 11)):
        other = int(row[f'p{p}_c{k}.other'] or 0)
        out.append((int(row[f'p{p}_c{k}.kind'] or 0), other))
    return out


def centre(row, p):
    return [float(row[f'p{p}_ch_centre.{c}']) for c in 'xyz']


hit = [i for i, r in enumerate(rows) if any(o for p in (1, 2) for _, o in entries(r, p))]
show = sorted({j for i in hit for j in range(max(0, i - pad), min(len(rows), i + pad + 1))})
print(len(rows), 'rows; contact rows:', [rows[i]['frame_counter'] for i in hit])
last = None
for i in show:
    r = rows[i]
    if last is not None and i != last + 1:
        print('...')
    last = i
    line = [r['frame_counter'], '*' if i in hit else ' ']
    for p in (1, 2):
        line.append(f"p{p} w{r[f'p{p}_wreck_reason']} v{float(r[f'p{p}_ch_speed']):6.2f} "
                    f"{[(k, hex(o)) for k, o in entries(r, p)]}")
    line.append('d %.2f' % math.dist(centre(r, 1), centre(r, 2)))
    print('  '.join(line))
