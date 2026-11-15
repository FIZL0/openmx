"""MD147: the race quad's surface hazard table from a state: 16 entries of 12 bytes at quad +0xde4
(hazard word, time f32, a third word), indexed by each wheel's surface byte (+0x110), as the quad update
reads them before its hazard clock call (0x004dd254, research/oracle.txt # MD81b). Also each wheel's surface.

  python3 -I research/md147/surfaces.py STATE.p2s QUAD
"""
import struct
import sys
import zipfile

with zipfile.ZipFile(sys.argv[1]) as z:
    ram = z.read(next(n for n in z.namelist() if n.endswith('eeMemory.bin')))
q = int(sys.argv[2], 16)
u = lambda a: struct.unpack_from('<I', ram, a)[0]
for i in range(16):
    h, t, x = struct.unpack_from('<IfI', ram, q + 0xde4 + 12 * i)
    print(f'surface {i:2d}: hazard {h} time {t:g} third {x:#x}')
print('wheel surfaces', [ram[u(q + 0x17f0 + 4 * w) + 0x110] for w in range(4)], 'on_hazard +0xc10', u(q + 0xc10))
