"""MD39a: a car state's RAM image, and the cars in it.

  python3 -I research/md39a/ram.py NAME

Writes context/mxvatv-ps2/md39a/NAME.ee.bin (eeMemory.bin out of NAME.p2s) and prints the vsync word
and every object whose +4 is one of the car vtables (research/oracle.txt # MD39), with its body's
position and velocity. The player's car is the one moving; each class has a second, idle object.
"""
import os
import struct
import sys
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DIR = os.path.join(ROOT, 'context', 'mxvatv-ps2', 'md39a')
VTABLES = {0x3cec60: 'buggy class (buggy, baja, sandrail)', 0x3cf600: 'golf cart', 0x3cff80: 'truck', 0x3cf130: 'car'}

name = sys.argv[1]
with zipfile.ZipFile(os.path.join(DIR, name + '.p2s')) as z:
    ram = z.read(next(n for n in z.namelist() if n.endswith('eeMemory.bin')))
open(os.path.join(DIR, name + '.ee.bin'), 'wb').write(ram)
u = lambda a: struct.unpack_from('<I', ram, a)[0]
f = lambda a, n: struct.unpack_from(f'<{n}f', ram, a)
print(f'vsync {u(0x3fa6d8)}')
for vt, what in VTABLES.items():
    w = struct.pack('<I', vt)
    o = ram.find(w)
    while o >= 0:
        if o % 4 == 0 and o > 0x400000:
            obj = o - 4
            body = u(obj + 0xb8)
            if 0 < body < len(ram):
                print(f'{obj:#010x} vptr {vt:#x} ({what}) pos {f(body + 0x60, 3)} vel {f(body + 0x80, 3)} '
                      f'CofG {f(obj + 0x10f0, 3)}')
        o = ram.find(w, o + 1)
