"""MD166: which camera a SLUS-21104 save state is on, and the event it is in.

  python3 -I research/md166/cams.py STATE.p2s|IMAGE.ee.bin [OUT.ee.bin]

Finds every camera controller (vtable 0x003ccc90 at +4, research/oracle.txt # MD166) and prints its race
camera (+0x7c), its follow camera (+0x80), which of the two is current (+0x78) and the camera's target
block (+0x80 of the camera). Then every player (+0x44 the controller) and its event's type word
(+0x6ac -> +0x3c0 -> +0x14: 5 hill climb, 8 open class, 17 free ride). Writes the RAM image to OUT.
"""
import struct
import sys
import zipfile

src = sys.argv[1]
if src.endswith('.p2s'):
    with zipfile.ZipFile(src) as z:
        ram = z.read(next(n for n in z.namelist() if n.endswith('eeMemory.bin')))
else:
    ram = open(src, 'rb').read()
if len(sys.argv) > 2:
    open(sys.argv[2], 'wb').write(ram)
u = lambda a: struct.unpack_from('<I', ram, a & 0x1ffffff)[0]
ok = lambda a: 0x100000 <= a < 0x2000000
RACE, FOLLOW, CTL = 0x3cc180, 0x3cc6f0, 0x3ccc90
print(f'vsync {u(0x3fa6d8)}')
ctls = []
o = ram.find(struct.pack('<I', CTL))
while o >= 0:
    if o % 4 == 0 and o > 0x400000:
        c = o - 4
        cur, race, follow = u(c + 0x78), u(c + 0x7c), u(c + 0x80)
        if ok(race) and ok(follow) and u(race + 4) == RACE and u(follow + 4) == FOLLOW:
            which = 'race' if cur == race else 'follow' if cur == follow else hex(cur)
            print(f'controller {c:#x}: race {race:#x} follow {follow:#x} current {which} target {u(cur + 0x80):#x}')
            ctls.append(c)
    o = ram.find(struct.pack('<I', CTL), o + 4)
for c in ctls:
    o = ram.find(struct.pack('<I', c))
    while o >= 0:
        p = o - 0x44
        if o % 4 == 0 and ok(u(p + 0x6ac)) and ok(u(u(p + 0x6ac) + 0x3c0)):
            ev = u(u(p + 0x6ac) + 0x3c0)
            name = ram[u(ev + 8) & 0x1ffffff:].split(b'\0')[0].decode('latin1')
            print(f'player {p:#x} -> controller {c:#x}: vehicle {u(p + 0x6a8):#x} event type {u(ev + 0x14)} ({name})')
        o = ram.find(struct.pack('<I', c), o + 4)
