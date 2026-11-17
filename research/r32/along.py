"""R32: rebuild S89's s89-along state while research/r32/switch.pad's tape plays.

usage: python3 -B research/r32/along.py PID SLOT

Run under `./kit pcsx2 with --` once the tape is playing (research/r32/make.sh). It waits for the tape's
brake (Square in port 0's pad buffer), pauses over AT-SPI, moves the human's bike onto the railway bed's
x≈4591 straight facing +z exactly as research/s76_tele.py does (every copy of its position near the
rider, the chassis, the vehicle and the rider's pool shifted by one delta, the chassis' velocities
zeroed), resumes, and pauses again when the train's clock reaches CLOCK, then saves SLOT.

CLOCK is where the old s89-along stood: S146's roof state (frame 11932, the engine's clock 41.487) was
made from it by 16 + 30 frames, so ≈ 41.487 - 46/30 ≈ 39.95, with the boxcar on the z≈2318 straight
running +x, which tests/traffic.rs and tests/ragdoll_cars.rs ride on.
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

PID, SLOT = int(sys.argv[1]), int(sys.argv[2])
ALONG = (4591.5, 135.46, 2899.7)         # S89's journal: the vehicle's spot on the bed
CLOCK = 39.95
ENGINE = 0x00d525f0                       # the train's engine, the first path car (tests/traffic.rs CARS)
SYM = cfg().symbols
FRAME = SYM['frame_counter']
MANAGER = SYM['game_manager']
PADS = SYM['pad_buffers']

app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])
p = Pine()


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


def pause(on):
    if paused() != on:
        A.press(item)
    time.sleep(0.5)


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


def square():
    """Square down in either half of port 0's buffer (byte 3, active low)."""
    return any(not p.read8(PADS + h + 3) & 0x80 for h in (0, 0x80))


def clock():
    return struct.unpack('<f', p.read_block(p.read32(ENGINE + 0x430) + 0xc, 4))[0]


deadline = time.time() + 600
while not square():
    assert time.time() < deadline, 'the tape never braked'
    time.sleep(0.1)
time.sleep(1.0)
pause(True)
f0 = p.read32(FRAME)
time.sleep(0.3)
assert p.read32(FRAME) == f0, 'not paused'
veh, rider = human(4), human(2)
ch = p.read32(rider + 0x7a4)
vp = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
d = tuple(ALONG[k] - vp[k] for k in range(3))
print('frame', f0, 'clock %.3f' % clock(), 'veh', hex(veh), 'rider', hex(rider), 'chassis', hex(ch),
      'pos', vp, 'delta', d, flush=True)
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
print('moved', moved, 'triples', flush=True)
for o in (0x260, 0x280, 0x2b8, 0x2d0, 0x310):
    for k in range(3):
        p.write_f32(ch + o + 4 * k, 0.0)
for k in range(3):
    p.write_f32(veh + 0x130 + 4 * k, 0.0)
pause(False)
deadline = time.time() + 600
last = 0
while True:
    c = clock()
    if c >= CLOCK - 0.05:
        break
    assert time.time() < deadline, 'the clock stuck at %.3f' % c
    if time.time() - last > 2:
        pos = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
        vel = struct.unpack('<3f', p.read_block(ch + 0x260, 12))
        print('%5d clock %7.3f pos %9.2f %8.2f %9.2f vel %7.2f %7.2f %7.2f'
              % ((p.read32(FRAME), c) + pos + vel), flush=True)
        last = time.time()
    time.sleep(0.02)
pause(True)
pos = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
print('paused at frame', p.read32(FRAME), 'clock %.3f' % clock(), 'pos %.2f %.2f %.2f' % pos, flush=True)
p.save_state(SLOT)
time.sleep(3.0)
print('saved', SLOT, flush=True)
