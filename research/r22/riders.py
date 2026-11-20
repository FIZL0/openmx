"""R22: both riders' bikes in a RAM image of the R22 split boot: for each kind-0 entry of the world's
machine list (research/r22/machines.py), the ridden vehicle's player index (+0x20 -> +0x30), chassis
(+0x7a4) centre (+0x60), velocity, its quaternion and its collision record (+0x7d0).

    python3 -B research/r22/riders.py <name under context/states, without .ram>
"""
import os, struct, sys

ram = open(os.path.join('context', 'states', sys.argv[1] + '.ram'), 'rb').read()
u32 = lambda a: struct.unpack_from('<I', ram, a & 0x1ffffff)[0]
f32s = lambda a, n: tuple(round(x, 4) for x in struct.unpack_from('<%df' % n, ram, a & 0x1ffffff))
world = u32(u32(0x0041f770) + 0x3cc)
for i in range(u32(world + 0x90)):
    e = u32(u32(world + 0x98) + 4 * i)
    if u32(e + 4) != 0:
        continue
    v = u32(e + 8)
    pl = u32(v + 0x20)
    ch = u32(v + 0x7a4)
    print('bike %#x player %s chassis %#x record %#x' % (v, u32(pl + 0x30) if pl else None, ch, u32(v + 0x7d0)))
    for o, n in ((0x60, 'centre'), (0x1b0, '+1b0'), (0x170, '+170'), (0x150, '+150')):
        print('   ', n, f32s(ch + o, 4))
