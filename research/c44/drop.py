"""C44: drop the biplane right in front of a riding AI bike, too late for it to steer round, and save.

usage: python3 -B research/c44/drop.py PID LOAD_SLOT RIDER FRAME LEAD SAVE_SLOT [UP] [SIDE]

research/c44/aim.py parks the plane on a point an AI bike passes, but free09's and free06's AI
riders steer round it (try1: a1 passed 16 ft off the plane's body). So: load LOAD_SLOT (running),
wait for frame_counter FRAME, pause, and put the plane (spawn and placement, crash word 1 with the
clock at WreckTime, as aim.py) LEAD seconds ahead of RIDER's bike along its velocity, UP ft (default
3) above its chassis centre and SIDE ft to its right (default 0), nose towards the bike. Unpause until
the put-back has run (the crash word back to 0), pause and save into SAVE_SLOT; the game is left
paused.

Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, on a copy started with the two Qt accessibility
variables.
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
AT_FRAME = int(sys.argv[4])
LEAD = float(sys.argv[5])
SAVE = int(sys.argv[6])
UP = float(sys.argv[7]) if len(sys.argv) > 7 else 3.0
SIDE = float(sys.argv[8]) if len(sys.argv) > 8 else 0.0
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


pause(False)
p.load_state(LOAD)
deadline = time.time() + 120
# The counter still reads the old run's frame until the load lands.
while p.read32(FRAME) >= AT_FRAME - 10:
    if time.time() > deadline:
        raise SystemExit('the load never landed')
    time.sleep(0.005)
while p.read32(FRAME) < AT_FRAME - 1:
    if time.time() > deadline:
        raise SystemExit('frame %d never came' % AT_FRAME)
    time.sleep(0.005)
pause(True)
show('paused')
body = p.read32(RIDER + 0x7a4)
at = v3(body + 0x60)
v = v3(body + 0x260)
n = (v[0] ** 2 + v[2] ** 2) ** 0.5
fx, fz = v[0] / n, v[2] / n
spot = (at[0] + LEAD * v[0] - SIDE * fz, at[1] + UP, at[2] + LEAD * v[2] + SIDE * fx)
print('bike at', ['%.3f' % c for c in at], 'speed %.2f heading %.4f %.4f' % (n, fx, fz),
      'spawn', ['%.3f' % c for c in spot], flush=True)
for base in (0xe0, 0xe10):
    put(PLANE + base, spot)
    put(PLANE + base + 0x10, (-fx, 0.0, -fz))
    put(PLANE + base + 0x20, (0.0, 1.0, 0.0))
p.write32(PLANE + 0x3b0, 1)
p.write_f32(PLANE + 0x3bc, struct.unpack('<f', p.read_block(PLANE + 0x3c0, 4))[0])
f0 = p.read32(FRAME)
pause(False)
deadline = time.time() + 20
while p.read32(PLANE + 0x3b0) != 0 and time.time() < deadline:
    time.sleep(0.002)
pause(True)
show('put')
print('put-back after', p.read32(FRAME) - f0, 'ticks', flush=True)
p.save_state(SAVE)
wait(2.0)
print('saved', SAVE, flush=True)
