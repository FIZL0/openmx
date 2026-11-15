"""MD147: an Untamed (SLUS-21701) race quad state's RAM image, and the quads in it.

  python3 -I research/md147/ram.py STATE.p2s [OUT.ee.bin]

research/md164/ram.py for the quad: finds the quad's vtable as the rodata word table holding its update
(0x004dd000, research/oracle.txt # MD81b), then prints every object whose +4 is that vtable, with its body
(+0xe8), engine (+0x143c), wheels (+0x17f0..+0x17fc), vehicle state pointer (+0xbb4), the hazard words
(stuck +0xc00, free +0xc04, time +0xde0) and the wheels' +0xd0 (grounded). The player's quad is the one
moving. With OUT, writes eeMemory.bin there.
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
ok = lambda p: 0x1000000 < p < len(ram)
print(f'vsync {u(0x410318)}')
slots = []
o = ram.find(struct.pack('<I', 0x004dd000))
while o >= 0:
    if o % 4 == 0 and o < 0x500000:
        slots.append(o)
    o = ram.find(struct.pack('<I', 0x004dd000), o + 1)
print('quad update held at', [hex(s) for s in slots])
seen = set()
for s in slots:
    for start in range(s, s - 0x600, -4):
        w = struct.pack('<I', start)
        o = ram.find(w, 0x1000000)
        while o >= 0:
            obj = o - 4
            if o % 4 == 0 and obj not in seen and ok(u(obj + 0xe8)) and ok(u(obj + 0x143c)) \
                    and all(ok(u(obj + 0x17f0 + 4 * i)) for i in range(4)):
                seen.add(obj)
                body, eng = u(obj + 0xe8), u(obj + 0x143c)
                wheels = [u(obj + 0x17f0 + 4 * i) for i in range(4)]
                print(f'vtable {start:#010x} (update at +{s - start:#x})')
                print(f'quad {obj:#010x} body {body:#010x} engine {eng:#010x} wheels {[hex(w) for w in wheels]} '
                      f'state ptr {u(obj + 0xbb4):#x} kind {u(obj + 0xbac)}')
                print(f'  pos {f(body + 0x60, 3)} vel {f(body + 0x80, 3)} rate {f(body + 0x1d0, 3)}')
                print(f'  lindamp {f(body + 0x140, 3)} angdamp {f(body + 0x150, 3)} quat {f(body + 0x170, 4)}')
                print(f'  euler {f(obj + 0x390, 3)} speed {f(obj + 0xc28, 1)} ctl {f(obj + 0x338, 4)} '
                      f'wreck words {u(obj + 0xc70)} {u(obj + 0xc74):#x}')
                print(f'  hazard stuck {u(obj + 0xc00)} free {u(obj + 0xc04)} time {f(obj + 0xde0, 1)} '
                      f'run {u(obj + 0x6a4):#x} run+8 {u(u(obj + 0x6a4) + 8) if ok(u(obj + 0x6a4)) else 0:#x}')
                print(f'  wheels +0xd0 {[u(w + 0xd0) for w in wheels]} +0xdc {[u(w + 0xdc) for w in wheels]}')
                print(f'  engine gear {u(eng + 0xc0)} of {u(eng + 0xd0)} final {f(eng + 0x48, 1)} '
                      f'redline {f(eng + 0x124, 1)}')
            o = ram.find(w, o + 1)
