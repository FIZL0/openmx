"""S240: the human rider's bones' search (actor 0x01797980 +0x628): its centre, bound, built flag and its
found list (each entry's collidable, with research/s240/what.py's fields, and the word after it).

usage: python3 -B research/s240/search.py NAME...   (context/states/s240-NAME.eeMemory.bin)"""
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
SEARCH = 0x17e9dc0
for name in sys.argv[1:]:
    m = (ROOT / 'context' / 'states' / ('s240-' + name + '.eeMemory' + '.bin')).read_bytes()
    u = lambda a: struct.unpack_from('<I', m, a)[0]
    f = lambda a: struct.unpack_from('<f', m, a)[0]
    found, n = u(SEARCH), u(SEARCH + 4)
    print(name, 'count', n, 'centre', ['%.3f' % v for v in struct.unpack_from('<3f', m, SEARCH + 0x10)],
          'bound %.3f built %d' % (f(SEARCH + 0x20), u(SEARCH + 0x30)), 'words', ['%#x' % u(SEARCH + 4 * k) for k in range(16)])
    for k in range(min(n, 40)):
        e = found + 8 * k
        c = u(e)
        print('  %2d %#x d %.3f vt %#x cat %#x +116 %#x +80 %#x centre %s r %.3f' % (
            k, c, f(e + 4), u(c + 0x38), u(c + 0x84), m[c + 0x116], u(c + 0x80),
            ['%.1f' % v for v in struct.unpack_from('<3f', m, c + 0x40)], f(c + 0x60)))
