"""S244: throw an awake beach ball at the wreck bike, and save the state.

usage: python3 -B research/s244/throw_ball.py PID FROM BALL AHEAD UP SPEED SAVE_SLOT

research/s197b/put_ball.py's move from the save state context/FROM (a context-relative name without its
extension, e.g. s244/hit3-put; copied into slot 1 first): ball BALL (awake in it) put AHEAD ft in front
of the wreck bike's body along its velocity on the ground plane (behind it for a negative AHEAD), UP ft
above it, its rates zeroed — but its velocity set to SPEED ft/s back at the wreck bike on the ground
plane (a negative SPEED throws it along the wreck bike's way, to catch it up), so the two close faster
than a hard contact's 10 ft/s and the ball's own entries on the wreck bike take an impulse. Saves into
SAVE_SLOT; the game is left paused.

Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, on a copy started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
QT_ACCESSIBILITY=1.
"""

import shutil
import struct
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'research/journal/2026-10-20-c6-c6-recording2-capture/playback'))
from ps2kit import emu                   # noqa: E402
from ps2kit.config import cfg            # noqa: E402
from ps2kit.pine import Pine             # noqa: E402
import atspi_lib as A                    # noqa: E402
from gi.repository import Atspi          # noqa: E402

PID, FROM, BALL = int(sys.argv[1]), sys.argv[2], int(sys.argv[3])
AHEAD, UP, SPEED = (float(v) for v in sys.argv[4:7])
SAVE = int(sys.argv[7])
LOAD = 1
shutil.copyfile(ROOT / 'context' / (FROM + '.p' + '2s'), emu.state_path(str(LOAD)))
WRECK_BODY = 0x00dc8980
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
wb, wv = v3(WRECK_BODY + 0x60), v3(WRECK_BODY + 0x70)
n = (wv[0] ** 2 + wv[2] ** 2) ** 0.5
X, Y, Z = wb[0] + AHEAD * wv[0] / n, wb[1] + UP, wb[2] + AHEAD * wv[2] / n
print('wreck bike at', ['%.3f' % v for v in wb], 'vel', ['%.3f' % v for v in wv], flush=True)
d = (X - at[0], Y - at[1], Z - at[2])
print('frame', p.read32(FRAME), 'ball', BALL, 'at', ['%.3f' % v for v in at], 'vel', ['%.3f' % v for v in v3(body + 0x70)],
      'moving it by', ['%.3f' % v for v in d], flush=True)
moved = 0
for name, base, n in [('body', body, 0x200), ('node', node, 0xc0), ('record', record, 0x200)]:
    b = p.read_block(base, n)
    off = 0
    while off + 12 <= n:
        v = struct.unpack_from('<3f', b, off)
        if all(abs(v[k] - at[k]) < 12 for k in range(3)):
            for k in range(3):
                p.write_f32(base + off + 4 * k, v[k] + d[k])
            print('  %s +0x%x %s' % (name, off, ['%.3f' % c for c in v]), flush=True)
            moved += 1
            off += 12
        else:
            off += 4
print('moved', moved, 'triples', flush=True)
for k in range(3):
    p.write_f32(body + 0x1b0 + 4 * k, 0.0)
g = (wv[0] ** 2 + wv[2] ** 2) ** 0.5
throw = (-SPEED * wv[0] / g, 0.0, -SPEED * wv[2] / g)
for k in range(3):
    p.write_f32(body + 0x70 + 4 * k, throw[k])
print('thrown at', ['%.3f' % v for v in throw], flush=True)
p.save_state(SAVE)
wait(2.0)
print('saved', SAVE, flush=True)
