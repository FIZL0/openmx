"""S241: what a state's free06 collision node holds: the frame, its filed collision objects, and their contact
lists (kind, the other record, the point), the plane's and the EyeBalls' named.

usage: python3 -B research/s241/lists.py NAME...   (context/states/s241-NAME.eeMemory.bin, made by
research/s241/slots.py ram NAME)"""
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
from ps2kit.config import cfg            # noqa: E402

NODE = 0x0060b200
PLANE = 0x01e73b00
RECORDS = [0xd46ac0, 0xd496a0, 0xd4c190, 0xd4ec80, 0xd51770, 0xd54260]

for name in sys.argv[1:]:
    m = (ROOT / 'context' / 'states' / ('s241-' + name + '.eeMemory' + '.bin')).read_bytes()
    u = lambda a: struct.unpack_from('<I', m, a)[0]
    f4 = lambda a: struct.unpack_from('<4f', m, a)
    balls = {r: k for k, r in enumerate(RECORDS)}
    who = lambda r: 'plane' if r == PLANE else 'ball%d' % balls[r] if r in balls else hex(r)
    print(name, 'frame', u(cfg().symbols['frame_counter']))
    n, at = u(NODE + 0x30), u(NODE + 0x38)
    filed = [u(at + 4 * k) for k in range(min(n, 64))] if 0 < at < len(m) else []
    print('  filed', n, [who(r) for r in filed])
    for r in filed + [x for x in RECORDS if x not in filed]:
        k, e = u(r + 0x100), u(r + 0x108)
        if not 0 < e < len(m) or k > 64:
            print('  %-9s no list' % who(r))
            continue
        flags = [u(r + o) for o in (0x40, 0x44, 0x48, 0x4c)]
        ents = [(u(e + 0x80 * i), who(u(e + 0x80 * i + 0x74)) if u(e + 0x80 * i + 0x74) else '-', ['%.3f' % v for v in f4(e + 0x80 * i + 0x40)[:3]]) for i in range(k)]
        print('  %-9s flags %s list %d %s' % (who(r), flags, k, ents))
