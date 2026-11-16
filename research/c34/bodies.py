"""C34: rigid bodies in a RAM image: quad-aligned blocks laid out as the biplane's body (+0x60 position
w 1, +0x50 last position w 1, +0x170 a unit turn, +0x10 an inverse mass > 0) inside the map.

usage: bodies.py IMAGE"""
import struct, sys
d = open(sys.argv[1], 'rb').read()
for a in range(0x100000, len(d) - 0x200, 16):
    p = struct.unpack_from('<4f', d, a + 0x60)
    if p[3] != 1.0 or not (0 < p[0] < 5000 and 0 < p[2] < 5000 and 0 < p[1] < 2000):
        continue
    l = struct.unpack_from('<4f', d, a + 0x50)
    q = struct.unpack_from('<4f', d, a + 0x170)
    m = struct.unpack_from('<f', d, a + 0x10)[0]
    if l[3] != 1.0 or not (0 < m < 10) or abs(sum(x * x for x in q) - 1) > 1e-3:
        continue
    print(hex(a), 'inv_mass %.4g' % m, 'pos', tuple(round(x, 2) for x in p[:3]), 'enabled', struct.unpack_from('<I', d, a)[0])
