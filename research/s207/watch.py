"""S207: a quick look at a parked state: load SLOT, run SECONDS wall seconds and print, every tenth of a
second, the frame, the car's place, the bike's place and velocity and the wreck reason (rider +0x748).

usage: python3 -B research/s207/watch.py PID SLOT CAR SECONDS

Run it under `./kit pcsx2 with --`. The game is left paused."""

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

PID, SLOT, CAR, SECONDS = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3], 16), float(sys.argv[4])
SYM = cfg().symbols
FRAME, MANAGER = SYM['frame_counter'], SYM['game_manager']
app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])
p = Pine()


def wait(s):
    t = time.time()
    while time.time() - t < s:
        time.sleep(0.01)


def pause(on):
    if item.get_state_set().contains(Atspi.StateType.CHECKED) != on:
        A.press(item)
    wait(0.4)


def ok(a):
    return 0x100000 <= a < 0x2000000


def human(kind):
    node = p.read32(p.read32(p.read32(MANAGER) + 0x34) + 0x34)
    for _ in range(40):
        it = p.read32(node)
        if ok(it) and p.read32(it) == kind:
            pl = p.read32(it + 0x20)
            if ok(pl) and p.read32(pl + 0x34) == 1:
                return it
        node = p.read32(node + 0x8)
    raise SystemExit('no human actor of type %d' % kind)


def v3(a):
    return struct.unpack('<3f', p.read_block(a, 12))


p.load_state(SLOT)
wait(3.0)
pause(True)
veh, rider = human(4), human(2)
ch = p.read32(rider + 0x7a4)
pause(False)
end = time.time() + SECONDS
while time.time() < end:
    print('%6d car %s bike %s vel %s reason %d' % (
        p.read32(FRAME), ' '.join('%7.1f' % v for v in v3(p.read32(CAR + 0x254) + 0xb0)),
        ' '.join('%8.2f' % v for v in v3(veh + 0x1b0)), ' '.join('%6.2f' % v for v in v3(ch + 0x260)),
        p.read32(rider + 0x748)), flush=True)
    wait(0.1)
pause(True)
