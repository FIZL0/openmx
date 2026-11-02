"""S207: park the human's bike in a path car's way and save the state, a few seconds before the car arrives.

usage: python3 -B research/s207/park.py PID LOAD_SLOT CAR CLOCK X Y Z LEAD SETTLE SAVE_SLOT

Load LOAD_SLOT and run the game until car CAR (its address, hex; `research/s207/cars.py` lists them)
has its path clock (path +0xc) LEAD game seconds short of CLOCK (its clock times its time scale +0x534;
the clock wraps at the path's last key). Then pause and move the bike (S76's teleport: every copy of its
position near the rider, the chassis, the vehicle and the rider's pool by one delta, the chassis'
velocities zeroed, the heading kept) to X Y Z, run SETTLE wall seconds for it to come to rest, pause and
save into SAVE_SLOT. Prints the car's and the bike's place when it pauses. The game is left paused.

No pad: /dev/uinput is root-only here, so the bike stands with nothing held, as S191's did.
Run it under `./kit pcsx2 with --`, PCSX2 started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1.
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

PID, LOAD = int(sys.argv[1]), int(sys.argv[2])
CAR = int(sys.argv[3], 16)
CLOCK = float(sys.argv[4])
X, Y, Z = (float(v) for v in sys.argv[5:8])
LEAD, SETTLE = float(sys.argv[8]), float(sys.argv[9])
SAVE = int(sys.argv[10])
SYM = cfg().symbols
FRAME, MANAGER = SYM['frame_counter'], SYM['game_manager']
CHASSIS = 0x7a4

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


def f1(a):
    return struct.unpack('<f', p.read_block(a, 4))[0]


def put(a, v):
    for k in range(3):
        p.write_f32(a + 4 * k, v[k])


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


path = None


def clock():
    return f1(path + 0xc)


def show(what):
    f = v3(node + 0xa0)
    print('%-8s frame %d clock %.3f car %s bike %s heading %.1f vel %s ghost %d' % (
        what, p.read32(FRAME), clock(), ['%.1f' % v for v in v3(p.read32(CAR + 0x254) + 0xb0)],
        ['%.2f' % v for v in v3(veh + 0x1b0)], math.degrees(math.atan2(f[0], f[2])),
        ['%.2f' % v for v in v3(ch + 0x260)], p.read32(rider + 0xb7c)), flush=True)


p.load_state(LOAD)
wait(3.0)
pause(True)
veh, rider = human(4), human(2)
ch = p.read32(rider + CHASSIS)
node = p.read32(ch + 0x254)
path = p.read32(CAR + 0x430)
scale = f1(CAR + 0x534)
n, keys = p.read32(path), p.read32(path + 8)
period = f1(keys + (n - 1) * 0x30)
print('rider', hex(rider), 'vehicle', hex(veh), 'chassis', hex(ch), 'car', hex(CAR), 'scale', scale, 'period', period, flush=True)
show('loaded')
want = LEAD * scale


def left():
    return (CLOCK - clock()) % period


if left() > want:
    pause(False)
    while left() > want + 0.3 * scale:
        wait(0.05 if left() < want + 3 * scale else 0.5)
    pause(True)
show('arrived')
teleport(X, Y, Z)
pause(False)
wait(SETTLE)
pause(True)
show('settled')
p.save_state(SAVE)
wait(2.0)
print('saved', SAVE, 'game seconds to the car: %.2f' % (left() / scale), flush=True)
