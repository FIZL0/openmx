"""S170: log the Dallas train's engine round one whole loop of its path in the original: one line every
STEP frames, the frame, the path clock and the engine's position, from LOAD_SLOT until the clock comes back
round past where it started.

usage: python3 -B research/s170/loop.py PID LOAD_SLOT STEP

Run it under `./kit pcsx2 with --`.
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

PID, LOAD, STEP = (int(v) for v in sys.argv[1:4])
ENGINE = 0x00d525f0
FRAME = cfg().symbols['frame_counter']
app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])
p = Pine()


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


def wait(seconds):
    end = time.time() + seconds
    while time.time() < end:
        time.sleep(0.05)


def clock():
    return struct.unpack('<f', p.read_block(p.read32(ENGINE + 0x430) + 0xc, 4))[0]


p.load_state(LOAD)
wait(2.0)
if paused():
    A.press(item)
c0 = clock()
wrapped = False
last = p.read32(FRAME)
while True:
    f = p.read32(FRAME)
    if f - last < STEP:
        time.sleep(0.01)
        continue
    last = f
    c = clock()
    e = struct.unpack('<3f', p.read_block(p.read32(ENGINE + 0x254) + 0xb0, 12))
    print('%d %.4f %.2f %.2f %.2f' % ((f, c) + e), flush=True)
    if c < c0 - 1:
        wrapped = True
    if wrapped and c > c0:
        break
A.press(item)
