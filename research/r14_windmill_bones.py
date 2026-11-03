"""R14: research/s167_windmill.py's run, logging the rider's ragdoll bones as well.

usage: python3 -B research/r14_windmill_bones.py PID STATE_SLOT X Y Z SECONDS OUT.csv [bones] [ride=N]

The same teleport, pause and resume as research/s167_windmill.py (its columns come first, so
tests/replay.rs `s211_capture` reads the csv as it reads S167's), and RAM images OUT-before.ram and
OUT-after.ram. Per game frame, read in one batch the moment the counter moves:

* frame, the vehicle's position, the chassis' velocity, the windmill body's +0x2d0 and +0x310
  translations (S167's thirteen columns);
* `cofg`: the chassis' +0x60, the second point the map edge asks of (S59);
* `pub`: the rider's +0xd40, the main bone the body actor publishes onto the bike (S59's a1);
* `reason`: the rider's +0x748 wreck reason (13 is the cannonball);
* the body actor's riding (+0x2a4), placed (+0x2ac), off-bike (+0x6b0), settled (+0x6b4) and flung
  (+0x6b8) words;
* `touch`: a bitmask of the nine bone collisions' touched words (+0x14);
* `b{i}` for the eighteen parts (actor +0x2f4, stride 0xb0, body at +0xa4): the body's +0x60
  position and +0x70 velocity;
* `same`: whether a second read straight after the first gave the same bytes (0 marks a row the
  game may have been writing while it was read).

The body actor is the human vehicle's address minus 0x400 (checked by its vtable, 0x00458eb0).

Without `bones` the teleport is S167's, which leaves the ragdoll's bone bodies where the bike was:
on the next frames the riding ragdoll's joints drag them across the gap, and the wreck flings them
at ~15,000 ft/s along it (R14). With `bones` the same delta also moves every triple near the bike
in the body actor's neighbourhood (actor - 0x2800 .. vehicle + 0x2800), the parts array and the
bodies after it, and the nine bone collisions.

After the load it rides 30 frames (no pad) before it pauses; `ride=0` teleports on the loaded frame
itself (B15: a state saved mid-jump, research/b15_recording2_state.py's).
Run it under `./kit pcsx2 with --`, with PCSX2 started under QT_ACCESSIBILITY=1.
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
BONES = 'bones' in sys.argv[8:]   # move the ragdoll with the bike
RIDE = next((int(a[5:]) for a in sys.argv[8:] if a.startswith('ride=')), 30)
SYM = cfg().symbols
FRAME = SYM['frame_counter']
MANAGER = SYM['game_manager']
BODY = 0x01291210          # free09's one kinematic body (oracle.txt S145)
BODY_VTABLE = 0x00451700
ACTOR_VTABLE = 0x00458eb0  # the rider's body actor (first_recording's at 0x01591c10)
PARTS = 18

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


if RIDE == 0 and not paused():   # a load keeps the VM paused
    A.press(item)
    wait(0.5)
p.load_state(STATE)
wait(1.0)
if RIDE:
    # Run a while past the load: on the loaded frame itself the image is not a ridden tick's (R14
    # saw the port wreck a tick early from one, which B16 found was the test's seeding), and the
    # first tick after a load reads the saved controls.
    if paused():   # a previous run leaves the VM paused, and a load keeps it so
        A.press(item)
    start = p.read32(FRAME)
    end = time.time() + 30
    while p.read32(FRAME) < start + RIDE and time.time() < end:
        wait(0.1)
if not paused():
    A.press(item)
wait(0.5)
f0 = p.read32(FRAME)
wait(0.3)
assert p.read32(FRAME) == f0, 'not paused'
assert p.read32(BODY + 8) == BODY_VTABLE, 'no windmill body at %x' % BODY
veh = human(4)
rider = human(2)
actor = veh - 0x400
assert p.read32(actor + 8) == ACTOR_VTABLE, 'no body actor at %x' % actor
parts = p.read32(actor + 0x2f4)
bones = [p.read32(parts + i * 0xb0 + 0xa4) for i in range(PARTS)]
cols = p.read32(actor + 0x624)
coll = [p.read32(cols + 4 * i) for i in range(9)]
ch = p.read32(rider + 0x7a4)
vp = f3(veh + 0x1b0)
d = (X - vp[0], Y - vp[1], Z - vp[2])
print('frame', f0, 'veh', hex(veh), 'rider', hex(rider), 'actor', hex(actor), 'chassis', hex(ch), 'pos', vp,
      'delta', d, flush=True)
print('bones', ' '.join('%x' % b for b in bones), flush=True)
regions = [(rider, 0x2000), (ch, 0x400), (veh, 0x800), (p.read32(rider + 0x18) - 0x1600, 0x1800)]
if BONES:
    # The body actor's neighbourhood, the parts and their bodies (and what follows them), and the
    # bone collisions: everything of the ragdoll's that keeps a world position (see the docstring).
    regions += [(actor - 0x2800, 0x2c00), (veh + 0x800, 0x2000), (parts, max(bones) + 0x1400 - parts),
                (min(coll), max(coll) + 0x100 - min(coll))]
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
(OUT.parent / (OUT.stem + '-before.ram')).write_bytes(p.read_block(0, 32 << 20))

# One batch a row: (address, length) and how each is unpacked.
lo, hi = min(bones), max(bones) + 0x80
reads = [(veh + 0x1b0, 12), (ch + 0x260, 12), (BODY + 0x2d0, 12), (BODY + 0x310, 12), (ch + 0x60, 12),
         (rider + 0xd40, 12), (rider + 0x748, 4), (actor + 0x2a4, 12), (actor + 0x6b0, 12)]
reads += [(c + 0x14, 4) for c in coll]
reads += [(lo, hi - lo)]

head = ['frame', 'x', 'y', 'z', 'vx', 'vy', 'vz', 'body_2d0_x', 'body_2d0_y', 'body_2d0_z', 'body_310_x',
        'body_310_y', 'body_310_z', 'cofg_x', 'cofg_y', 'cofg_z', 'pub_x', 'pub_y', 'pub_z', 'reason', 'riding',
        'placed', 'off_bike', 'settled', 'flung', 'touch']
for i in range(PARTS):
    head += ['b%d_%s' % (i, c) for c in ('x', 'y', 'z', 'vx', 'vy', 'vz')]
head += ['same']


def row(f, raw):
    o = 0

    def take(fmt):
        nonlocal o
        v = struct.unpack_from(fmt, raw, o)
        o += struct.calcsize(fmt)
        return v
    out = [f]
    for _ in range(6):
        out += take('<3f')
    out += take('<I')
    riding, _, placed = take('<3I')
    off_bike, settled, flung = take('<3I')
    out += [riding, placed, off_bike, settled, flung]
    touch = 0
    for i in range(9):
        touch |= (take('<I')[0] & 1) << i
    out.append(touch)
    for b in bones:
        out += struct.unpack_from('<3f', raw, o + b - lo + 0x60) + struct.unpack_from('<3f', raw, o + b - lo + 0x70)
    return out


rows = [','.join(head)]
A.press(item)
t0 = time.time()
last = None
while time.time() - t0 < SECS:
    f = p.read32(FRAME)
    if f == last:
        continue
    last = f
    raw = p.read_regions(reads)
    again = p.read_regions(reads)
    rows.append(','.join('%.9g' % v for v in row(f, raw) + [int(raw == again)]))
if not paused():
    A.press(item)
OUT.parent.mkdir(parents=True, exist_ok=True)
(OUT.parent / (OUT.stem + '-after.ram')).write_bytes(p.read_block(0, 32 << 20))
OUT.write_text('\n'.join(rows) + '\n')
print(len(rows) - 1, 'rows to', OUT, flush=True)
