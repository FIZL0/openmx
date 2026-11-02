"""S240: what a state's props node holds (research/s197b/lists.py's), and each of the human rider's bone
collisions: touched, swept, the contact point, normal and other (+0x70).

usage: python3 -B research/s240/lists.py NAME...   (context/states/s240-NAME.eeMemory.bin, made by
research/s240/slots.py ram NAME)"""
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
from ps2kit.config import cfg            # noqa: E402

NODE = 0x00e17ef0
WRECK = 0x00dc98f0
OBJECTS = [0x11b6f90, 0x11bed70, 0x11c1860, 0x11c4350, 0x11c6e40]
COLLISIONS = 0x17ea790

for name in sys.argv[1:]:
    m = (ROOT / 'context' / 'states' / ('s240-' + name + '.eeMemory' + '.bin')).read_bytes()
    u = lambda a: struct.unpack_from('<I', m, a)[0]
    f4 = lambda a: struct.unpack_from('<4f', m, a)
    balls = {u(o + 0x120): k for k, o in enumerate(OBJECTS)}
    bones = {u(COLLISIONS + 4 * k): k for k in range(9)}
    def who(r):
        if r == WRECK:
            return 'wreck'
        if r in balls:
            return 'ball%d' % balls[r]
        if r in bones:
            return 'bone%d' % bones[r]
        return hex(r)
    print(name, 'frame', u(cfg().symbols['frame_counter']))
    n, at = u(NODE + 0x30), u(NODE + 0x38)
    filed = [u(at + 4 * k) for k in range(min(n, 64))] if 0 < at < len(m) else []
    print('  filed', n, [who(r) for r in filed])
    for r in filed:
        k, e = u(r + 0x100), u(r + 0x108)
        if not 0 < e < len(m) or k > 64:
            print('  %-9s no list' % who(r))
            continue
        flags = [u(r + o) for o in (0x40, 0x44, 0x48, 0x4c)]
        ents = [(u(e + 0x80 * i), who(u(e + 0x80 * i + 0x74)) if u(e + 0x80 * i + 0x74) else '-', ['%.3f' % v for v in f4(e + 0x80 * i + 0x40)[:3]]) for i in range(k)]
        print('  %-9s flags %s list %d %s' % (who(r), flags, k, ents))
    for k in range(9):
        c = u(COLLISIONS + 4 * k)
        print('  bone%d %#x on %d touched %d swept %d point %s normal %s other %s' % (
            k, c, u(c + 4), u(c + 0x14), u(c + 0x20), ['%.3f' % v for v in f4(c + 0x40)[:3]],
            ['%.3f' % v for v in f4(c + 0x50)[:3]], who(u(c + 0x70)) if u(c + 0x70) else '-'))
