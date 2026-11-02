"""S242: a ball's ray probe and mesh collidable in a state: the probe's category list (+0xe8 mode,
+0xec list, +0xf0 count) and ignore words (+0xd0, +0xd4, +0xd8), the mesh collidable's category and flags.

usage: python3 -B research/s242/probe.py NAME BALL...   (context/states/s242-NAME.eeMemory.bin)"""
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
OBJECTS = [0x11b6f90, 0x11bed70, 0x11c1860, 0x11c4350, 0x11c6e40]
m = (ROOT / 'context' / 'states' / ('s242-' + sys.argv[1] + '.eeMemory' + '.bin')).read_bytes()
u = lambda a: struct.unpack_from('<I', m, a)[0]
for b in (int(x) for x in sys.argv[2:]):
    o = OBJECTS[b]
    p = u(o + 0x124)
    n, lst = u(p + 0xf0), u(p + 0xec)
    print('ball', b, 'probe %#x cat %#x mask %#x +116 %#x mode %d list %s ignore %#x %#x %#x' % (
        p, u(p + 0x84), u(p + 0xcc), m[p + 0x116], u(p + 0xe8),
        ['%#x' % u(lst + 4 * k) for k in range(min(n, 8))] if 0 < lst < len(m) else [], u(p + 0xd0), u(p + 0xd4), u(p + 0xd8)))
    for w in (0xd0, 0xd4, 0xd8):
        a = u(p + w)
        if 0 < a < len(m):
            print('   +%#x -> %s' % (w, ['%#x' % u(a + 4 * k) for k in range(6)]))
