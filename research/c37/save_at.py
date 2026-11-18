"""C37: load SLOT on a running copy, let the game run to FRAME, pause, print the bike's pitch and
vertical speed, save into SAVE. Run under `./kit pcsx2 with --` (QT_ACCESSIBILITY=1).
usage: python3 -B research/c37/save_at.py PID SLOT FRAME SAVE"""
import math, struct, sys, time
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'research/journal/2026-10-20-c6-c6-recording2-capture/playback'))
from ps2kit.config import cfg            # noqa: E402
from ps2kit.pine import Pine             # noqa: E402
import atspi_lib as A                    # noqa: E402
from gi.repository import Atspi          # noqa: E402

PID, SLOT, FRAME, SAVE = (int(a) for a in sys.argv[1:5])
item = A.menu_path(A.app_of(PID), ['System', 'Pause'])
p = Pine()
FC = cfg().symbols['frame_counter']


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


def v3(a):
    return struct.unpack('<3f', p.read_block(a, 12))


if paused():
    A.press(item)
    time.sleep(0.5)
start = p.read32(FC)
p.load_state(SLOT)
end = time.time() + 30
while p.read32(FC) >= start and time.time() < end:   # the load lands
    pass
while p.read32(FC) < FRAME and time.time() < end:
    pass
A.press(item)
time.sleep(1.0)
rider = p.read32(cfg().symbols['game_manager'])
for off in (0x34, 0x34, 0x8, 0x8, 0x0):
    rider = p.read32(rider + off)
ch = p.read32(rider + 0x7a4)
fwd = v3(p.read32(ch + 0x254) + 0xa0)
print('paused at', p.read32(FC), 'pitch %.1f deg' % math.degrees(math.asin(fwd[1])), 'vel', v3(ch + 0x260), flush=True)
p.save_state(SAVE)
time.sleep(2.0)
print('saved', SAVE, flush=True)
