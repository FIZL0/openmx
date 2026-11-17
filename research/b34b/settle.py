"""B34b: load a free06 save state, let it run, and print every EyeBall's place and whether it is awake.

usage: python3 -B research/b34b/settle.py PID LOAD_SLOT SECONDS [SAVE_SLOT]

Prints the frame, then each ball's body position and enabled word (research/oracle.txt's S241 block),
once a wall second for SECONDS; with SAVE_SLOT, pauses at the end and saves there. Run it under
`KIT_PCSX2=N ./kit pcsx2 with --`, on a copy started with the two Qt accessibility variables."""

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

PID, LOAD, SECONDS = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3])
SAVE = int(sys.argv[4]) if len(sys.argv) > 4 else None
FRAME = cfg().symbols['frame_counter']
BODIES = [0xd3ce80, 0xd3d670, 0xd47e90, 0xd4a980, 0xd4d470, 0xd4ff60]
PLANE = 0x00c44a40

item = A.menu_path(A.app_of(PID), ['System', 'Pause'])
p = Pine()


def pause(on):
    if item.get_state_set().contains(Atspi.StateType.CHECKED) != on:
        A.press(item)
    t = time.time()
    while time.time() - t < 0.4:
        time.sleep(0.01)


pause(True)
p.load_state(LOAD)
pause(False)
end = time.time() + SECONDS
while time.time() < end:
    t = time.time()
    while time.time() - t < 1.0:
        time.sleep(0.05)
    row = ['%d%s(%.1f,%.1f,%.1f)' % (k, '*' if p.read32(b) else ' ', *struct.unpack('<3f', p.read_block(b + 0x60, 12)))
           for k, b in enumerate(BODIES)]
    pl = struct.unpack('<3f', p.read_block(p.read32(PLANE + 0x160) + 0x60, 12))
    print(p.read32(FRAME), 'plane (%.1f,%.1f,%.1f)' % pl, ' '.join(row), flush=True)
if SAVE is not None:
    pause(True)
    p.save_state(SAVE)
    time.sleep(2.0)
    print('saved', SAVE, flush=True)
