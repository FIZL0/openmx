"""MD81e: an Untamed (SLUS-21701) golf cart state's RAM image, and the trucks in it.

  python3 -I research/md81e/ram.py STATE.p2s [OUT.ee.bin]

Finds the golf cart class's vtable as the rodata word table holding its class update (0x004edbe0, MD81e;
the baja's twin, research/oracle.txt # MD81d), then prints every object whose +4 is that vtable, with its body (+0xe8),
collision record (+0xf8), engine/tires (+0x1240..+0x1250), car state (+0xc70) and the preload words
(+0xc40..+0xc60, cued +0x724), the wreck words (+0xc70.., as research/md160's)
and the engine's gearbox (research/oracle.txt # MD125). The player's cart is the one moving. With OUT, writes eeMemory.bin there.
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
slots = []
o = ram.find(struct.pack('<I', 0x004edbe0))
while o >= 0:
    if o % 4 == 0 and o < 0x500000:
        slots.append(o)
    o = ram.find(struct.pack('<I', 0x004edbe0), o + 1)
print('class update held at', [hex(s) for s in slots])
seen = set()
for s in slots:
    # Walk back to the table's start: objects point at it, so try each word before the slot.
    for start in range(s, s - 0x400, -4):
        w = struct.pack('<I', start)
        o = ram.find(w, 0x1000000)
        while o >= 0:
            obj = o - 4
            if o % 4 == 0 and obj not in seen:
                body = u(obj + 0xe8)
                if 0x1000000 < body < len(ram) and 0x1000000 < u(obj + 0x1240) < len(ram):
                    seen.add(obj)
                    rec = u(obj + 0xf8)
                    print(f'vtable {start:#010x} (class update at +{s - start:#x})')
                    print(f'car {obj:#010x} body {body:#010x} record {rec:#010x} engine {u(obj + 0x1240):#010x} '
                          f'tires {[hex(u(obj + 0x1244 + 4 * i)) for i in range(4)]} state ptr {u(obj + 0xbb4):#x} '
                          f'car state {u(obj + 0xc70)} kind {u(obj + 0xbac)}')
                    print(f'  pos {f(body + 0x60, 3)} vel {f(body + 0x80, 3)} rate {f(body + 0x1d0, 3)}')
                    print(f'  lindamp {f(body + 0x140, 3)} angdamp {f(body + 0x150, 3)} rate clamp {f(body + 0x210, 1)}')
                    print(f'  preload ready/active/armed {u(obj + 0xc40)} {u(obj + 0xc44)} {u(obj + 0xc48)} '
                          f'time/timer/min/max/maxpre/force {f(obj + 0xc4c, 6)} cued {u(obj + 0x724)} '
                          f'pad word {u(obj + 0x1114):#x}')
                    e = u(obj + 0x1240)
                    print(f'  wreck state/clock/max {u(obj + 0xc70)} {f(obj + 0xc74, 2)} WreckDamping {f(obj + 0xc90, 3)} '
                          f'WreckInertia {f(obj + 0xca0, 1)} Inertia {f(obj + 0xcb0, 3)} wrecks_on {u(obj + 0x110c)}')
                    n, r = u(e + 0xd0), u(e + 0xd4)
                    print(f'  engine gear {u(e + 0xc0)} of {n} ratios {f(r, n) if 0 < r < len(ram) and n < 16 else hex(r)} '
                          f'final {f(e + 0x48, 1)} redline {f(e + 0x124, 1)}')
                    if 0 < rec < len(ram):
                        print(f'  record +0x34 {u(rec + 0x34)} +0x44 {u(rec + 0x44)} +0x48 {f(rec + 0x48, 4)} '
                              f'+0xe4 {u(rec + 0xe4):#x}')
            o = ram.find(w, o + 1)
