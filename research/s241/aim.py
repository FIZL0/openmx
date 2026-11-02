"""S241: put the biplane on free06's ground short of an EyeBall, rolling at it, and save the state.

usage: python3 -B research/s241/aim.py PID LOAD_SLOT BALL DIST SPEED SAVE_SLOT [SIDE] [SETTLE]

From biplane.p2m2's state (free06, the plane flown and at rest by its bay, the six EyeBalls asleep at
the collection addresses research/oracle.txt's S241 block gives): load LOAD_SLOT, pause, and move the
plane by the game's own put-back: its spawn transform (+0xe10 position, +0xe20 nose, +0xe30 up) and the
placement mxbiplane_reset reads first (the same three at +0xe0, which it then copies the spawn over) set
DIST ft short of ball BALL's body on the line from the plane to it (SIDE ft to its right), 3 ft above
the ball's centre, and its crash word (+0x3b0) set to 1 with the crash clock (+0x3bc) at WreckTime
(+0x3c0), so the next wreck step recovers it there (biplane_recover -> mxbiplane_reset: body, surfaces
and throttle reset, speed and spin zero). Unpause for SETTLE wall seconds (default 3) so it drops onto
its wheels, pause, and give its body SPEED ft/s along its nose (+0x70; the throttle stays shut, so it
coasts in). Save into SAVE_SLOT; the game is left paused. No pad: /dev/uinput is root-only here.

Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, on a copy started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
QT_ACCESSIBILITY=1.
"""

import math
import struct
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

PID, LOAD, BALL = (int(v) for v in sys.argv[1:4])
DIST, SPEED = float(sys.argv[4]), float(sys.argv[5])
SAVE = int(sys.argv[6])
SIDE = float(sys.argv[7]) if len(sys.argv) > 7 else 0.0
SETTLE = float(sys.argv[8]) if len(sys.argv) > 8 else 3.0
FRAME = cfg().symbols['frame_counter']
PLANE = 0x00c44a40
BODIES = [0xd3ce80, 0xd3d670, 0xd47e90, 0xd4a980, 0xd4d470, 0xd4ff60]

app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])
p = Pine()


def wait(s):
    t = time.time()
    while time.time() - t < s:
        time.sleep(0.01)


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


def pause(on):
    if paused() != on:
        A.press(item)
    wait(0.4)


def v3(a):
    return struct.unpack('<3f', p.read_block(a, 12))


def put(a, v):
    for k in range(3):
        p.write_f32(a + 4 * k, v[k])


def body():
    return p.read32(PLANE + 0x160)


def show(what):
    b = body()
    print('%-8s frame %d crash %d pos %9.3f %8.3f %9.3f vel %7.2f %7.2f %7.2f' % (
        (what, p.read32(FRAME), p.read32(PLANE + 0x3b0)) + v3(b + 0x60) + v3(b + 0x70)), flush=True)


p.load_state(LOAD)
wait(3.0)
pause(True)
show('loaded')
ball = v3(BODIES[BALL] + 0x60)
at = v3(body() + 0x60)
dx, dz = ball[0] - at[0], ball[2] - at[2]
n = math.hypot(dx, dz)
fx, fz = dx / n, dz / n
rx, rz = -fz, fx
spot = (ball[0] - DIST * fx + SIDE * rx, ball[1] + 3.0, ball[2] - DIST * fz + SIDE * rz)
print('ball', BALL, 'at', ['%.3f' % v for v in ball], 'spawn', ['%.3f' % v for v in spot], 'nose', '%.4f %.4f' % (fx, fz), flush=True)
for base in (0xe0, 0xe10):
    put(PLANE + base, spot)
    put(PLANE + base + 0x10, (fx, 0.0, fz))
    put(PLANE + base + 0x20, (0.0, 1.0, 0.0))
p.write32(PLANE + 0x3b0, 1)
p.write_f32(PLANE + 0x3bc, struct.unpack('<f', p.read_block(PLANE + 0x3c0, 4))[0])
pause(False)
wait(SETTLE)
pause(True)
show('settled')
b = body()
put(b + 0x70, (SPEED * fx, 0.0, SPEED * fz))
show('aimed')
p.save_state(SAVE)
wait(2.0)
print('saved', SAVE, flush=True)
