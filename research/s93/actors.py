"""List the actor manager's actors in an EE RAM image (S93).

    ./kit state ram <state.p2s> --out context/states/<name>.ram
    python3 -B research/s93/actors.py <name>

Walks @game_manager -> 0x34 -> 0x34 (list head) along `next` (+0x8) and prints each item's type,
its player record (+0x20), the control source (+0x20 -> +0x34) and, for vehicles, the position
(+0x1b0) and velocity (+0x130). Takes a name, not a path: the guard refuses big context paths.
"""
import os, struct, sys

GAME_MANAGER = 0x004e1e98
ram = open(os.path.join('context', 'states', sys.argv[1] + '.ram'), 'rb').read()


def u32(a):
    a &= 0x1ffffff
    return struct.unpack_from('<I', ram, a)[0]


def f32s(a, n):
    a &= 0x1ffffff
    return struct.unpack_from('<%df' % n, ram, a)


def r(v):
    return tuple(round(x, 2) for x in v)


gm = u32(GAME_MANAGER)
node = u32(u32(gm + 0x34) + 0x34)
i = 0
while node and i < 64:
    item = u32(node)
    typ = u32(item)
    player = u32(item + 0x20)
    src = u32(player + 0x34) if player else None
    extra = ''
    if typ == 4:
        extra = ' pos %s vel %s' % (r(f32s(item + 0x1b0, 3)), r(f32s(item + 0x130, 3)))
    if typ == 2:
        extra = ' wreck %d' % u32(item + 0x748)
    print('%2d node %08x item %08x type %d player %08x src %s%s' % (i, node, item, typ, player, src, extra))
    node = u32(node + 8)
    i += 1
