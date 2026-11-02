"""S197b: drop the human's free09 bike hard beside a beach ball, so it wrecks and its wreck bike strikes the ball.

usage: python3 -B research/s197b/drop.py PID LOAD_SLOT BALL DIST HEIGHT SPEED FALL SAVE_SLOT [SIDE]

From first_recording's state (free09, every ball asleep): load LOAD_SLOT, pause, and move the bike
(research/s195/aim.py's teleport) DIST ft short of collection BALL's body along the bike's own heading
and SIDE ft to its right (default 0), HEIGHT ft above the ball's centre, its chassis given SPEED ft/s along its heading and FALL ft/s down
(the world velocity +0x260 and the vehicle's +0x130). It lands too hard (a Flat wreck), and the wreck
bike carries on at the ball. Pause and save into SAVE_SLOT; the game is left paused. No pad.

Run it under `./kit pcsx2 with --`, on a copy started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
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

PID, LOAD, BALL = (int(v) for v in sys.argv[1:4])
DIST, HEIGHT, SPEED, FALL = (float(v) for v in sys.argv[4:8])
SAVE = int(sys.argv[8])
SIDE = float(sys.argv[9]) if len(sys.argv) > 9 else 0.0
SYM = cfg().symbols
FRAME, MANAGER = SYM['frame_counter'], SYM['game_manager']
CHASSIS = 0x7a4
LIST = 0x011b43f4

app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])
p = Pine()


def wait(s):
    t = time.time()
    while time.time() - t < s:
        time.sleep(0.01)


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


def pause(on):
    if paused() != on:
        A.press(item)
    wait(0.4)


def ok(a):
    return 0x100000 <= a < 0x2000000


def human(kind):
    """The human player's actor of `kind` (2 the rider, 4 the vehicle) in the manager's list."""
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


def put(a, v):
    for k in range(3):
        p.write_f32(a + 4 * k, v[k])


def balls():
    """Every collection's name and its first body's address."""
    out = []
    cell = p.read32(LIST + 4)
    while cell:
        c = p.read32(cell)
        o = p.read32(p.read32(c + 0x328))
        name = p.read_block(c + 0x144, 24).split(b'\0')[0].decode('latin1')
        out.append((name, p.read32(o + 0x110)))
        cell = p.read32(cell + 8)
    return out


def teleport(x, y, z):
    vp = v3(veh + 0x1b0)
    d = (x - vp[0], y - vp[1], z - vp[2])
    regions = [(rider, 0x2000), (ch, 0x400), (veh, 0x800), (p.read32(rider + 0x18) - 0x1600, 0x1800)]
    moved = 0
    for base, n in regions:
        b = p.read_block(base, n)
        off = 0
        while off + 12 <= n:
            v = struct.unpack_from('<3f', b, off)
            if all(abs(v[k] - vp[k]) < 12 for k in range(3)):
                put(base + off, [v[k] + d[k] for k in range(3)])
                moved += 1
                off += 12
            else:
                off += 4
    for o in (0x260, 0x280, 0x2b8, 0x2d0, 0x310):
        put(ch + o, (0.0, 0.0, 0.0))
    put(veh + 0x130, (0.0, 0.0, 0.0))
    print('moved', moved, 'triples by', ['%.2f' % v for v in d], flush=True)


def forward():
    f = v3(node + 0xa0)
    n = math.hypot(f[0], f[2])
    return f[0] / n, f[2] / n


def show(what):
    print('%-8s frame %d pos %9.2f %8.2f %9.2f fwd %6.3f %6.3f vel %7.2f %7.2f %7.2f' % (
        (what, p.read32(FRAME)) + v3(veh + 0x1b0) + forward() + v3(ch + 0x260)), flush=True)


p.load_state(LOAD)
wait(3.0)
pause(True)
veh, rider = human(4), human(2)
ch = p.read32(rider + CHASSIS)
node = p.read32(ch + 0x254)
print('rider', hex(rider), 'vehicle', hex(veh), 'chassis', hex(ch), flush=True)
every = balls()
name, body = every[BALL]
ball = v3(body + 0x60)
print('dropping beside', BALL, name, 'body', hex(body), 'at', ['%.2f' % v for v in ball], flush=True)
fx, fz = forward()
rx, rz = -fz, fx                                    # the bike's right, on the ground plane
teleport(ball[0] - DIST * fx + SIDE * rx, ball[1] + HEIGHT, ball[2] - DIST * fz + SIDE * rz)
put(ch + 0x260, (SPEED * fx, -FALL, SPEED * fz))
put(veh + 0x130, (SPEED * fx, -FALL, SPEED * fz))
show('dropped')
p.save_state(SAVE)
wait(2.0)
print('saved', SAVE, flush=True)
