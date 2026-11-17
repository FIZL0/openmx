"""F21: free11's bowling pins (its one ten-body collection) in a state: each pin's object, body, collision
object, probe and mesh collidable, whether its body is enabled, where it stands; the props node's filed list.

usage: python3 -B research/f21/pins.py NAME...   (context/states/f21-NAME.eeMemory.bin, research/f21/slots.py ram NAME)"""
import math
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
COLLECTION_VTABLE = 0x0045da90
FRAME = 0x00479d04

for name in sys.argv[1:]:
    m = (ROOT / 'context' / 'states' / ('f21-' + name + '.eeMemory' + '.bin')).read_bytes()
    u = lambda a: struct.unpack_from('<I', m, a)[0]
    f3 = lambda a: struct.unpack_from('<3f', m, a)
    colls = [a - 8 for a in range(8, 0x2000000, 4) if u(a) == COLLECTION_VTABLE]
    pins = [c for c in colls if u(c + 0x320) == 10]
    assert len(pins) == 1, 'one ten-body collection'
    c = pins[0]
    print(name, 'frame', u(FRAME), 'collections', len(colls), 'pins: collection', colls.index(c), hex(c), 'flags %#x' % u(c + 0x20))
    arr = u(c + 0x328)
    objs = [u(arr + 4 * k) for k in range(10)]
    ps = []
    for k, o in enumerate(objs):
        b = u(o + 0x110)
        p = f3(b + 0x60)
        ps.append(p)
        print('  pin %d obj %#x body %#x col %#x probe %#x mesh %#x enabled %d pos %s vel %s' % (
            k, o, b, u(o + 0x120), u(o + 0x124), u(o + 0x128), u(b), ['%.3f' % v for v in p],
            ['%.3f' % v for v in f3(b + 0x70)]))
    for k in range(10):
        near = sorted((math.dist(ps[k], ps[j]), j) for j in range(10) if j != k)[:2]
        print('  pin %d nearest %s' % (k, ['%d at %.2f' % (j, d) for d, j in near]))
