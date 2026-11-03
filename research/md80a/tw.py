"""MD80a: the f32 words of one raw block that differ across the picked rows.

  python3 -I research/md80a/tw.py CSV BLOCK ROWS

BLOCK is a raw column (obj0, obj1, body, engine, rotor, aero, tire0.raw ..), ROWS comma-separated row indices.
"""
import csv, struct, sys
csv.field_size_limit(1 << 26)
rows = list(csv.DictReader(open(sys.argv[1])))
k = sys.argv[2]
pick = [int(x) for x in sys.argv[3].split(',')]


def w(r, o):
    return struct.unpack_from('<f', bytes.fromhex(r[k][2 * o:2 * o + 8]))[0]


n = len(bytes.fromhex(rows[0][k])) // 4
for o in range(n):
    vals = [w(rows[i], o * 4) for i in pick]
    if len(set(vals)) > 1 and all(abs(v) < 1e7 for v in vals):
        print(hex(o * 4), ' '.join(f'{v:.4g}' for v in vals))
