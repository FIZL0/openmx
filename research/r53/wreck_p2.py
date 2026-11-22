"""R53: wreck P2 through the original's own wreck test, riding at SPEED ft/s toward free09's parked truck.

usage: python3 -B research/r53/wreck_p2.py PID LOAD_SLOT SPEED SAVE_SLOT [SECONDS]

Load LOAD_SLOT (P2 staged by tele_p2.py, standing a few ft from the truck and facing it), pause, and
in one paused moment:
- set P2's bike velocity (the chassis' +0x260/+0x280/+0x2b8/+0x2d0/+0x310 and the vehicle's +0x130,
  the words tele_p2.py zeroes) to SPEED along the bike's heading to the truck (level);
- set P2's forward limit (rider +0x758, 22.0 in this boot) to -1e30, so the wreck test's first clause
  (0x002522c0: `+0x758 < +0x558`, reason 4) holds on the next tick and the game enters its own wreck
  (vt +0xfc). handling/wreck.rs's offsets are SLUS-21104's, not this game's.
Save that poked moment to SAVE_SLOT + 1 (its next tick enters the wreck; the limit is still -1e30
there). Resume, pause as soon as the wreck reason (+0x748) is set, put the limit back, print the wreck body
(*(rider + 0x7a8) + 0x110: +0x60 position, +0x70 velocity) and save SAVE_SLOT. Then resume SECONDS
(default 0) logging a line a frame, so a dry run shows whether the bike reaches the truck.
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

PID, LOAD, SPEED, SLOT = int(sys.argv[1]), int(sys.argv[2]), float(sys.argv[3]), int(sys.argv[4])
SECS = float(sys.argv[5]) if len(sys.argv) > 5 else 0.0
FRAME, MANAGER, CAR_VTABLE, ACTOR_VTABLE = 0x00479d04, 0x004e1e98, 0x0045edac, 0x00458eb0
FORE_LIMIT, REASON = 0x758, 0x748

item = A.menu_path(A.app_of(PID), ['System', 'Pause'])
paused = lambda: item.get_state_set().contains(Atspi.StateType.CHECKED)
p = Pine(check=False, lock=False, timeout=5.0)
ok = lambda a: 0x100000 <= a < 0x2000000


def wait(s):
    end = time.time() + s
    while time.time() < end:
        time.sleep(0.01)


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


def line(rider, veh, truck):
    tb = p.read32(truck + 0x160)
    wb = p.read32(p.read32(rider + 0x7a8) + 0x110)
    w = f3(wb + 0x60) + f3(wb + 0x70) if ok(wb) else (0,) * 6
    return '%6d reason %2d bike %8.2f %7.2f %8.2f wreck %8.2f %7.2f %8.2f v %6.2f %6.2f %6.2f truck %d %8.2f %7.2f %8.2f v %6.2f %6.2f %6.2f' % (
        (p.read32(FRAME), p.read32(rider + REASON)) + f3(veh + 0x1b0) + w + (p.read32(tb),) + f3(tb + 0x60) + f3(tb + 0x70))


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
ch = p.read32(rider + 0x7a4)
at = f3(p.read32(truck + 0x160) + 0x60)
vp = f3(veh + 0x1b0)
d = (at[0] - vp[0], at[2] - vp[2])
n = math.hypot(*d)
v = (SPEED * d[0] / n, 0.0, SPEED * d[1] / n)
limit = p.read32(rider + FORE_LIMIT)
print('frame', f0, 'rider', hex(rider), 'veh', hex(veh), 'chassis', hex(ch), 'truck', hex(truck),
      '%.2f ft from the truck, velocity' % n, ['%.2f' % c for c in v],
      'fore limit %#x (%.3f)' % (limit, struct.unpack('<f', struct.pack('<I', limit))[0]), flush=True)
print(line(rider, veh, truck), flush=True)
for o in (0x260, 0x280, 0x2b8, 0x2d0, 0x310):
    for k in range(3):
        p.write_f32(ch + o + 4 * k, v[k])
for k in range(3):
    p.write_f32(veh + 0x130 + 4 * k, v[k])
p.write_f32(rider + FORE_LIMIT, -1e30)
p.save_state(SLOT + 1)
wait(2.0)
print('saved the poked state', SLOT + 1, 'at frame', p.read32(FRAME), flush=True)
A.press(item)
t0 = time.time()
while p.read32(rider + REASON) == 0:
    assert time.time() - t0 < 5, 'no wreck in 5 s'
A.press(item)
wait(0.5)
p.write32(rider + FORE_LIMIT, limit)
print('wrecked:', line(rider, veh, truck), flush=True)
p.save_state(SLOT)
wait(2.0)
print('saved', SLOT, 'at frame', p.read32(FRAME), flush=True)
if SECS:
    A.press(item)
    last = None
    t0 = time.time()
    while time.time() - t0 < SECS:
        f = p.read32(FRAME)
        if f != last:
            print(line(rider, veh, truck), flush=True)
            last = f
        time.sleep(0.004)
    A.press(item)
