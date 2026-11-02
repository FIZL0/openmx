"""S240: a state's rider body actors: each bone collision's body position and velocity, and the balls'.

usage: python3 -B research/s240/bones.py STATE...   (context/states/STATE.eeMemory.bin)"""
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
BALLS = [0x11b6030, 0x11b6a90, 0x11bf460, 0x11c1f50, 0x11c4a40]


def actors(m):
    u = lambda a: struct.unpack_from('<I', m, a)[0]
    ok = lambda a: 0x100000 <= a < 0x2000000 and a % 4 == 0
    for a in range(0x100000, 0x1fff000, 4):
        if u(a + 0x2e0) != 18:
            continue
        s = u(a + 0x628)
        if not ok(s) or struct.unpack_from('<f', m, s + 0x24)[0] != 10.0:
            continue
        yield a


for name in sys.argv[1:]:
    m = (ROOT / 'context' / 'states' / (name + '.eeMemory.bin')).read_bytes()
    u = lambda a: struct.unpack_from('<I', m, a)[0]
    f3 = lambda a: struct.unpack_from('<3f', m, a)
    fmt = lambda t: ' '.join('%8.2f' % x for x in t)
    print(name)
    for a in actors(m):
        cs = u(a + 0x624)
        print('  actor %#x collisions %#x search %#x' % (a, cs, u(a + 0x628)))
        for k in range(9):
            c = u(cs + 4 * k)
            if not 0x100000 <= c < 0x2000000:
                break
            b = u(c + 0x7c)
            print('   coll %d %#x on %d body %#x en %d pos %s vel %s r %.3f' % (
                k, c, u(c + 4), b, u(b), fmt(f3(b + 0x60)), fmt(f3(b + 0x70)), struct.unpack_from('<f', m, b + 0x14)[0]))
    for k, b in enumerate(BALLS):
        print('  ball %d en %d pos %s vel %s' % (k, u(b), fmt(f3(b + 0x60)), fmt(f3(b + 0x70))))
