"""S191: load a state, hold both brakes, let the game run to a frame and save there.

usage: python3 -B research/s191/run_to.py PID LOAD_SLOT FRAME SAVE_SLOT

S170's head-on state (copy 2's slot 16) stands the bike on the rails 2,350 frames before the engine's nose
arrives: too long a capture. This runs it on to FRAME (brakes held, as the state was saved) and saves
there, so a capture from SAVE_SLOT starts a little before the nose. The brakes go through `./kit pad send`;
with no pad server (S191's day: /dev/uinput was root's) the send fails, the run goes on with nothing
held, and the bike rolls about a foot in 2,300 frames. Run it under `./kit pcsx2 with --`.
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
if paused():
    A.press(item)
f0 = p.read32(FRAME)
print('loaded', LOAD, 'frame', f0, flush=True)
assert f0 < TARGET, 'the state is past the frame'
deadline = time.time() + 1800
while p.read32(FRAME) < TARGET - 1:
    assert time.time() < deadline, 'the game stopped'
    time.sleep(0.002)
A.press(item)
wait(0.5)
f1 = p.read32(FRAME)
wait(0.3)
assert p.read32(FRAME) == f1, 'not paused'
print('paused at frame', f1, flush=True)
p.save_state(SLOT)
wait(2.0)
print('saved', SLOT, flush=True)
