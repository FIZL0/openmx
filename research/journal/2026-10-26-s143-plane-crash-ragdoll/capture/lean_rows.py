#!/usr/bin/env python3
"""S143: a few joints of the pilot, plane-local, beside the pad, from a to_csv.py csv.

    python3 -B lean_rows.py context/capture/s143/waggle.csv [--every 5] [--joints 0,5,9]

Research, not product.
"""

import argparse
import csv


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('csv')
    ap.add_argument('--every', type=int, default=5)
    ap.add_argument('--joints', default='0,5,9')
    ap.add_argument('--from', dest='lo', type=int, default=0)
    ap.add_argument('--to', dest='hi', type=int, default=1 << 31)
    a = ap.parse_args()
    js = [int(j) for j in a.joints.split(',')]
    for i, x in enumerate(csv.DictReader(open(a.csv))):
        f = int(x['frame'])
        if i % a.every or not (a.lo <= f <= a.hi):
            continue
        bones = ' '.join(f"{x[f'bone{j}.x']},{x[f'bone{j}.y']},{x[f'bone{j}.z']}" for j in js)
        print(f, 'w', x['wreck'], 'elev', '%.2f' % float(x['elevator']), 'ail', '%.2f' % float(x['aileron']), bones)


if __name__ == '__main__':
    main()
