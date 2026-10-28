"""S162: put the human's free06 bike at a spot facing a set heading, braked and settled, and save it.

usage: python3 -B research/s162/turn.py PID LOAD_SLOT X Y Z HEADING SAVE_SLOT [SETTLE_S]

From S89's along state (free06, the 250; its rider chain is S89's, the human found by control source as
research/s129/tele.py does): load LOAD_SLOT, pause, move every copy of the bike's position by one delta
so the vehicle stands at X Y Z (research/s76_tele.py's teleport) and zero the chassis' velocities. Then
turn it to HEADING (degrees, atan2 of the node's forward x over z — the port's `--trace-at` heading) by
riding it round: a little gas with the stick full over until the heading is near, then both brakes,
again until it is within TOL. (Writing a yaw rate in the air does not turn it: the game's air control
replaces the chassis' angular velocity the next tick.) Then teleport it back onto X Z at the height it
first settled at there, heading kept, settle SETTLE_S wall seconds braked, pause, and save into SAVE_SLOT
(brakes held). The game is left paused. Run it under `./kit pcsx2 with --`, on a copy started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
QT_ACCESSIBILITY=1, `./kit pad serve` up.
"""

import math
import struct
import subprocess
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
X, Y, Z, HEADING = (float(v) for v in sys.argv[3:7])
SAVE = int(sys.argv[7])
SETTLE = float(sys.argv[8]) if len(sys.argv) > 8 else 16.0
TOL = 6.0
SYM = cfg().symbols
FRAME, MANAGER = SYM['frame_counter'], SYM['game_manager']
AIR, WRECK, CHASSIS = 0x57c, 0x748, 0x7a4

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


def wrap(d):
    return (d + 180.0) % 360.0 - 180.0


def heading():
    f = v3(node + 0xa0)
    return math.degrees(math.atan2(f[0], f[2]))


def show(what):
    print('%-8s frame %d pos %9.2f %8.2f %9.2f heading %7.2f vel %7.2f %7.2f %7.2f air %d reason %d' % (
        (what, p.read32(FRAME)) + v3(veh + 0x1b0) + (heading(),) + v3(ch + 0x260) + (p.read32(rider + AIR), p.read32(rider + WRECK))),
        flush=True)


def pad(*cmds):
    subprocess.run([str(ROOT / 'kit'), 'pad', 'send', *cmds], check=False)


def teleport(x, y, z):
    """Every copy of the bike's position near the rider, the chassis, the vehicle and the rider's pool
    moved by one delta so the vehicle stands at x y z, and the chassis' velocities zeroed."""
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
    print('moved', moved, 'triples by', d, flush=True)


p.load_state(LOAD)
wait(3.0)
pause(True)
veh, rider = human(4), human(2)
ch = p.read32(rider + CHASSIS)
node = p.read32(ch + 0x254)
print('rider', hex(rider), 'vehicle', hex(veh), 'chassis', hex(ch), flush=True)
teleport(X, Y, Z)
pad('down square', 'down r2')
pause(False)
wait(12.0)
show('landed')
ground = v3(veh + 0x1b0)[1]

# Which way the stick turns it: right is +1.
sign = 1.0
for go in range(16):
    err = wrap(HEADING - heading())
    if abs(err) < TOL:
        break
    if p.read32(rider + WRECK):
        raise SystemExit('wrecked')
    pad('release', 'stick l %g 0' % (sign if err > 0 else -sign), 'down cross')
    h0, t0 = heading(), time.time()
    while time.time() - t0 < 30:
        e = wrap(HEADING - heading())
        if abs(e) < TOL / 2 or e * err < 0:
            break
        if time.time() - t0 > 6 and abs(wrap(heading() - h0)) < 3:
            break
        if go == 0 and abs(wrap(heading() - h0)) > 10:
            if wrap(heading() - h0) * err < 0:
                sign = -sign
                print('the stick turns it the other way', flush=True)
                break
    pad('release', 'down square', 'down r2')
    show('turn %d' % go)
    wait(16.0)
    show('stopped')

err = wrap(HEADING - heading())
if abs(err) >= TOL:
    raise SystemExit('heading %.2f still %.2f off' % (heading(), err))
pause(True)
teleport(X, ground + 0.5, Z)
pause(False)
wait(SETTLE)
pause(True)
show('saving')
p.save_state(SAVE)
wait(2.0)
print('saved', SAVE, flush=True)
