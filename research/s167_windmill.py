"""S167: drop the human's bike onto free09's rustic windmill and log whether the original flings it.

usage: python3 -B research/s167_windmill.py PID STATE_SLOT X Y Z SECONDS OUT.csv [dump]

Loads STATE_SLOT (a free09 state holding the windmill's kinematic body), pauses over AT-SPI, moves the
bike so its vehicle stands at X Y Z as research/s76_tele.py does (every copy of its position near the
rider, chassis, vehicle and the rider's pool, by one delta; the chassis' velocities zeroed), resumes
with no input and writes one row a game frame for SECONDS: the vehicle's position, the chassis'
velocity (+0x260) and the windmill body's update matrices' translations (+0x2d0, +0x310). The body is
found by the class's vtable at +8, at the address research/oracle.txt S145 names for free09.

Run it under `./kit pcsx2 with --`, with PCSX2 started under QT_ACCESSIBILITY=1 and `./kit pad serve`.
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
STATE = int(sys.argv[2])
X, Y, Z = (float(v) for v in sys.argv[3:6])
SECS = float(sys.argv[6])
OUT = Path(sys.argv[7])
SYM = cfg().symbols
FRAME = SYM['frame_counter']
MANAGER = SYM['game_manager']
BODY = 0x01291210          # free09's one kinematic body (oracle.txt S145)
BODY_VTABLE = 0x00451700
DUMP = len(sys.argv) > 8 and sys.argv[8] == 'dump'   # RAM images before the resume and at the end

app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])
p = Pine()


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


def ok(a):
    return 0x100000 <= a < 0x2000000


def wait(seconds):
    end = time.time() + seconds
    while time.time() < end:
        time.sleep(0.02)


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


def f3(a):
    return struct.unpack('<3f', p.read_block(a, 12))


p.load_state(STATE)
wait(3.0)
if not paused():
    A.press(item)
wait(0.5)
f0 = p.read32(FRAME)
wait(0.3)
assert p.read32(FRAME) == f0, 'not paused'
assert p.read32(BODY + 8) == BODY_VTABLE, 'no windmill body at %x' % BODY
veh = human(4)
print('body node before', ' '.join('%08x' % p.read32(BODY + o) for o in range(0, 0x40, 4)), flush=True)
rider = human(2)
ch = p.read32(rider + 0x7a4)
vp = f3(veh + 0x1b0)
d = (X - vp[0], Y - vp[1], Z - vp[2])
print('frame', f0, 'veh', hex(veh), 'chassis', hex(ch), 'pos', vp, 'delta', d, flush=True)
regions = [(rider, 0x2000), (ch, 0x400), (veh, 0x800), (p.read32(rider + 0x18) - 0x1600, 0x1800)]
moved = 0
for base, n in regions:
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
print('moved', moved, 'triples', flush=True)
for o in (0x260, 0x280, 0x2b8, 0x2d0, 0x310):
    for k in range(3):
        p.write_f32(ch + o + 4 * k, 0.0)
for k in range(3):
    p.write_f32(veh + 0x130 + 4 * k, 0.0)
if DUMP:
    (OUT.parent / (OUT.stem + '-before.ram')).write_bytes(p.read_block(0, 32 << 20))
rows = ['frame,x,y,z,vx,vy,vz,body_2d0_x,body_2d0_y,body_2d0_z,body_310_x,body_310_y,body_310_z']
A.press(item)
t0 = time.time()
last = None
peak = 0.0
while time.time() - t0 < SECS:
    f = p.read32(FRAME)
    if f == last:
        continue
    last = f
    pos, vel = f3(veh + 0x1b0), f3(ch + 0x260)
    b0, b1 = f3(BODY + 0x2d0), f3(BODY + 0x310)
    peak = max(peak, sum(v * v for v in vel) ** 0.5)
    rows.append(','.join('%g' % v for v in (f,) + pos + vel + b0 + b1))
if not paused():
    A.press(item)
print('body node after ', ' '.join('%08x' % p.read32(BODY + o) for o in range(0, 0x40, 4)), flush=True)
OUT.parent.mkdir(parents=True, exist_ok=True)
if DUMP:
    (OUT.parent / (OUT.stem + '-after.ram')).write_bytes(p.read_block(0, 32 << 20))
OUT.write_text('\n'.join(rows) + '\n')
print(len(rows) - 1, 'rows to', OUT, 'peak speed %.2f' % peak, 'last', rows[-1], flush=True)
