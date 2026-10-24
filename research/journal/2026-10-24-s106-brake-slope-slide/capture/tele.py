"""S106: a braked bike standing on a slope, saved as a state.

    ./kit pcsx2 with -- python3 -B tele.py DX DY DZ FRAMES SLOT

Loads slot 8 (first_recording), pauses (the pad's Guide), moves the human's bike by (DX, DY, DZ) in
every copy of its position (P21's tele2.py / S89's tele.py: every float triple within 12 ft of the
vehicle's position in the rider, the chassis, the vehicle and the transform block), zeroes the
chassis' and the vehicle's velocities, holds the brake (square), lets FRAMES game frames run so the
wheels' contact words are the new ground's, pauses and saves to SLOT, with the brake still held."""
import os
import struct
import subprocess
import sys
import time

sys.path.insert(0, '.')
from ps2kit.pine import Pine

DX, DY, DZ = (float(v) for v in sys.argv[1:4])
FRAMES = int(sys.argv[4])
SLOT = int(sys.argv[5])
FC = 0x479d04


def pad(*cmds):
    subprocess.run(['./kit', 'pad', 'send', *cmds], check=True)


p = Pine()
pad('release')
p.load_state(8)
time.sleep(1.5)


def ok(a):
    return 0x100000 <= a < 0x2000000


def human(kind):
    node = p.read32(p.read32(p.read32(0x004e1e98) + 0x34) + 0x34)
    for _ in range(40):
        it = p.read32(node)
        if ok(it) and p.read32(it) == kind:
            pl = p.read32(it + 0x20)
            if ok(pl) and p.read32(pl + 0x34) == 1:
                return it
        node = p.read32(node + 0x8)
    raise SystemExit('no human actor of type %d' % kind)


p.pause()
f0 = p.read32(FC)
time.sleep(0.4)
assert p.read32(FC) == f0, 'not paused'
veh = human(4)
rider = human(2)
ch = p.read32(rider + 0x7a4)
vp = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
d = (DX, DY, DZ)
print('frame', f0, 'veh', hex(veh), 'rider', hex(rider), 'pos', vp, 'delta', d)
regions = [(rider, 0x2000), (ch, 0x400), (veh, 0x800), (p.read32(rider + 0x18) - 0x1600, 0x1800)]
# The wheels' own blocks hold contact points and load points too.
for o in (0x7b0, 0x7b4):
    regions.append((p.read32(rider + o), 0x900))
moved = 0
for base, n in regions:
    b = p.read_block(base, n)
    off = 0
    while off + 12 <= n:
        v = struct.unpack_from('<3f', b, off)
        if all(abs(v[k] - vp[k]) < 12 for k in range(3)):
            for k in range(3):
                p.write_f32(base + off + 4 * k, v[k] + d[k])
            moved += 1
            off += 12
        else:
            off += 4
print('moved', moved, 'triples')
for o in (0x260, 0x280, 0x2b8, 0x2d0, 0x310):
    for k in range(3):
        p.write_f32(ch + o + 4 * k, 0.0)
for k in range(3):
    p.write_f32(veh + 0x130 + 4 * k, 0.0)
pad('down square')
p.resume()
while p.read32(FC) < f0 + FRAMES:
    time.sleep(0.02)
p.pause()
pos = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
vel = struct.unpack('<3f', p.read_block(ch + 0x260, 12))
print('paused at', p.read32(FC), 'pos %.2f %.2f %.2f' % pos, 'vel %.3f %.3f %.3f' % vel)
p.save_state(SLOT)
time.sleep(2.0)
print('saved', SLOT)
p.resume()
pad('release')
