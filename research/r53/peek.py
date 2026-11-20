"""R48: print the live game's plane, truck and actor list over PINE.

usage: python3 -B research/r48/peek.py     (under KIT_PCSX2=N ./kit pcsx2 with --)
"""
import struct, sys
sys.path.insert(0, '.')
from ps2kit.pine import Pine

MANAGER, CAR_VTABLE = 0x004e1e98, 0x0045edac
p = Pine(check=False, lock=False, timeout=5.0)


def v3(a):
    return tuple(round(x, 3) for x in struct.unpack('<3f', p.read_block(a, 12)))


def actors():
    node = p.read32(p.read32(p.read32(MANAGER) + 0x34) + 0x34)
    out = []
    for _ in range(40):
        if not node:
            break
        it = p.read32(node)
        if it:
            out.append((it, p.read32(it)))
        node = p.read32(node + 0x8)
    return out


print('frame', p.frame(), 'cheat', p.read32(p.read32(0x004e2778) + 0x5c))
for it, kind in actors():
    line = '%#x type %d' % (it, kind)
    if kind == 1:
        b = p.read32(it + 0x160)
        line += ' plane pos %s vel %s spawn %s nose %s crash %d flown %d' % (
            v3(b + 0x60), v3(b + 0x70), v3(it + 0xe10), v3(it + 0xe20), p.read32(it + 0x3b0), p.read32(it + 0x334))
    if p.read32(it + 0x88) == CAR_VTABLE:
        b = p.read32(it + 0x160)
        line += ' car pos %s vel %s awake %d spawn %s nose %s' % (v3(b + 0x60), v3(b + 0x70), p.read32(b), v3(it + 0x270), v3(it + 0x280))
    print(line)
