"""R22: the world's machine list (world +0x90: count, +0x98 the array) in a RAM image, as the pause
menu's "Switch To Machine" walks it (0x0028ff30): each entry's kind (+4: 0 bike, 1 biplane, 2 helicopter,
3 buggy, 4 truck, 5 baja), its actor (+8) and that actor's driver (+0x20), body (+0x160) and the body's
position (+0x60) and velocity (+0x70). Also the cheat flag that unlocks every machine.

    python3 -B research/r22/machines.py <name under context/states, without .ram>
"""
import os, struct, sys

APP = 0x0041f770
ram = open(os.path.join('context', 'states', sys.argv[1] + '.ram'), 'rb').read()
u32 = lambda a: struct.unpack_from('<I', ram, a & 0x1ffffff)[0]
f32s = lambda a, n: tuple(round(x, 3) for x in struct.unpack_from('<%df' % n, ram, a & 0x1ffffff))
world = u32(u32(APP) + 0x3cc)
n = u32(world + 0x90)
print('world %#x, %d machines, cheat flag %d' % (world, n, u32(u32(0x004e2778) + 0x5c)))
for i in range(n):
    e = u32(u32(world + 0x98) + 4 * i)
    actor = u32(e + 8)
    body = u32(actor + 0x160) if actor else 0
    print(i, 'entry %#x kind %d actor %#x driver %#x body %#x' % (e, u32(e + 4), actor, u32(actor + 0x20) if actor else 0, body),
          f32s(body + 0x60, 3) if body else '', f32s(body + 0x70, 3) if body else '')
