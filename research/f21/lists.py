"""F21: what a state's props node holds on free11: the frame, its filed collision objects, and each
filed pin's contact list (kind, the object named, the point) and flags.

usage: python3 -B research/f21/lists.py NAME...   (context/states/f21-NAME.eeMemory.bin)"""
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
NODE = 0x007a3d40
FRAME = 0x00479d04
OBJECTS = [0xd08ae0, 0xd0b200, 0xd0fc70, 0xd12d30, 0xd15df0, 0xd18eb0, 0xd1bf90, 0xd1f050, 0xd22110, 0xd251d0]

for name in sys.argv[1:]:
    m = (ROOT / 'context' / 'states' / ('f21-' + name + '.eeMemory' + '.bin')).read_bytes()
    u = lambda a: struct.unpack_from('<I', m, a)[0]
    f4 = lambda a: struct.unpack_from('<4f', m, a)
    pins = {u(o + 0x120): k for k, o in enumerate(OBJECTS)}
    who = lambda r: 'pin%d' % pins[r] if r in pins else hex(r)
    n, at = u(NODE + 0x30), u(NODE + 0x38)
    filed = [u(at + 4 * k) for k in range(min(n, 64))]
    print(name, 'frame', u(FRAME), 'filed', n, [who(r) for r in filed],
          'listeners', [hex(u(u(NODE + 0x50) + 4 * k)) for k in range(u(NODE + 0x48))])
    for r in filed:
        k, e = u(r + 0x100), u(r + 0x108)
        if not 0 < e < len(m) or k > 64:
            print('  %-9s no list' % who(r))
            continue
        flags = [u(r + o) for o in (0x40, 0x44, 0x48, 0x4c)]
        ents = [(u(e + 0x80 * i), who(u(e + 0x80 * i + 0x74)) if u(e + 0x80 * i + 0x74) else '-',
                 ['%.3f' % v for v in f4(e + 0x80 * i + 0x40)[:3]]) for i in range(k)]
        print('  %-9s flags %s list %d %s' % (who(r), flags, k, ents))
