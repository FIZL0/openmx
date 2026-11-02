"""S195: move a knocked beach ball to just inside the map's edge, and save the state.

usage: python3 -B research/s195/edge.py PID LOAD_SLOT BALL X SAVE_SLOT

No prop on free09 can be knocked off the map (the terrain runs 0..6912 ft on x and z, and the
world-ends signs stop the bike at about 1160 and 5750), so the edge reset is reached with a poke: load
LOAD_SLOT (a state research/s195/knock.py saved after a strike, the ball flying), pause, and move ball
BALL (0..4, the five BeachBall collections in the list's order) to x = X by shifting every copy of its
position — in its body (+0x50 last, +0x60 position), its node and its collision object (the centre of
mass, the swept segment), every float triple within 12 ft of the body's position — by one delta, the
way research/s76_tele.py moves the bike. Its velocity is left as the strike made it. Pause and save into
SAVE_SLOT; the game is left paused.

Run it under `./kit pcsx2 with --`, on a copy started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
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

PID, LOAD, BALL = (int(v) for v in sys.argv[1:4])
X = float(sys.argv[4])
SAVE = int(sys.argv[5])
FRAME = cfg().symbols['frame_counter']
OBJECTS = [0x11b6f90, 0x11bed70, 0x11c1860, 0x11c4350, 0x11c6e40]

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


def v3(a):
    return struct.unpack('<3f', p.read_block(a, 12))


p.load_state(LOAD)
wait(3.0)
pause(True)
o = OBJECTS[BALL]
body, record, node = p.read32(o + 0x110), p.read32(o + 0x120), p.read32(o + 0x13c)
at = v3(body + 0x60)
dx = X - at[0]
print('frame', p.read32(FRAME), 'ball', BALL, 'at', ['%.3f' % v for v in at], 'vel', ['%.3f' % v for v in v3(body + 0x70)],
      'moving it %.3f ft on x' % dx, flush=True)
moved = 0
for name, base, n in [('body', body, 0x200), ('node', node, 0xc0), ('record', record, 0x200)]:
    b = p.read_block(base, n)
    off = 0
    while off + 12 <= n:
        v = struct.unpack_from('<3f', b, off)
        if all(abs(v[k] - at[k]) < 12 for k in range(3)):
            p.write_f32(base + off, v[0] + dx)
            print('  %s +0x%x %s' % (name, off, ['%.3f' % c for c in v]), flush=True)
            moved += 1
            off += 12
        else:
            off += 4
print('moved', moved, 'triples', flush=True)
p.save_state(SAVE)
wait(2.0)
print('saved', SAVE, flush=True)
