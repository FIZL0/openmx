"""R53: research/r48/aim.py for R53's two-player boot, whose plane and truck sit elsewhere in RAM
(the list's first entry and the car with vtable 0x0045edac at +0x88; research/r53/peek.py prints them).

usage: python3 -B research/r53/aim.py PID LOAD_SLOT DX DZ SAVE_SLOT [SETTLE]

R48's notes follow.

research/s252/aim.py's put-back for this boot (research/r48/boot.py, the human switched into the
plane): load LOAD_SLOT, pause, set the plane's spawn and placement (+0xe10/+0xe0: position, nose, up)
to the truck's body position + (DX, 6, DZ), its nose along the level line to the truck, and its crash
word (+0x3b0) to 1 with the crash clock at WreckTime, so the next wreck step puts it there. Unpause
SETTLE wall seconds (default 3) for it to drop onto its wheels, pause and save SAVE_SLOT.

Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, on a copy started with the accessibility bus on.
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
DX, DZ = float(sys.argv[3]), float(sys.argv[4])
SAVE = int(sys.argv[5])
SETTLE = float(sys.argv[6]) if len(sys.argv) > 6 else 3.0
FRAME = cfg().symbols['frame_counter']
PLANE = 0x00604740
TRUCK = 0x01339fd0

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
    t = p.read32(TRUCK + 0x160)
    print('%-8s frame %d crash %d plane %9.3f %8.3f %9.3f vel %7.2f %7.2f %7.2f truck %9.3f %8.3f %9.3f awake %d' % (
        (what, p.read32(FRAME), p.read32(PLANE + 0x3b0)) + v3(b + 0x60) + v3(b + 0x70) + v3(t + 0x60)
        + (p.read32(t),)), flush=True)


p.load_state(LOAD)
wait(3.0)
pause(True)
show('loaded')
at = v3(p.read32(TRUCK + 0x160) + 0x60)
spot = (at[0] + DX, at[1] + 6.0, at[2] + DZ)
n = (DX * DX + DZ * DZ) ** 0.5
fx, fz = -DX / n, -DZ / n
print('truck at', ['%.3f' % v for v in at], 'spawn', ['%.3f' % v for v in spot], 'nose', '%.4f %.4f' % (fx, fz), flush=True)
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
p.save_state(SAVE)
wait(2.0)
print('saved', SAVE, flush=True)
