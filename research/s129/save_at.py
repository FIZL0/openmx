"""S129: load a save-state slot, hold both brakes, let the game run until its frame counter reaches a set
frame, pause and save into another slot — a RAM image of the original part-way into a capture, whose
search records (the wheel probe's, the body rays') can be read against the port's at that tick.

usage: python3 -B research/s129/save_at.py PID LOAD_SLOT FRAME SAVE_SLOT

Run it under `./kit pcsx2 with --`, at a quarter speed (a frame is 133 ms, so the pause lands on FRAME or
the one after; the image's own frame counter says which).
"""

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

PID, LOAD, TARGET, SLOT = (int(v) for v in sys.argv[1:5])
FRAME = cfg().symbols['frame_counter']

app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


def wait(seconds):
    end = time.time() + seconds
    while time.time() < end:
        time.sleep(0.05)


p = Pine()
if not paused():
    A.press(item)
wait(0.5)
subprocess.run([str(ROOT / 'kit'), 'pad', 'send', 'down square', 'down r2'], check=False)
p.load_state(LOAD)
wait(2.0)
print('loaded', LOAD, 'frame', p.read32(FRAME), 'paused', paused(), flush=True)
if paused():
    A.press(item)
while p.read32(FRAME) < TARGET:
    time.sleep(0.005)
A.press(item)
wait(0.5)
print('paused at frame', p.read32(FRAME), flush=True)
p.save_state(SLOT)
wait(2.0)
print('saved', SLOT, flush=True)
