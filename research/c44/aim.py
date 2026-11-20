"""C44: park the biplane on free06's ground in an AI rider's way, and save the state.

usage: python3 -B research/c44/aim.py PID LOAD_SLOT RIDER X,Y,Z SAVE_SLOT [SETTLE]

research/s252/aim.py's put-back, aimed at a point an AI rider's bike passes (a c44-plane-bike scout
capture shows its path) instead of a truck's. From biplane.p2m2's state: load LOAD_SLOT, pause, set
the plane's spawn and placement (+0xe10/+0xe0: position, nose, up) to X,Y,Z, its nose towards the
rider's bike (RIDER: the rider's address, its chassis at +0x7a4), and its crash word (+0x3b0) to 1
with the crash clock at WreckTime, so the next wreck step puts it there. Unpause SETTLE wall seconds
(default 3) for it to drop onto its wheels, pause and save into SAVE_SLOT; the game is left paused.

Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, on a copy started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
QT_ACCESSIBILITY=1.
"""

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

PID, LOAD = int(sys.argv[1]), int(sys.argv[2])
RIDER = int(sys.argv[3], 0)
SPOT = tuple(float(v) for v in sys.argv[4].split(','))
SAVE = int(sys.argv[5])
SETTLE = float(sys.argv[6]) if len(sys.argv) > 6 else 3.0
FRAME = cfg().symbols['frame_counter']
PLANE = 0x00c44a40

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
    c = p.read32(RIDER + 0x7a4)
    print('%-8s frame %d crash %d plane %9.3f %8.3f %9.3f vel %7.2f %7.2f %7.2f bike %9.3f %8.3f %9.3f' % (
        (what, p.read32(FRAME), p.read32(PLANE + 0x3b0)) + v3(b + 0x60) + v3(b + 0x70) + v3(c + 0x60)), flush=True)


p.load_state(LOAD)
wait(3.0)
pause(True)
show('loaded')
at = v3(p.read32(RIDER + 0x7a4) + 0x60)
fx, fz = SPOT[0] - at[0], SPOT[2] - at[2]
n = (fx * fx + fz * fz) ** 0.5
fx, fz = fx / n, fz / n
print('bike at', ['%.3f' % v for v in at], 'heading', '%.4f %.4f' % (fx, fz),
      'spawn', ['%.3f' % v for v in SPOT], flush=True)
for base in (0xe0, 0xe10):
    put(PLANE + base, SPOT)
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
