"""S219: put the human's bike in a path car's way, rolling at its flank or standing in its path, and save the
state a little before they meet.

usage: python3 -B research/s219/launch.py PID LOAD_SLOT CAR CLOCK SPEED HALF LEAD FRAMES SAVE_SLOT [UP [LAG]]

CAR is the car's address (hex: `research/s207/cars.py`, or the two HasPhysics trucks). P is the point of
its path at path clock (+0xc) CLOCK, read off its own keys; pick CLOCK on a stretch square to the bike
(`research/s219/segments.py`), or along it for a rear-end. LAG (ft, default 0) meets a car that trails
CAR on the same path by that much (free09's trailers, LagDistance 50): its centre reaches P LAG / speed
seconds after CAR's.

Load LOAD_SLOT and run the game until the car is LEAD game seconds short of P. Pause, and put the bike
(S76's teleport, as `research/s207/park.py`: every copy of its position near the rider, the chassis, the
vehicle and the rider's pool moved by one delta, the chassis' velocities zeroed, the heading kept) UP ft
(default 1.5) over P's height (the car's height then where the path's keys have none, its road's):
- SPEED > 0: HALF + SPEED x (the time actually left) ft back from P along the bike's heading, the chassis
  given SPEED ft/s along that heading and both wheels the rolling spin (speed over the 1.2336 ft the
  captures' spin and speed share), so it reaches HALF ft short of P as the car's centre does;
- SPEED = 0: on P itself, standing.
Then run FRAMES game frames, pause and save into SAVE_SLOT. Prints the car and the bike at each step. The
game is left paused.

No pad (/dev/uinput is root-only here): nothing is held, so a rolling bike coasts.
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
CLOCK, SPEED, HALF, LEAD = (float(v) for v in sys.argv[4:8])
FRAMES, SAVE = int(sys.argv[8]), int(sys.argv[9])
UP = float(sys.argv[10]) if len(sys.argv) > 10 else 1.5
LAG = float(sys.argv[11]) if len(sys.argv) > 11 else 0.0
SYM = cfg().symbols
FRAME, MANAGER = SYM['frame_counter'], SYM['game_manager']
CHASSIS, FRONT, REAR = 0x7a4, 0x7b0, 0x7b4
SPIN = 0x1d4
ROLL = 1.2336

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


def car_pos():
    return v3(p.read32(CAR + 0x254) + 0xb0)


def heading():
    f = v3(node + 0xa0)
    return math.atan2(f[0], f[2])


def show(what):
    c = car_pos()
    b = v3(veh + 0x1b0)
    print('%-8s frame %d clock %.3f car %s bike %s heading %.1f vel %s apart %.1f reason %d ghost %d' % (
        what, p.read32(FRAME), f1(path + 0xc), ['%.1f' % v for v in c], ['%.2f' % v for v in b],
        math.degrees(heading()), ['%.2f' % v for v in v3(ch + 0x260)],
        math.hypot(c[0] - b[0], c[2] - b[2]), p.read32(rider + 0x748), p.read32(rider + 0xb7c)), flush=True)


def left(clock):
    """Game seconds until the car's clock reads `clock`."""
    return ((clock - f1(path + 0xc)) % period) / scale


def run_until(clock, lead):
    """Run the game until the car is `lead` game seconds short of `clock`, then pause."""
    if left(clock) > lead:
        pause(False)
        while left(clock) > lead + 0.05:
            wait(0.002 if left(clock) < lead + 2 else 0.25)
        pause(True)


def run_frames(frames):
    f0 = p.read32(FRAME)
    pause(False)
    deadline = time.time() + 60
    while p.read32(FRAME) < f0 + frames - 1:
        assert time.time() < deadline, 'the game stopped'
        time.sleep(0.002)
    pause(True)


p.load_state(LOAD)
wait(3.0)
pause(True)
veh, rider = human(4), human(2)
ch = p.read32(rider + CHASSIS)
node = p.read32(ch + 0x254)
path = p.read32(CAR + 0x430)
scale = f1(CAR + 0x534)
n, keys = p.read32(path), p.read32(path + 8)
raw = p.read_block(keys, n * 0x30)
key = [(struct.unpack_from('<f', raw, 0x30 * i)[0], struct.unpack_from('<3f', raw, 0x30 * i + 0x10)) for i in range(n)]
period = key[-1][0]
i = max(j for j in range(n - 1) if key[j][0] <= CLOCK)
(ta, a), (tb, b) = key[i], key[i + 1]
u = (CLOCK - ta) / (tb - ta)
P = [a[k] + u * (b[k] - a[k]) for k in range(3)]
carh = math.degrees(math.atan2(b[0] - a[0], b[2] - a[2]))
carv = math.hypot(b[0] - a[0], b[2] - a[2]) / (tb - ta) * scale
print('rider', hex(rider), 'vehicle', hex(veh), 'chassis', hex(ch), 'car', hex(CAR), 'scale', scale, 'period', period,
      flush=True)
print('P %.2f %.2f at clock %.3f: the car heading %.1f at %.1f ft/s' % (P[0], P[2], CLOCK, carh, carv), flush=True)
show('loaded')
late = LAG / carv
run_until(CLOCK, LEAD - late)
show('arrived')
h = heading()
L = left(CLOCK) + late
back = HALF + SPEED * L if SPEED > 0 else 0.0
print('%.3f s left: the bike %.2f ft back from P at heading %.1f, %.1f deg off square to the car' % (
    L, back, math.degrees(h), (math.degrees(h) - carh) % 180 - 90), flush=True)
teleport(P[0] - back * math.sin(h), (P[1] if abs(P[1]) > 1 else car_pos()[1]) + UP, P[2] - back * math.cos(h))
if SPEED > 0:
    put(ch + 0x260, (SPEED * math.sin(h), 0.0, SPEED * math.cos(h)))
    for wheel in (FRONT, REAR):
        p.write_f32(p.read32(rider + wheel) + SPIN, SPEED / ROLL)
show('placed')
run_frames(FRAMES)
show('saved')
p.save_state(SAVE)
wait(2.0)
print('saved', SAVE, 'game seconds to P: %.2f' % (left(CLOCK) + late), flush=True)
