"""S134: put the human's bike in an AI rider's path, so the rider meets it head on and wrecks into it.

usage: python3 -B research/s134_tele.py PID LEAD AHEAD SLOT [bike K] [speed S | match] [up H] [at X Y Z]

S76's tele.py (research/s76_tele.py), aimed at a bike: pause the game over AT-SPI (PCSX2 `PID`'s
System > Pause), read the three AI riders' bikes off their collidables (fixed in recording2's session,
category 0x9a048; research/oracle.txt S134), take bike `K` (0..2) or the one least in the air, and move the human's
bike — every copy of its position near the rider, the chassis, the vehicle and the rider's pool — to
where that bike will be in `LEAD` seconds at its present velocity (half a foot up), then `AHEAD` feet back along the
human's own heading, so riding straight on meets it. With `at X Y Z` the bike goes there instead
(the AI riders' routes are the same every run from one state, so a fixed spot on one and a timed
start in the pad script meet it better than an aim from wherever the pause fell). The chassis' velocities are zeroed — or, with `speed S`, its velocity is S ft/s
straight on (the AI riders brake for a bike standing in their way, so the human is launched at them
from `AHEAD` = LEAD × S back), or with `match` the AI bike's own velocity, so a human set `up H`
feet above its line (half a foot by default) comes down on it — and the
state is saved into `SLOT` while still paused (there is no settling: the AI bike would ride on).

Run it under `./kit pcsx2 with --`, PCSX2 started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
QT_ACCESSIBILITY=1; the game is left paused.
"""

import struct
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'research/journal/2026-10-20-c6-c6-recording2-capture/playback'))
from ps2kit.config import cfg            # noqa: E402
from ps2kit.pine import Pine             # noqa: E402
import atspi_lib as A                    # noqa: E402
from gi.repository import Atspi          # noqa: E402

PID = int(sys.argv[1])
LEAD = float(sys.argv[2])
AHEAD = float(sys.argv[3])
SLOT = int(sys.argv[4])
OPTS = sys.argv[5:]
AT = PICK = None
SPEED = 0.0
UP = 0.5
MATCH = False
while OPTS:
    word = OPTS.pop(0)
    if word == 'at':
        AT = tuple(float(OPTS.pop(0)) for _ in range(3))
    elif word == 'bike':
        PICK = int(OPTS.pop(0))
    elif word == 'speed':
        SPEED = float(OPTS.pop(0))
    elif word == 'up':
        UP = float(OPTS.pop(0))
    elif word == 'match':
        MATCH = True
    else:
        raise SystemExit('unknown option %r' % word)
SYM = cfg().symbols
FRAME = SYM['frame_counter']
MANAGER = SYM['game_manager']
# The AI riders' bike collidables in recording2's session: +0x7c the chassis, +0x120 its world transform.
BIKES = (0x1034c00, 0xc89cf0, 0x15ab1b0)

app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


p = Pine()


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


def wait(seconds):
    end = time.time() + seconds
    while time.time() < end:
        time.sleep(0.05)


if not paused():
    A.press(item)
wait(0.5)
f0 = p.read32(FRAME)
wait(0.3)
assert p.read32(FRAME) == f0, 'not paused'
veh = human(4)
rider = human(2)
ch = p.read32(rider + 0x7a4)
vp = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
xf = struct.unpack('<16f', p.read_block(veh + 0x180, 64))
print('frame', f0, 'veh', hex(veh), 'pos', vp, 'xform rows', [xf[4 * r:4 * r + 3] for r in range(4)], flush=True)
fwd = xf[8:11]
flat = (fwd[0], 0.0, fwd[2])
n = (flat[0] ** 2 + flat[2] ** 2) ** 0.5
fwd = (flat[0] / n, 0.0, flat[2] / n)
best = None
for k, c in enumerate(BIKES):
    assert p.read32(c + 0x84) == 0x9a048, 'not a bike collidable: %x' % c
    at = struct.unpack('<3f', p.read_block(c + 0x150, 12))
    v = struct.unpack('<3f', p.read_block(p.read32(c + 0x7c) + 0x260, 12))
    d = sum((at[i] - vp[i]) ** 2 for i in range(3)) ** 0.5
    print('bike', k, hex(c), 'at', at, 'vel', v, 'dist', round(d, 1), flush=True)
    # By default the bike that is least in the air (the smallest climb or fall), so the human is set
    # down on its line at its height.
    if (PICK is None and (best is None or abs(v[1]) < best[0])) or PICK == k:
        best = (abs(v[1]), k, at, v)
_, k, at, v = best
X = at[0] + v[0] * LEAD - fwd[0] * AHEAD
Z = at[2] + v[2] * LEAD - fwd[2] * AHEAD
Y = at[1] + v[1] * LEAD + UP
if AT:
    X, Y, Z = AT
print('bike', k, 'meets at', (at[0] + v[0] * LEAD, at[2] + v[2] * LEAD), '-> human to', (X, Y, Z), flush=True)
d = (X - vp[0], Y - vp[1], Z - vp[2])
regions = [(rider, 0x2000), (ch, 0x400), (veh, 0x800), (p.read32(rider + 0x18) - 0x1600, 0x1800)]
moved = 0
for base, n in regions:
    b = p.read_block(base, n)
    off = 0
    while off + 12 <= n:
        w = struct.unpack_from('<3f', b, off)
        if all(abs(w[i] - vp[i]) < 12 for i in range(3)):
            for i in range(3):
                p.write_f32(base + off + 4 * i, w[i] + d[i])
            moved += 1
            off += 12
        else:
            off += 4
print('moved', moved, 'triples', flush=True)
for o in (0x260, 0x280, 0x2b8, 0x2d0, 0x310):
    for i in range(3):
        p.write_f32(ch + o + 4 * i, 0.0)
for i in range(3):
    p.write_f32(veh + 0x130 + 4 * i, 0.0)
if MATCH:
    for i in range(3):
        p.write_f32(ch + 0x260 + 4 * i, v[i])
        p.write_f32(veh + 0x130 + 4 * i, v[i])
elif SPEED:
    for i in range(3):
        p.write_f32(ch + 0x260 + 4 * i, fwd[i] * SPEED)
        p.write_f32(veh + 0x130 + 4 * i, fwd[i] * SPEED)
p.save_state(SLOT)
wait(2.0)
print('saved', SLOT, 'at frame', p.read32(FRAME), flush=True)
