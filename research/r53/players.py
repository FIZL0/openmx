"""R53: print each actor in the manager's list with its player record (+0x20: index +0x30, control
source +0x34), and for a rider (+0x7a4 the chassis) its wreck words and chassis place.

usage: python3 -B research/r53/players.py     (under KIT_PCSX2=N ./kit pcsx2 with --)
"""
import struct, sys
sys.path.insert(0, '.')
from ps2kit.pine import Pine

MANAGER = 0x004e1e98
p = Pine(check=False, lock=False, timeout=5.0)
ok = lambda a: 0x100000 <= a < 0x2000000


def v3(a):
    return tuple(round(x, 3) for x in struct.unpack('<3f', p.read_block(a, 12)))


node = p.read32(p.read32(p.read32(MANAGER) + 0x34) + 0x34)
print('frame', p.read32(0x00479d04))
for _ in range(60):
    if not ok(node):
        break
    it = p.read32(node)
    if ok(it):
        kind, pl = p.read32(it), p.read32(it + 0x20)
        line = '%#x type %d' % (it, kind)
        if ok(pl):
            line += ' player %#x index %d src %d' % (pl, p.read32(pl + 0x30), p.read32(pl + 0x34))
        ch = p.read32(it + 0x7a4)
        if kind in (2, 8) and ok(ch) and ok(p.read32(it + 0x18)):
            line += ' reason %d wrecked %d chassis %#x centre %s speed %.2f' % (
                p.read32(it + 0x748), p.read32(it + 0xb7c), ch, v3(ch + 0x60), struct.unpack('<f', p.read_block(ch + 0x34, 4))[0])
        if kind == 4:
            line += ' pos %s' % (v3(it + 0x1b0),)
        print(line)
    node = p.read32(node + 0x8)
