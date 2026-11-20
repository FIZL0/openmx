"""S494: ride the human's bike into a parked flier (free10's biplane, free06's helicopter) and log the
flier's body, as research/s484/ride.py does for free09's truck.

  python3 -B research/s494/ride.py PID MACHINE DIST FRAMES OUT.csv YAW LIFT

flier.sh's tape holds the gas and leaves the VM paused at the state's frame. This finds the flier's
body (machine +0x160, as the car's), moves the bike DIST ft short of the body on the line of
heading YAW (degrees, 0 = +z: give the bike's own, which this does not turn; without it, the line
from the bike to the flier) at the ground's height under the flier (machine +0xe0's stored spot less
LIFT) plus 0.3 ft (S76's tele, one
delta to every copy of the bike's place near the rider), zeroes its velocities, resumes, and logs one
row a game frame: the flier's body (+0 awake, place +0x60, velocity +0x70, quaternion +0x170) and the
bike's place (veh +0x1b0).

  python3 -B research/s494/ride.py PID MACHINE probe

prints the bike's transform and the flier's body and changes nothing.
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

PID = int(sys.argv[1])
MACHINE = int(sys.argv[2], 16)
PROBE = sys.argv[3] == 'probe'
SYM = cfg().symbols
FRAME = SYM['frame_counter']
MANAGER = SYM['game_manager']
p = Pine()


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


def floats(a, n):
    return struct.unpack('<%df' % n, p.read_block(a, 4 * n))


def wait(seconds):
    end = time.time() + seconds
    while time.time() < end:
        time.sleep(0.02)


veh, rider = human(4), human(2)
ch = p.read32(rider + 0x7a4)
body = p.read32(MACHINE + 0x160)
spot = floats(MACHINE + 0xe0, 3)
fp = floats(body + 0x60, 3)
vp = floats(veh + 0x1b0, 3)
m = floats(veh + 0x180, 16)
print('veh', hex(veh), 'chassis', hex(ch), 'flier body', hex(body), 'awake', p.read32(body), 'at', fp, 'spot', spot, flush=True)
print('bike at', vp, flush=True)
for r in range(4):
    print('  m%d' % r, ['%.4f' % v for v in m[4 * r:4 * r + 4]], flush=True)
if PROBE:
    raise SystemExit(0)

DIST, FRAMES, OUT = float(sys.argv[3]), int(sys.argv[4]), sys.argv[5]
YAW = float(sys.argv[6]) if len(sys.argv) > 6 and sys.argv[6] != '-' else None
LIFT = float(sys.argv[7]) if len(sys.argv) > 7 else 0.0
app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


if not paused():
    A.press(item)
wait(0.5)
f0 = p.read32(FRAME)
wait(0.3)
assert p.read32(FRAME) == f0, 'not paused'
vp = floats(veh + 0x1b0, 3)
fp = floats(body + 0x60, 3)
if YAW is None:
    yaw = math.atan2(fp[0] - vp[0], fp[2] - vp[2])
else:
    yaw = math.radians(YAW)
fwd = (math.sin(yaw), 0.0, math.cos(yaw))
target = (fp[0] - fwd[0] * DIST, spot[1] - LIFT + 0.3, fp[2] - fwd[2] * DIST)
d = tuple(target[k] - vp[k] for k in range(3))
print('frame', f0, 'yaw', round(math.degrees(yaw), 2), 'target', target, flush=True)
moved = 0
for base, n in [(rider, 0x2000), (ch, 0x400), (veh, 0x800), (p.read32(rider + 0x18) - 0x1600, 0x1800)]:
    b = p.read_block(base, n)
    off = 0
    while off + 12 <= n:
        v = struct.unpack_from('<3f', b, off)
        if all(abs(v[k] - vp[k]) < 12 for k in range(3)):
            for k in range(3):
                p.write_f32(base + off + 4 * k, v[k] + d[k])
            moved += 1
            off += 12
        else:
            off += 4
print('moved', moved, 'triples by', d, flush=True)
for o in (0x260, 0x280, 0x2b8, 0x2d0, 0x310):
    for k in range(3):
        p.write_f32(ch + o + 4 * k, 0.0)
for k in range(3):
    p.write_f32(veh + 0x130 + 4 * k, 0.0)
A.press(item)
rows = []
last = None
t0 = time.time()
while len(rows) < FRAMES and time.time() - t0 < 900:
    f = p.read32(FRAME)
    if f == last:
        if paused():
            A.press(item)        # the movie's end pauses the VM
            wait(0.3)
        continue
    last = f
    b = p.read_regions([(body, 4), (body + 0x60, 0x20), (body + 0x170, 0x10), (veh + 0x1b0, 12)])
    v = struct.unpack_from('<I8f4f3f', b, 0)
    rows.append((f,) + v[0:4] + v[5:8] + v[9:13] + v[13:16] + (p.read32(FRAME),))
if not paused():
    A.press(item)
with open(OUT, 'w') as out:
    out.write('frame,awake,x,y,z,vx,vy,vz,qx,qy,qz,qw,bike_x,bike_y,bike_z,frame_after\n')
    for r in rows:
        out.write(','.join(str(v) for v in r) + '\n')
print(len(rows), 'rows; flier first', rows[0][2:8], 'last', rows[-1][2:8], 'bike last', rows[-1][12:15], flush=True)
