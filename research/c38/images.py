"""C38: read the saved states back: each one's own frame (vsync 0x3fa6d8) and the five quads' records, and
with OUT_DIR write its EE RAM and scratchpad as c38-<vsync>.ee.bin / .spr.bin.
usage: images.py SAVESTATES_DIR FIRST_SLOT LAST_SLOT [OUT_DIR]"""
import os
import struct
import sys
import zipfile

VSYNC = 0x3FA6D8
QUADS = [0x1551620, 0x15CA4B0, 0x16219D0, 0x1A0FC70, 0x1A65BA0]


def main():
    d, lo, hi = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
    out = sys.argv[4] if len(sys.argv) > 4 else None
    for slot in range(lo, hi + 1):
        z = zipfile.ZipFile(os.path.join(d, 'SLUS-21104 (95BB1901).%02d.' % slot + 'p2s'))
        ram = z.read('eeMemory' + '.bin')
        u = lambda a: struct.unpack_from('<I', ram, a & 0x1FFFFFF)[0]
        f = lambda a: struct.unpack_from('<f', ram, a & 0x1FFFFFF)[0]
        v = u(VSYNC)
        print(f'slot {slot} vsync {v}')
        for q in QUADS:
            assert u(q + 4) == 0x003CE250, hex(q)
            r = u(u(q + 0x1434) + 0x90)
            b = u(q + 0xB8)
            n, lst = u(r + 0xF0), u(r + 0xF8)
            ents = []
            for i in range(n):
                e = lst + i * 0x90
                ents.append(f'{u(e)}:{u(e + 0x84):x}')
            pos = ' '.join(f'{f(b + 0x70 + 4 * k):.3f}' for k in range(3))
            vel = ' '.join(f'{f(b + 0x80 + 4 * k):.3f}' for k in range(3))
            print(f'  quad {q:x} body {b:x} pos {pos} vel {vel} record {r:x} n {n} [{" ".join(ents)}]')
        if out:
            os.makedirs(out, exist_ok=True)
            open(os.path.join(out, f'c38-{v}.ee' + '.bin'), 'wb').write(ram)
            open(os.path.join(out, f'c38-{v}.spr' + '.bin'), 'wb').write(z.read('Scratchpad' + '.bin'))


main()
