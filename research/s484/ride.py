"""S484: ride recording2's bike into free09's parked monster truck and log the truck's body.

  python3 -B research/s484/ride.py PID X Z FRAMES OUT.csv

The tape (truck.sh) holds the gas and leaves the VM paused at the state's frame. This moves the
human's bike — every copy of its position near the rider, the chassis, the vehicle and the rider's
pool, by one delta (S76's tele) — so the vehicle stands at X, its own height over flat ground, Z,
keeping its heading (+z on recording2), zeroes its velocities, resumes, and then logs one row a game
frame for FRAMES frames: the truck's body (+0 awake, position, velocity, turn) and the bike's place.

  python3 -B research/s484/ride.py PID X Z FRAMES OUT.csv bones

R57: with `bones`, each row also holds the rider's nine bone collisions (the body actor's +0x620 /
+0x624 list, the rider file's order), per collision its bone (the part whose body +0xa4 it is, of the
actor's +0x2f4 parts), the body's +0 awake, place +0x60 and velocity +0x70, and the collision's +0x14
touched, +0x1c scene and +4 on (tested: off while riding) words; then the actor's +0x48c (something
is touching) and +0x6b0 (the bones are off the bike), and the frame counter read after the row (a
row whose two frames differ was read across a tick).

  python3 -B research/s484/ride.py PID X Z FRAMES OUT.csv full

R60a: `full` is `bones` plus, per row, the bike's drawn transform (veh +0x180..+0x1bf, whose last
row is +0x1b0's place), the body actor's request block (actor +0x488 -> its +0x50 frame, +0x90
velocity, +0xa0 rates, +0x104 reason, +0x170 pitch, +0x178 speed, +0x180 centre, +0x190 quat), the
actor's gravity scale (+0x2d0) and mode (+0x47c), and per bone its whole dynamics body (BODY_FIELDS:
the last substep's force +0x80, the collision response's +0xc0, the orientation +0x170, the rates
+0x1b0, ...).

  python3 -B research/s484/ride.py PID X Z FRAMES OUT.csv actor

R60a: `actor` is `full` plus the body actor's words +0x400..+0x6ff as floats (`a<off>`, hex): the
look clocks, the touching flag, the hold targets (+0x4d0..) and velocity (+0x530), the pose spring
(+0x650) and damper (+0x660), the wreck fields.
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

PID = int(sys.argv[1])
X, Z = float(sys.argv[2]), float(sys.argv[3])
FRAMES = int(sys.argv[4])
OUT = sys.argv[5]
ACTOR = sys.argv[6:7] == ['actor']
FULL = sys.argv[6:7] == ['full'] or ACTOR
BONES = sys.argv[6:7] == ['bones'] or FULL
ACTOR_VTABLE = 0x00458eb0       # the rider body actor's, at +8 (S476)
SYM = cfg().symbols
FRAME = SYM['frame_counter']
MANAGER = SYM['game_manager']
TRUCK = 0x00e1e210              # free09's parked truck on recording2 (research/oracle.txt S461)

app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])
p = Pine()


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


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


def body_actor():
    """The human's rider body actor: a list item with the player record whose actor (item - 0x400)
    has the body actor's vtable (research/r53/tele_p2.py)."""
    node = p.read32(p.read32(p.read32(MANAGER) + 0x34) + 0x34)
    for _ in range(60):
        if not ok(node):
            break
        it = p.read32(node)
        if ok(it):
            pl = p.read32(it + 0x20)
            if ok(pl) and p.read32(pl + 0x34) == 1 and p.read32(it - 0x400 + 8) == ACTOR_VTABLE:
                return it - 0x400
        node = p.read32(node + 0x8)
    raise SystemExit('no rider body actor')


def bone_collisions(actor):
    """Per bone collision (the rider file's order): (collision, body, bone)."""
    n, arr = p.read32(actor + 0x620), p.read32(actor + 0x624)
    parts = p.read32(actor + 0x2f4)
    bodies = [p.read32(parts + 0xb0 * i + 0xa4) for i in range(p.read32(actor + 0x2e0))]
    out = []
    for k in range(n):
        c = p.read32(arr + 4 * k)
        b = p.read32(c + 0x7c)
        out.append((c, b, bodies.index(b) if b in bodies else -1))
    return out


def wait(seconds):
    end = time.time() + seconds
    while time.time() < end:
        time.sleep(0.02)


def quad(a):
    return struct.unpack('<4f', p.read_block(a, 16))


if not paused():
    A.press(item)
wait(0.5)
f0 = p.read32(FRAME)
wait(0.3)
assert p.read32(FRAME) == f0, 'not paused'
veh, rider = human(4), human(2)
ch = p.read32(rider + 0x7a4)
body = p.read32(TRUCK + 0x160)
vp = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
d = (X - vp[0], 135.0 + 0.3 - vp[1], Z - vp[2])
print('frame', f0, 'veh', hex(veh), 'chassis', hex(ch), 'truck body', hex(body), 'awake', p.read32(body), 'at', quad(body + 0x60), flush=True)
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
bones = []
if BONES:
    actor = body_actor()
    bones = bone_collisions(actor)
    print('body actor', hex(actor), 'bones', [(hex(c), hex(b), i) for c, b, i in bones], flush=True)
regions = [(body, 4), (body + 0x60, 0x20), (body + 0x170, 0x10), (veh + 0x1b0, 12)]
for c, b, _ in bones:
    regions += [(b, 4), (b + 0x60, 0x20), (c + 0x14, 4), (c + 0x1c, 4), (c + 4, 4)]
if bones:
    regions += [(actor + 0x48c, 4), (actor + 0x6b0, 4)]
REQ_FIELDS = [('frame', 0x50, 16), ('vel', 0x90, 4), ('rates', 0xa0, 4), ('pitch', 0x170, 1),
              ('speed', 0x178, 1), ('centre', 0x180, 4), ('quat', 0x190, 4)]
# The dynamics body's quadwords (game_sim::biplane::Body, tests/car_drive.rs body_of), after +0x10's
# inverse mass.
BODY_FIELDS = [('damp', 0x20), ('lastp', 0x50), ('lastf', 0x80), ('force', 0x90), ('acc', 0xa0),
               ('shockpk', 0xb0), ('shock', 0xc0), ('step', 0xf0), ('invi', 0x120), ('angdamp', 0x130),
               ('lastq', 0x150), ('q', 0x170), ('cofg', 0x180), ('mom', 0x190), ('lastmom', 0x1a0),
               ('rates', 0x1b0), ('angacc', 0x1c0), ('twistpk', 0x1d0), ('twist', 0x1e0)]
if FULL:
    req = p.read32(actor + 0x488)
    print('request', hex(req), flush=True)
    regions += [(veh + 0x180, 0x40)]
    regions += [(req + o, 4 * n) for _, o, n in REQ_FIELDS]
    regions += [(req + 0x104, 4), (actor + 0x2d0, 4), (actor + 0x47c, 4)]
    for c, b, _ in bones:
        regions += [(b + 0x10, 4)] + [(b + o, 0x10) for _, o in BODY_FIELDS]
if ACTOR:
    regions += [(actor + 0x400, 0x300)]
A.press(item)
rows = []
last = None
t0 = time.time()
while len(rows) < FRAMES and time.time() - t0 < 600:
    f = p.read32(FRAME)
    if f == last:
        if paused():
            A.press(item)        # the movie's end pauses the VM
            wait(0.3)
        continue
    last = f
    if not bones:
        pos, vel, q = quad(body + 0x60), quad(body + 0x70), quad(body + 0x170)
        bike = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
        rows.append((f, p.read32(body)) + pos[:3] + vel[:3] + q + bike)
        continue
    b = p.read_regions(regions)
    f1 = p.read32(FRAME)
    v = struct.unpack_from('<I8f4f3f', b, 0)
    row = v[0:4] + v[5:8] + v[9:13] + v[13:16]
    row = (f,) + row
    off = 64
    for _ in bones:
        on, = struct.unpack_from('<I', b, off)
        bv = struct.unpack_from('<8f', b, off + 4)
        touched, scene, tested = struct.unpack_from('<3I', b, off + 36)
        row += (on,) + bv[0:3] + bv[4:7] + (touched, scene, tested)
        off += 48
    row += struct.unpack_from('<2I', b, off) + (f1,)
    off += 8
    if FULL:
        row += struct.unpack_from('<16f', b, off)
        off += 0x40
        for _, _, n in REQ_FIELDS:
            row += struct.unpack_from('<%df' % n, b, off)
            off += 4 * n
        reason, = struct.unpack_from('<i', b, off)
        grav, = struct.unpack_from('<f', b, off + 4)
        mode, = struct.unpack_from('<I', b, off + 8)
        row += (reason, grav, mode)
        off += 12
        for _ in bones:
            n = 1 + 4 * len(BODY_FIELDS)
            row += struct.unpack_from('<%df' % n, b, off)
            off += 4 * n
        if ACTOR:
            row += struct.unpack_from('<192f', b, off)
    rows.append(row)
if not paused():
    A.press(item)
with open(OUT, 'w') as out:
    head = 'frame,awake,x,y,z,vx,vy,vz,qx,qy,qz,qw,bike_x,bike_y,bike_z'
    for _, _, i in bones:
        head += ''.join(',b%d_%s' % (i, s) for s in ('awake', 'x', 'y', 'z', 'vx', 'vy', 'vz', 'touched', 'scene', 'on'))
    if bones:
        head += ',touching,off_bike,frame_after'
    if FULL:
        head += ''.join(',bike_m%d%d' % (r, k) for r in range(4) for k in range(4))
        for name, _, n in REQ_FIELDS:
            head += ''.join(',req_%s%d' % (name, k) for k in range(n)) if n > 1 else ',req_' + name
        head += ',req_reason,gravity,mode'
        for _, _, i in bones:
            head += ',b%d_invmass' % i + ''.join(',b%d_%s%s' % (i, s, a) for s, _ in BODY_FIELDS for a in 'xyzw')
    if ACTOR:
        head += ''.join(',a%03x' % o for o in range(0x400, 0x700, 4))
    out.write(head + '\n')
    for r in rows:
        out.write(','.join(str(v) for v in r) + '\n')
woke = [r for r in rows if r[1]]
print(len(rows), 'rows;', len(woke), 'with the truck awake; first', woke[0][0] if woke else None, flush=True)
print('truck first', rows[0][2:5], 'last', rows[-1][2:5], 'bike last', rows[-1][12:15], flush=True)
