"""C16: put P2's idle bike in the plane's ground path, settle it, save a state.

usage: python3 -B research/c16/tele_p2.py PID LOAD_SLOT X Y Z SECONDS SAVE_SLOT

research/s76_tele.py for the second player of the 2p state: load LOAD_SLOT, pause over AT-SPI, move P2's
bike (the type-2 actor whose player record has index +0x30 = 1, and the type-4 actor of the same player)
by one delta, every copy of its position near the rider, the chassis, the vehicle and the rider's pool, so
the vehicle stands at X Y Z; zero the chassis' velocities, resume for SECONDS with the pad idle (P2 has no
tape here), pause and save SAVE_SLOT. The bike keeps its heading. Run it under `./kit pcsx2 with --`.
"""
import struct
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'research/journal/2026-10-20-c6-c6-recording2-capture/playback'))
from ps2kit.pine import Pine             # noqa: E402
import atspi_lib as A                    # noqa: E402
from gi.repository import Atspi          # noqa: E402

PID, LOAD = int(sys.argv[1]), int(sys.argv[2])
X, Y, Z = (float(v) for v in sys.argv[3:6])
SECS, SLOT = float(sys.argv[6]), int(sys.argv[7])
FRAME, MANAGER = 0x00479d04, 0x004e1e98

item = A.menu_path(A.app_of(PID), ['System', 'Pause'])
paused = lambda: item.get_state_set().contains(Atspi.StateType.CHECKED)
p = Pine(check=False, lock=False, timeout=5.0)
ok = lambda a: 0x100000 <= a < 0x2000000


def wait(s):
    end = time.time() + s
    while time.time() < end:
        time.sleep(0.05)


def p2(kind):
    node = p.read32(p.read32(p.read32(MANAGER) + 0x34) + 0x34)
    for _ in range(40):
        it = p.read32(node)
        if ok(it) and p.read32(it) == kind:
            pl = p.read32(it + 0x20)
            if ok(pl) and p.read32(pl + 0x30) == 1:
                return it
        node = p.read32(node + 0x8)
    raise SystemExit('no P2 actor of type %d' % kind)


if paused():
    A.press(item)
p.load_state(LOAD)
wait(3)
A.press(item)
wait(0.5)
f0 = p.read32(FRAME)
wait(0.3)
assert p.read32(FRAME) == f0, 'not paused'
veh, rider = p2(4), p2(2)
ch = p.read32(rider + 0x7a4)
vp = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
d = (X - vp[0], Y - vp[1], Z - vp[2])
print('frame', f0, 'veh', hex(veh), 'rider', hex(rider), 'chassis', hex(ch), 'pos', vp, 'delta', d, flush=True)
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
A.press(item)
t0 = time.time()
while time.time() - t0 < SECS:
    pos = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
    vel = struct.unpack('<3f', p.read_block(ch + 0x260, 12))
    print('%5d pos %9.2f %8.2f %9.2f vel %7.2f %7.2f %7.2f' % ((p.read32(FRAME),) + pos + vel), flush=True)
    wait(0.5)
if not paused():
    A.press(item)
wait(0.5)
p.save_state(SLOT)
wait(2.0)
print('saved', SLOT, 'at frame', p.read32(FRAME), flush=True)
