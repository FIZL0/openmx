"""B34b: put one free06 EyeBall beside the ball the plane is about to strike, and save the state.

usage: python3 -B research/b34b/put_ball.py PID LOAD_SLOT BALL ONTO GAP SAVE_SLOT [ACROSS] [own] [SPEED]

research/s242/put_ball.py's, for free06 (research/oracle.txt's S241 block): load LOAD_SLOT (a state of
research/s241/aim.py's run, the plane rolling at ball ONTO), pause, and move ball BALL so its body stands
GAP ft past ball ONTO's on the line from the plane to ONTO, ACROSS ft to its right (default 0), at ONTO's
height (with `own`, on the line from ONTO to BALL's own place instead: it keeps its side), by shifting every copy of its position - in its body, its node and its collision object, every
float triple within 12 ft of the body's position - by one delta; its velocity (+0x70) and rates (+0x1b0)
zeroed, and ONTO's;
with SPEED, BALL is then given SPEED ft/s toward ONTO. Paused before the load, so the state's frame is the one it holds; save into SAVE_SLOT; the game is left paused.

Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, on a copy started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
QT_ACCESSIBILITY=1.
"""

import math
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

PID, LOAD, BALL, ONTO = (int(v) for v in sys.argv[1:5])
GAP = float(sys.argv[5])
SAVE = int(sys.argv[6])
ACROSS = float(sys.argv[7]) if len(sys.argv) > 7 else 0.0
FRAME = cfg().symbols['frame_counter']
OBJECTS = [0xd3f510, 0xd477a0, 0xd4a290, 0xd4cd80, 0xd4f870, 0xd52360]
PLANE = 0x00c44a40

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


pause(True)
p.load_state(LOAD)
wait(3.0)
pause(True)
o = OBJECTS[BALL]
body, record, node = p.read32(o + 0x110), p.read32(o + 0x120), p.read32(o + 0x13c)
at = v3(body + 0x60)
onto = v3(p.read32(OBJECTS[ONTO] + 0x110) + 0x60)
plane = v3(p.read32(PLANE + 0x160) + 0x60)
own = len(sys.argv) > 8 and sys.argv[8] == 'own'
dx, dz = (at[0] - onto[0], at[2] - onto[2]) if own else (onto[0] - plane[0], onto[2] - plane[2])
n = math.hypot(dx, dz)
fx, fz = dx / n, dz / n
X, Y, Z = onto[0] + GAP * fx - ACROSS * fz, onto[1], onto[2] + GAP * fz + ACROSS * fx
d = (X - at[0], Y - at[1], Z - at[2])
print('frame', p.read32(FRAME), 'plane at', ['%.3f' % v for v in plane], 'ball', BALL, 'at', ['%.3f' % v for v in at],
      'beside ball', ONTO, 'at', ['%.3f' % v for v in onto], 'moving it by', ['%.3f' % v for v in d], flush=True)
moved = 0
for name, base, size in [('body', body, 0x200), ('node', node, 0xc0), ('record', record, 0x200)]:
    b = p.read_block(base, size)
    off = 0
    while off + 12 <= size:
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
for b in (body, p.read32(OBJECTS[ONTO] + 0x110)):
    for off in (0x70, 0x1b0):
        for k in range(3):
            p.write_f32(b + off + 4 * k, 0.0)
if len(sys.argv) > 9:
    speed = float(sys.argv[9])
    gx, gz = onto[0] - X, onto[2] - Z
    g = math.hypot(gx, gz)
    p.write_f32(body + 0x70, speed * gx / g)
    p.write_f32(body + 0x78, speed * gz / g)
p.save_state(SAVE)
wait(2.0)
print('saved', SAVE, flush=True)
