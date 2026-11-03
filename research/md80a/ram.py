"""MD80a: a flier state's RAM image, and the fliers in it.

  python3 -I research/md80a/ram.py NAME

Writes context/mxvatv-ps2/md80a/NAME.ee.bin (eeMemory.bin out of NAME.p2s) and prints the vsync word and
every object whose +4 is the biplane's or the Kiowa's vtable (0x003ce7b0, 0x003cfad0: the vtables holding
atv_biplane_read_phy and atv_kiowa_read_phy at +0xf4, research/oracle.txt # MD80), with its body's position
and velocity and the model word (+0x1320 biplane, +0x1340 Kiowa: 1 model one, 0 model two).
"""
import os
import struct
import sys
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DIR = os.path.join(ROOT, 'context', 'mxvatv-ps2', 'md80a')
VTABLES = {0x3ce7b0: ('biplane', 0x1320), 0x3cfad0: ('kiowa', 0x1340)}

name = sys.argv[1]
with zipfile.ZipFile(os.path.join(DIR, name + '.p2s')) as z:
    ram = z.read(next(n for n in z.namelist() if n.endswith('eeMemory.bin')))
open(os.path.join(DIR, name + '.ee.bin'), 'wb').write(ram)
u = lambda a: struct.unpack_from('<I', ram, a)[0]
f = lambda a, n: struct.unpack_from(f'<{n}f', ram, a)
print(f'vsync {u(0x3fa6d8)}')
for vt, (what, model) in VTABLES.items():
    w = struct.pack('<I', vt)
    o = ram.find(w)
    while o >= 0:
        if o % 4 == 0 and o > 0x400000:
            obj = o - 4
            body = u(obj + 0xb8)
            if 0 < body < len(ram):
                print(f'{obj:#010x} vptr {vt:#x} ({what}) model word {u(obj + model)} '
                      f'pos {f(body + 0x60, 3)} vel {f(body + 0x80, 3)}')
            else:
                print(f'{obj:#010x} vptr {vt:#x} ({what}) model word {u(obj + model)}, body {body:#x}')
        o = ram.find(w, o + 1)
