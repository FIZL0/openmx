"""Both riders of a C10 RAM image: player records, vehicles, wheels, records and the bodies.

    python3 -B research/c10/peek.py c10/2p-start        # context/states/<name>.ram

Takes a name, not a path (the guard refuses big context paths). Research, not product.
"""
import os
import struct
import sys

GAME_MANAGER = 0x004e1e98
FRAME_COUNTER = 0x00479d04
ram = open(os.path.join('context', 'states', sys.argv[1] + '.ram'), 'rb').read()


def u32(a):
    return struct.unpack_from('<I', ram, a & 0x1ffffff)[0]


def f(a, n):
    return tuple(round(x, 3) for x in struct.unpack_from('<%df' % n, ram, a & 0x1ffffff))


def s(a, n=64):
    b = ram[a & 0x1ffffff:(a & 0x1ffffff) + n]
    return b.split(b'\0')[0].decode('latin1')


def actors():
    node = u32(u32(u32(GAME_MANAGER) + 0x34) + 0x34)
    out = []
    while node and len(out) < 64:
        out.append(u32(node))
        node = u32(node + 8)
    return out


print('frame_counter', u32(FRAME_COUNTER))
items = actors()
for i, it in enumerate(items):
    typ = u32(it)
    pl = u32(it + 0x20)
    idx = u32(pl + 0x30) if pl else None
    line = '%d %08x type %d player %08x idx %s' % (i, it, typ, pl, idx)
    if typ == 2:
        phy = u32(it + 0x84)
        line += ' phy %s wreck %d front %08x rear %08x rec %08x centre %s' % (
            s(phy) if 0x100000 < phy < 0x2000000 else hex(phy), u32(it + 0x748), u32(it + 0x7b0),
            u32(it + 0x7b4), u32(it + 0x7d0), f(u32(it + 0x7a4) + 0x60, 3))
        for w in ('front', 'rear'):
            wh = u32(it + (0x7b0 if w == 'front' else 0x7b4))
            line += '\n    %s contact %d owner %08x vehicle %08x' % (w, u32(wh + 0xd4), u32(wh + 0x1f0),
                                                                  u32(wh + 0x1f4))
    if typ == 4:
        body = it - 0x400
        line += ' pos %s body %s joints %d mode %d prev %d req %08x' % (
            f(it + 0x1b0, 3), s(body + 0x1e0, 12), u32(body + 0x230), u32(body + 0x47c), u32(body + 0x480),
            u32(body + 0x488))
    print(line)
