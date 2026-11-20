"""R53: put P2's bike DIST ft behind free09's parked truck along P2's own heading, so riding straight
on meets the truck, settle it and save.

usage: python3 -B research/r53/tele_p2.py PID LOAD_SLOT DIST SIDE DY SECONDS SAVE_SLOT

research/c16/tele_p2.py with research/r14_windmill_bones.py's `bones` move, for R53's boot. In this boot
the list's types differ from the C16 state's: a player's rider (with the chassis at +0x7a4) is type 4
and its vehicle (the body actor at vehicle - 0x400, vtable 0x00458eb0) type 8; both carry the player
record (+0x20, index +0x30 = 1 for P2). The heading of travel is (-sin yaw, cos yaw), yaw the chassis'
+0x94. The vehicle goes to truck - DIST * heading + SIDE * (cos yaw, sin yaw) (SIDE > 0 to the bike's
right... or left: see the printout), at the truck body's height + DY. Load LOAD_SLOT, pause, move every
triple near the bike (rider, chassis, vehicle, the rider's pool, the body actor's neighbourhood, the
ragdoll parts and their bodies, the bone collisions) by one delta, zero the chassis' and vehicle's
velocities, resume SECONDS with P2's pad idle, pause and save SAVE_SLOT.
Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, on a copy started with the accessibility bus on.
"""
import math
import struct
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'research/journal/2026-10-20-c6-c6-recording2-capture/playback'))
from ps2kit.pine import Pine             # noqa: E402
import atspi_lib as A                    # noqa: E402
from gi.repository import Atspi          # noqa: E402

PID, LOAD = int(sys.argv[1]), int(sys.argv[2])
DIST, SIDE, DY = (float(v) for v in sys.argv[3:6])
SECS, SLOT = float(sys.argv[6]), int(sys.argv[7])
FRAME, MANAGER, CAR_VTABLE, ACTOR_VTABLE, PARTS = 0x00479d04, 0x004e1e98, 0x0045edac, 0x00458eb0, 18

item = A.menu_path(A.app_of(PID), ['System', 'Pause'])
paused = lambda: item.get_state_set().contains(Atspi.StateType.CHECKED)
p = Pine(check=False, lock=False, timeout=5.0)
ok = lambda a: 0x100000 <= a < 0x2000000


def wait(s):
    end = time.time() + s
    while time.time() < end:
        time.sleep(0.05)


def f3(a):
    return struct.unpack('<3f', p.read_block(a, 12))


def actors():
    node = p.read32(p.read32(p.read32(MANAGER) + 0x34) + 0x34)
    for _ in range(60):
        if not ok(node):
            return
        it = p.read32(node)
        if ok(it):
            yield it
        node = p.read32(node + 0x8)


if paused():
    A.press(item)
p.load_state(LOAD)
wait(3)
if not paused():
    A.press(item)
wait(0.5)
f0 = p.read32(FRAME)
wait(0.3)
assert p.read32(FRAME) == f0, 'not paused'
rider = veh = truck = None
for it in actors():
    pl = p.read32(it + 0x20)
    if p.read32(it + 0x88) == CAR_VTABLE:
        truck = it
    elif ok(pl) and p.read32(pl + 0x30) == 1:
        if p.read32(it - 0x400 + 8) == ACTOR_VTABLE:
            veh = it
        elif ok(p.read32(it + 0x7a4)):
            rider = it
assert rider and veh and truck, (rider, veh, truck)
actor = veh - 0x400
ch = p.read32(rider + 0x7a4)
yaw = f3(ch + 0x90)[1]
fwd = (-math.sin(yaw), math.cos(yaw))
side = (math.cos(yaw), math.sin(yaw))
at = f3(p.read32(truck + 0x160) + 0x60)
X = at[0] - DIST * fwd[0] + SIDE * side[0]
Z = at[2] - DIST * fwd[1] + SIDE * side[1]
Y = at[1] + DY
parts = p.read32(actor + 0x2f4)
bones = [p.read32(parts + i * 0xb0 + 0xa4) for i in range(PARTS)]
cols = p.read32(actor + 0x624)
coll = [p.read32(cols + 4 * i) for i in range(9)]
vp = f3(veh + 0x1b0)
d = (X - vp[0], Y - vp[1], Z - vp[2])
print('frame', f0, 'rider', hex(rider), 'veh', hex(veh), 'actor', hex(actor), 'chassis', hex(ch), 'truck', hex(truck),
      'yaw %.4f heading %.4f %.4f' % (yaw, fwd[0], fwd[1]), flush=True)
print('truck at', ['%.2f' % v for v in at], 'bike', ['%.2f' % v for v in vp], 'to', ['%.2f' % v for v in (X, Y, Z)], flush=True)
regions = [(rider, 0x2000), (ch, 0x400), (veh, 0x800), (p.read32(rider + 0x18) - 0x1600, 0x1800),
           (actor - 0x2800, 0x2c00), (veh + 0x800, 0x2000), (parts, max(bones) + 0x1400 - parts),
           (min(coll), max(coll) + 0x100 - min(coll))]
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
print('moved', moved, 'triples', flush=True)
for o in (0x260, 0x280, 0x2b8, 0x2d0, 0x310):
    for k in range(3):
        p.write_f32(ch + o + 4 * k, 0.0)
for k in range(3):
    p.write_f32(veh + 0x130 + 4 * k, 0.0)
A.press(item)
t0 = time.time()
while time.time() - t0 < SECS:
    pos = f3(veh + 0x1b0)
    vel = f3(ch + 0x260)
    print('%5d pos %9.2f %8.2f %9.2f vel %7.2f %7.2f %7.2f yaw %.4f reason %d' % (
        (p.read32(FRAME),) + pos + vel + (f3(ch + 0x90)[1], p.read32(rider + 0x748))), flush=True)
    wait(0.5)
if not paused():
    A.press(item)
wait(0.5)
p.save_state(SLOT)
wait(2.0)
print('saved', SLOT, 'at frame', p.read32(FRAME), flush=True)
