"""The collision records and clips a C10 capture names, out of a RAM image of the same run.

    python3 -B research/c10/records.py c10/2p-start          # context/states/<name>.ram

Prints: the six path cars (vtable at +8) with their mesh collidable (+0x428) and position; both
riders' records (vehicle +0x7d0) and their wreck bikes' records; the biplanes' records (+0x170); and
each rider body's sixty-entry clip table (+0x900 of the body, bike actor - 0x400) with the name each
clip object carries. Research, not product.
"""
import os
import struct
import sys

GAME_MANAGER = 0x004e1e98
CAR_VTABLE = 0x0044d370
ram = open(os.path.join('context', 'states', sys.argv[1] + '.ram'), 'rb').read()


def u32(a):
    return struct.unpack_from('<I', ram, a & 0x1ffffff)[0]


def f(a, n):
    return tuple(round(x, 1) for x in struct.unpack_from('<%df' % n, ram, a & 0x1ffffff))


def cstr(a, n=48):
    b = ram[a & 0x1ffffff:(a & 0x1ffffff) + n].split(b'\0')[0]
    return b.decode('latin1') if b and all(32 <= c < 127 for c in b) else None


def actors():
    node = u32(u32(u32(GAME_MANAGER) + 0x34) + 0x34)
    out = []
    while node and len(out) < 64:
        out.append(u32(node))
        node = u32(node + 8)
    return out


v = struct.pack('<I', CAR_VTABLE)
i = 0
n = 0
while True:
    i = ram.find(v, i)
    if i < 0:
        break
    if i % 4 == 0:
        car = i - 8
        col = u32(car + 0x428)
        print('car%d %08x collidable %08x (its +0x7c %08x) pos %s' % (n, car, col, u32(col + 0x7c),
                                                                    f(car + 0x2d0, 3)))
        n += 1
    i += 4
items = actors()
for k, it in enumerate(items):
    t = u32(it)
    if t == 1:
        print('biplane entry %d record %08x' % (k, it + 0x170))
    if t == 2:
        print('vehicle entry %d player %d record %08x' % (k, u32(u32(it + 0x20) + 0x30), u32(it + 0x7d0)))
    if t == 4:
        body = it - 0x400
        print('bike entry %d player %d' % (k, u32(u32(it + 0x20) + 0x30)))
        if '--clips' in sys.argv:
            for c in range(60):
                clip = u32(body + 0x900 + 4 * c)
                name = cstr(u32(u32(clip + 4))) if 0x100000 < clip < 0x2000000 else None  # a handle: {joint map, clip}
                print('  clip %2d %08x %s' % (c, clip, name))
