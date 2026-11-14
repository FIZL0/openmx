"""MD160: an Untamed (SLUS-21701) truck state's RAM image, and the trucks in it.

  python3 -I research/md160/ram.py STATE.p2s [OUT.ee.bin]

Prints the vsync word (0x410318) and every object whose +4 is the monster truck's vtable (0x003f1840),
with its body (+0xe8) position, velocity and angular damping, its collision record (+0xf8: +0x44 damp,
+0x48/+0x50 rest damping, +0x4c/+0x54 contact damping) and its engine/tire pointers. The player's truck
is the one moving. With OUT, writes the state's eeMemory.bin there.
"""
import struct
import sys
import zipfile

state = sys.argv[1]
with zipfile.ZipFile(state) as z:
    ram = z.read(next(n for n in z.namelist() if n.endswith('eeMemory.bin')))
if len(sys.argv) > 2:
    open(sys.argv[2], 'wb').write(ram)
u = lambda a: struct.unpack_from('<I', ram, a)[0]
f = lambda a, n: tuple(round(x, 4) for x in struct.unpack_from(f'<{n}f', ram, a))
print(f'vsync {u(0x410318)}')
w = struct.pack('<I', 0x3f1840)
o = ram.find(w)
while o >= 0:
    if o % 4 == 0 and o > 0x400000:
        obj = o - 4
        body, rec = u(obj + 0xe8), u(obj + 0xf8)
        if 0 < body < len(ram):
            print(f'truck {obj:#010x} body {body:#010x} record {rec:#010x} engine {u(obj + 0x1240):#010x} '
                  f'tires {[hex(u(obj + 0x1244 + 4 * i)) for i in range(4)]} state {u(obj + 0xbb4)}')
            print(f'  pos {f(body + 0x60, 3)} vel {f(body + 0x80, 3)} rate {f(body + 0x1d0, 3)}')
            print(f'  lindamp {f(body + 0x140, 3)} angdamp {f(body + 0x150, 3)}')
            if 0 < rec < len(ram):
                print(f'  record +0x34 {u(rec + 0x34)} +0x44 {u(rec + 0x44)} +0x48 {f(rec + 0x48, 1)} '
                      f'+0x4c {f(rec + 0x4c, 1)} +0x50 {f(rec + 0x50, 1)} +0x54 {f(rec + 0x54, 1)} '
                      f'+0xe4 {u(rec + 0xe4)}')
    o = ram.find(w, o + 1)
