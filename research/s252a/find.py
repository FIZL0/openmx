"""S252a: load a slot, run SECONDS wall seconds, pause, and list every 16-byte-aligned body-like triple (a
position at +0x60 with w 1.0 and a velocity at +0x70) within NEAR ft of X Y Z, to find the wreck bike's body.

usage: python3 -B research/s252a/find.py PID SLOT SECONDS X Y Z [NEAR]
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

PID, SLOT = int(sys.argv[1]), int(sys.argv[2])
SECONDS = float(sys.argv[3])
X, Y, Z = (float(v) for v in sys.argv[4:7])
NEAR = float(sys.argv[7]) if len(sys.argv) > 7 else 20.0
item = A.menu_path(A.app_of(PID), ['System', 'Pause'])
p = Pine()


def wait(s):
    t = time.time()
    while time.time() - t < s:
        time.sleep(0.01)


def pause(on):
    if item.get_state_set().contains(Atspi.StateType.CHECKED) != on:
        A.press(item)
    wait(0.4)


p.load_state(SLOT)
wait(3.0)
pause(False)
wait(SECONDS)
pause(True)
print('frame', p.read32(cfg().symbols['frame_counter']), flush=True)
lo, hi = 0x00100000, 0x02000000
step = 0x100000
for base in range(lo, hi, step):
    b = p.read_block(base, step + 0x80)
    for off in range(0, step, 16):
        x, y, z, w = struct.unpack_from('<4f', b, off + 0x60)
        if w == 1.0 and abs(x - X) < NEAR and abs(y - Y) < NEAR and abs(z - Z) < NEAR:
            vel = struct.unpack_from('<4f', b, off + 0x70)
            print('%08x vt %08x pos %.2f %.2f %.2f vel %.2f %.2f %.2f %.2f' % (
                (base + off, struct.unpack_from('<I', b, off + 8)[0], x, y, z) + vel), flush=True)
