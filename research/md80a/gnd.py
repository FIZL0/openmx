"""MD80a: the biplane against the ground under its first tire, every EVERY-th row in LO..HI.

  python3 -I research/md80a/gnd.py CSV EVERY LO HI

Per row: wreck word, x/z, body y, tire 0's gear point y (+0x104), its ray's ground hit y (+0xc4), the gap
between them and the body's vertical speed.
"""
import csv, struct, sys
csv.field_size_limit(1 << 26)
every = int(sys.argv[2])
lo, hi = int(sys.argv[3]), int(sys.argv[4])


def w(h, o):
    return struct.unpack_from('<f', bytes.fromhex(h[2 * o:2 * o + 8]))[0]


for i, r in enumerate(csv.DictReader(open(sys.argv[1]))):
    if i < lo or i > hi or i % every:
        continue
    t = r['tire0.raw']
    g, y = w(t, 0xc4), w(t, 0x104)
    print(i, r['wreck'], f"x {float(r['pos.x']):7.1f} z {float(r['pos.z']):7.1f} body {float(r['pos.y']):6.1f} gear {y:6.1f} ground {g:6.1f} agl {y - g:6.1f} vy {float(r['vel.y']):6.2f}")
