"""P74: list the human's vehicle particle manager out of a save state's EE RAM image.

usage: python3 -B walk_state.py <ee.bin>

Prints the manager's entries in order: each one's vtable (its type), flag byte, tire record, emitter,
and the emitter's group, carry and file rates, so the capture spec can name them by index.
"""

import struct
import sys

ram = open(sys.argv[1], 'rb').read()


def u32(a):
    return struct.unpack_from('<I', ram, a & 0x1ffffff)[0]


def f32(a):
    return struct.unpack_from('<f', ram, a & 0x1ffffff)[0]


TYPES = {0x45e780: 'Exhaust', 0x45e750: 'Dust', 0x45e720: 'Spray', 0x45e6f0: 'Generic',
         0x45e660: 'ParticleCollisionConstraint'}
GAME_MANAGER = 0x004e1e98
g = u32(u32(u32(GAME_MANAGER) + 0x34) + 0x34)
rider = u32(u32(g + 0x8) + 0x0)
print(f'rider {rider:#x} reason {u32(rider + 0x748)} tires {u32(rider + 0xf60)} at {u32(rider + 0xf64):#x}')
mgr = u32(rider + 0x1490)
n = u32(mgr + 0x34)
arr = u32(mgr + 0x3c)
print(f'manager {mgr:#x}: {n} entries at {arr:#x}')
tires = u32(rider + 0xf64)
for i in range(n):
    e = u32(arr + 4 * i)
    vt = u32(e + 0x10)
    em = u32(e + 0x14)
    tire = u32(e + 4)
    t = 'none' if tire == 0 else f'tire {(tire - tires) // 0x2c}'
    print(f'{i:2} {e:#x} {TYPES.get(vt, hex(vt)):28} flag {ram[(e + 8) & 0x1ffffff]:#x} disable {u32(e + 0xc):#x} {t}'
          f' emitter {em:#x} group {u32(em + 0x214) if em else -1} carry {f32(em + 0x3b0) if em else 0:.4f}'
          f' rate {f32(em + 0x340) if em else 0:.1f}..{f32(em + 0x344) if em else 0:.1f}'
          f' scale {f32(em + 0x254) if em else 0:.3f} flags {u32(em + 0x1d4) if em else 0:#x}')
em = u32(u32(arr + 4 * 5) + 0x14)
vt = u32(em + 0x38)
print(f'emitter vtable {vt:#x}: ' + ' '.join(f'+{o:#x} {u32(vt + o):#x}' for o in range(0x80, 0xa4, 4)))
