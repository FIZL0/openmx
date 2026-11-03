"""MD80a: which f32 words of a capture's raw blocks track a given column or expression.

  python3 -I research/md80a/find.py CSV EXPR [ROWS] [--tol T]

EXPR is a Python expression over a row's decoded words, w(block, off) (f32 at off in that raw block) or
c(name) (a csv column as a float), e.g. "w('engine', 0x50) / 250". Every word of obj0/obj1 (as one block,
"obj"), body, engine/rotor, aero and tire0..2.raw is compared with it over ROWS (comma-separated, default
every 37th row); those within T (relative, default 1e-4) on every row are printed.
"""
import argparse
import csv
import struct

csv.field_size_limit(1 << 26)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('csv')
    ap.add_argument('expr')
    ap.add_argument('rows', nargs='?', default='')
    ap.add_argument('--tol', type=float, default=1e-4)
    a = ap.parse_args()
    rows = list(csv.DictReader(open(a.csv)))
    picks = [int(x) for x in a.rows.split(',')] if a.rows else range(0, len(rows), 37)
    blocks = ['obj', 'body', 'engine', 'rotor', 'aero', 'tire0.raw', 'tire1.raw', 'tire2.raw']

    def raw(r, k):
        return bytes.fromhex(r['obj0'] + r['obj1']) if k == 'obj' else bytes.fromhex(r[k])

    hits = None
    for i in picks:
        r = rows[i]
        cache = {k: raw(r, k) for k in blocks if k == 'obj' or k in r}
        want = eval(a.expr, {'w': lambda k, o: struct.unpack_from('<f', cache[k], o)[0],
                             'c': lambda n: float(r[n])})
        found = set()
        for k, b in cache.items():
            for o, (v,) in enumerate(struct.iter_unpack('<f', b[:len(b) // 4 * 4])):
                if abs(v - want) <= a.tol * max(1.0, abs(want)):
                    found.add((k, o * 4))
        hits = found if hits is None else hits & found
    for k, o in sorted(hits):
        print(f'{k} +{o:#x}')


main()
