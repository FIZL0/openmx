"""S252: park the biplane on free06's ground in a HasPhysics truck's way, and save the state.

usage: python3 -B research/s252/aim.py PID LOAD_SLOT TRUCK DIST SAVE_SLOT [SIDE] [SETTLE]

research/s241/aim.py's put-back, aimed at a truck instead of a ball. From biplane.p2m2's state (free06,
the plane at rest by its bay; the two HasPhysics trucks at TRUCKS below, research/oracle.txt S186):
load LOAD_SLOT, pause, and set the plane's spawn and placement (+0xe10/+0xe0: position, nose, up) DIST
ft ahead of truck TRUCK (0 or 1) along its heading (car +0x480), SIDE ft to its right, 6 ft above the
truck's position (car +0x470), nose towards the truck, and its crash word (+0x3b0) to 1 with the crash
clock at WreckTime, so the next wreck step puts it there. Unpause SETTLE wall seconds (default 3) for it
to drop onto its wheels, pause and save into SAVE_SLOT; the game is left paused. The truck drives on
into the parked plane.

With AT=x,y,z in the environment the plane goes there instead (y as given, nose towards the truck):
a point a capture shows the truck pass, since its path bends away from its heading.

Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, on a copy started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
QT_ACCESSIBILITY=1.
"""

import os
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

PID, LOAD, TRUCK = (int(v) for v in sys.argv[1:4])
DIST = float(sys.argv[4])
SAVE = int(sys.argv[5])
SIDE = float(sys.argv[6]) if len(sys.argv) > 6 else 0.0
SETTLE = float(sys.argv[7]) if len(sys.argv) > 7 else 3.0
FRAME = cfg().symbols['frame_counter']
PLANE = 0x00c44a40
TRUCKS = [0x01cc4440, 0x01cc7630]

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


def show(what):
    b = p.read32(PLANE + 0x160)
    t = TRUCKS[TRUCK]
    print('%-8s frame %d crash %d plane %9.3f %8.3f %9.3f vel %7.2f %7.2f %7.2f truck %9.3f %8.3f %9.3f physical %d' % (
        (what, p.read32(FRAME), p.read32(PLANE + 0x3b0)) + v3(b + 0x60) + v3(b + 0x70) + v3(t + 0x470)
        + (p.read32(t + 0x588),)), flush=True)


p.load_state(LOAD)
wait(3.0)
pause(True)
show('loaded')
t = TRUCKS[TRUCK]
at = v3(t + 0x470)
h = v3(t + 0x480)
n = (h[0] ** 2 + h[2] ** 2) ** 0.5
fx, fz = h[0] / n, h[2] / n
rx, rz = -fz, fx
spot = (at[0] + DIST * fx + SIDE * rx, at[1] + 6.0, at[2] + DIST * fz + SIDE * rz)
if os.environ.get('AT'):
    spot = tuple(float(v) for v in os.environ['AT'].split(','))
    fx, fz = spot[0] - at[0], spot[2] - at[2]
    n = (fx * fx + fz * fz) ** 0.5
    fx, fz = fx / n, fz / n
print('truck', TRUCK, 'at', ['%.3f' % v for v in at], 'heading', '%.4f %.4f' % (fx, fz),
      'spawn', ['%.3f' % v for v in spot], flush=True)
for base in (0xe0, 0xe10):
    put(PLANE + base, spot)
    put(PLANE + base + 0x10, (-fx, 0.0, -fz))
    put(PLANE + base + 0x20, (0.0, 1.0, 0.0))
p.write32(PLANE + 0x3b0, 1)
p.write_f32(PLANE + 0x3bc, struct.unpack('<f', p.read_block(PLANE + 0x3c0, 4))[0])
pause(False)
wait(SETTLE)
pause(True)
show('settled')
p.save_state(SAVE)
wait(2.0)
print('saved', SAVE, flush=True)
