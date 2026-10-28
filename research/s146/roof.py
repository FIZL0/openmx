"""S146: park the bike on a Dallas train wagon's roof in the original, and save the state once it has
ridden there a while — so a capture from the state is a braked bike carried by the train from its first row.

usage: python3 -B research/s146/roof.py PID LOAD_SLOT TARGET_CLOCK RIDE_FRAMES SAVE_SLOT [CAR]

From S129's along state (copy 2's slot 8): hold both brakes (square and r2), let the game run a few frames and
until the engine's path clock (research/oracle.txt S81) has passed TARGET_CLOCK,
pause, and move the bike (research/s129/tele.py's teleport: every copy of its position moved by one delta)
to 2.5 ft over the middle of car CAR's roof (S81's tenth car by default, the train's second train_car_06, a boxcar whose roof
is 16.6 ft over its node), with the chassis' velocity set to the wagon's own (its node's position change
over the last frame) so the bike does not land at rest under a 55 ft/s wagon. Then run RIDE_FRAMES more
frames, brakes held, and save into SAVE_SLOT. Run it under `./kit pcsx2 with --`.
"""

import struct
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'research/journal/2026-10-20-c6-c6-recording2-capture/playback'))
from ps2kit.config import cfg            # noqa: E402
from ps2kit.pine import Pine             # noqa: E402
import atspi_lib as A                    # noqa: E402
from gi.repository import Atspi          # noqa: E402

PID, LOAD = int(sys.argv[1]), int(sys.argv[2])
TARGET = float(sys.argv[3])
RIDE, SLOT = int(sys.argv[4]), int(sys.argv[5])
CARS = [0x00d525f0, 0x00d5cb60, 0x00d63660, 0x00d6c990, 0x00d76440, 0x00d7fd50, 0x00d869e0, 0x00d87930,
        0x00d8bab0, 0x00d8f670]
CAR = CARS[int(sys.argv[6]) if len(sys.argv) > 6 else 9]
ENGINE = CARS[0]
ROOF = 16.6
OVER = 2.5
SYM = cfg().symbols
FRAME = SYM['frame_counter']
MANAGER = SYM['game_manager']

app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


def wait(seconds):
    end = time.time() + seconds
    while time.time() < end:
        time.sleep(0.05)


p = Pine()


def ok(a):
    return 0x100000 <= a < 0x2000000


def human(kind):
    node = p.read32(p.read32(p.read32(MANAGER) + 0x34) + 0x34)
    for _ in range(40):
        it = p.read32(node)
        if ok(it) and p.read32(it) == kind:
            pl = p.read32(it + 0x20)
            if ok(pl) and p.read32(pl + 0x34) == 1:
                return it
        node = p.read32(node + 0x8)
    raise SystemExit('no human actor of type %d' % kind)


def clock():
    return struct.unpack('<f', p.read_block(p.read32(ENGINE + 0x430) + 0xc, 4))[0]


def car_pos():
    return struct.unpack('<3f', p.read_block(p.read32(CAR + 0x254) + 0xb0, 12))


def car_old():
    """The car's node position before this frame's step (car +0x2e0's matrix, S136)."""
    return struct.unpack('<3f', p.read_block(CAR + 0x310, 12))


if not paused():
    A.press(item)
wait(0.5)
subprocess.run([str(ROOT / 'kit'), 'pad', 'send', 'down square', 'down r2'], check=False)
p.load_state(LOAD)
wait(2.0)
if paused():
    A.press(item)
f0 = p.read32(FRAME)
c0 = clock()
print('loaded', LOAD, 'frame', f0, 'clock', c0, flush=True)
wrapped = TARGET >= c0 or TARGET <= 0
while True:
    c = clock()
    if c < c0 - 1:
        wrapped = True
    # A few frames on, so the loaded state's brakes are the pad's.
    if wrapped and c >= TARGET and p.read32(FRAME) > f0 + 15:
        break
    time.sleep(0.01)
A.press(item)
wait(0.5)
f1 = p.read32(FRAME)
wait(0.3)
assert p.read32(FRAME) == f1, 'not paused'
veh = human(4)
rider = human(2)
ch = p.read32(rider + 0x7a4)
vp = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
cp, co = car_pos(), car_old()
dt = struct.unpack('<f', p.read_block(SYM['frame_dt'], 4))[0]
cv = tuple((cp[k] - co[k]) / dt for k in range(3))
X, Y, Z = cp[0], cp[1] + ROOF + OVER, cp[2]
d = (X - vp[0], Y - vp[1], Z - vp[2])
print('frame', f1, 'clock', clock(), 'car', cp, 'car vel', cv, 'bike', vp, 'delta', d, flush=True)
regions = [(rider, 0x2000), (ch, 0x400), (veh, 0x800), (p.read32(rider + 0x18) - 0x1600, 0x1800)]
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
for o in (0x280, 0x2b8, 0x2d0, 0x310):
    for k in range(3):
        p.write_f32(ch + o + 4 * k, 0.0)
for k in range(3):
    p.write_f32(ch + 0x260 + 4 * k, cv[k])
    p.write_f32(veh + 0x130 + 4 * k, 0.0)
A.press(item)
end = f1 + RIDE
last = 0.0
while p.read32(FRAME) < end:
    if time.time() - last > 1.0:
        pos = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
        vel = struct.unpack('<3f', p.read_block(ch + 0x260, 12))
        print('%5d car x %8.2f pos %9.2f %8.2f %9.2f vel %7.2f %7.2f %7.2f' % ((p.read32(FRAME), car_pos()[0]) + pos + vel), flush=True)
        last = time.time()
    time.sleep(0.005)
A.press(item)
wait(0.5)
pos = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
print('paused at frame', p.read32(FRAME), 'car', car_pos(), 'bike', pos, flush=True)
p.save_state(SLOT)
wait(2.0)
print('saved', SLOT, flush=True)
