"""S117: the human's bike's pose and speed now: the node's right, up and forward rows, the chassis'
world velocity and each wheel's contact normal (+0x40 of the wheel block, S89's contact).

    ./kit pcsx2 with -- python3 -B pose.py"""
import struct
import sys

sys.path.insert(0, '.')
from ps2kit.pine import Pine

p = Pine()


def ok(a):
    return 0x100000 <= a < 0x2000000


def human(kind):
    node = p.read32(p.read32(p.read32(0x004e1e98) + 0x34) + 0x34)
    for _ in range(40):
        it = p.read32(node)
        if ok(it) and p.read32(it) == kind:
            pl = p.read32(it + 0x20)
            if ok(pl) and p.read32(pl + 0x34) == 1:
                return it
        node = p.read32(node + 0x8)
    raise SystemExit('no human actor of type %d' % kind)


rider = human(2)
ch = p.read32(rider + 0x7a4)
tf = p.read32(ch + 0x254)
v = lambda a: struct.unpack('<3f', p.read_block(a, 12))
for name, o in (('right', 0x80), ('up', 0x90), ('forward', 0xa0), ('origin', 0xb0)):
    print('%8s %s' % (name, ' '.join('%9.3f' % x for x in v(tf + o))))
print('     vel', ' '.join('%9.3f' % x for x in v(ch + 0x260)))
