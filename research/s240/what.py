"""S240: what a collidable is in a state: its vtable (+0x38), category (+0x84), mask (+0xcc), flags (+0x116,
+0x31), owners (+0x7c, +0x80), centre and reach, and the words in RAM that point at it.

usage: python3 -B research/s240/what.py NAME ADDR...   (context/states/s240-NAME.eeMemory.bin)"""
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
m = (ROOT / 'context' / 'states' / ('s240-' + sys.argv[1] + '.eeMemory' + '.bin')).read_bytes()
u = lambda a: struct.unpack_from('<I', m, a)[0]
for a in (int(x, 16) for x in sys.argv[2:]):
    print('%#x vt %#x class %d cat %#x mask %#x +116 %#x +31 %#x +7c %#x +80 %#x centre %s reach %.3f' % (
        a, u(a + 0x38), m[a + 2], u(a + 0x84), u(a + 0xcc), m[a + 0x116], m[a + 0x31], u(a + 0x7c), u(a + 0x80),
        ['%.3f' % v for v in struct.unpack_from('<3f', m, a + 0x40)], struct.unpack_from('<f', m, a + 0x60)[0]))
    print('  pointed at from', [hex(i) for i in range(0x100000, len(m) - 4, 4) if u(i) == a][:20])
    print('  listeners +0xf4 %d at %s, callback +0x10c %#x, +0x110 %#x' % (
        u(a + 0xf4), ['%#x' % u(u(a + 0xf4 + 8) + 4 * k) for k in range(min(u(a + 0xf4), 4))] if 0 < u(a + 0xf4 + 8) < len(m) else '-',
        u(a + 0x10c), u(a + 0x110)))
    for k in range(min(u(a + 0xf4), 4)):
        l = u(u(a + 0xf4 + 8) + 4 * k)
        if 0 < l < len(m):
            print('    listener %#x vtable %#x slots +8 %#x +0xc %#x' % (l, u(l), u(u(l) + 8), u(u(l) + 0xc)))
