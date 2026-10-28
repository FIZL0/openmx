"""S170: stand the bike on free06's rails in the train's way in the original, and log, frame by frame, what
the train's nose does to it: the wreck reason it enters, whether its search holds the cars, and the
collision record's own entries (who the other and the mover are).

usage: python3 -B research/s170/nose.py PID LOAD_SLOT X Y Z FRAMES [SAVE_SLOT]

From S129's across state (copy 2's slot 10: the bike has ridden since its put-back, so it is no ghost and
its search gathers the cars), pause at once and move the bike (research/s129/tele.py's teleport: every copy
of its position moved by one delta, the chassis' velocities zeroed, the heading kept) to X Y Z, hold both
brakes (square and r2) and run FRAMES frames, one log line a frame. SAVE_SLOT, if given, gets a state
60 frames after the teleport (the bike settled, the train still far). Run it under `./kit pcsx2 with --`.
"""

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
X, Y, Z = (float(v) for v in sys.argv[3:6])
FRAMES = int(sys.argv[6])
SAVE = int(sys.argv[7]) if len(sys.argv) > 7 else None
ENGINE = 0x00d525f0
SYM = cfg().symbols
FRAME = SYM['frame_counter']
MANAGER = SYM['game_manager']

app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


def wait(seconds):
    end = time.time() + seconds
    while time.time() < end:
        time.sleep(0.05)


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


def f3(a):
    return struct.unpack('<3f', p.read_block(a, 12))


def clock():
    return struct.unpack('<f', p.read_block(p.read32(ENGINE + 0x430) + 0xc, 4))[0]


def engine():
    return f3(p.read32(ENGINE + 0x254) + 0xb0)


if not paused():
    A.press(item)
wait(0.5)
subprocess.run([str(ROOT / 'kit'), 'pad', 'send', 'down square', 'down r2'], check=False)
p.load_state(LOAD)
wait(2.0)
if not paused():
    A.press(item)
wait(0.5)
f0 = p.read32(FRAME)
wait(0.3)
assert p.read32(FRAME) == f0, 'not paused'
veh = human(4)
bike = human(2)
ch = p.read32(bike + 0x7a4)
vp = f3(veh + 0x1b0)
d = (X - vp[0], Y - vp[1], Z - vp[2])
print('frame', f0, 'clock', clock(), 'engine', engine(), 'veh', hex(veh), 'bike', hex(bike), 'chassis', hex(ch),
      'pos', vp, 'delta', d, 'ghost state', p.read32(bike + 0xb7c), flush=True)
regions = [(bike, 0x2000), (ch, 0x400), (veh, 0x800), (p.read32(bike + 0x18) - 0x1600, 0x1800)]
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
q = struct.unpack('<4f', p.read_block(ch + 0x0, 16))
print('chassis head words', q, 'veh matrix rows', [f3(veh + 0x180 + 16 * r) for r in range(3)], flush=True)


def search():
    """The bike's nearby search (+0x7bc): candidates, and how many are category 0x48 (a car, a toy)."""
    s = p.read32(bike + 0x7bc)
    if not ok(s):
        return (-1, -1)
    lst, n = p.read32(s), p.read32(s + 4)
    cars = 0
    for i in range(min(n, 0x100)):
        o = p.read32(lst + 8 * i)
        if ok(o) and p.read32(o + 0x84) == 0x48:
            cars += 1
    return (n, cars)


def record():
    """The bike's collision record's entries: (kind, reach y, other, mover) for each."""
    r = p.read32(bike + 0x7d0)
    if not ok(r):
        return 'record?'
    n = p.read32(r + 0x100)
    lst = p.read32(r + 0x108)
    out = []
    for i in range(min(n, 16)):
        e = lst + 0x80 * i
        out.append('%d/%.2f/%x/%x' % (p.read32(e), struct.unpack('<f', p.read_block(e + 0x24, 4))[0],
                                       p.read32(e + 0x74), p.read32(e + 0x78)))
    return '%d[%s]' % (n, ' '.join(out))


A.press(item)
last = p.read32(FRAME)
start = last
saved = SAVE is None
while last - start < FRAMES:
    f = p.read32(FRAME)
    if f == last:
        time.sleep(0.005)
        continue
    last = f
    pos = f3(veh + 0x1b0)
    vel = f3(ch + 0x260)
    print('%5d clk %7.3f eng %8.2f %7.2f %8.2f pos %8.2f %7.2f %8.2f vel %7.2f %7.2f %7.2f reason %d ghost %d '
          'search %d/%d rec %s' % ((f, clock()) + engine() + pos + vel +
                                    (p.read32(bike + 0x748), p.read32(bike + 0xb7c)) + search() + (record(),)),
          flush=True)
    if not saved and f - start >= 60:
        A.press(item)
        wait(0.5)
        print('saving at frame', p.read32(FRAME), 'clock', clock(), flush=True)
        p.save_state(SAVE)
        wait(2.0)
        A.press(item)
        saved = True
A.press(item)
print('paused at frame', p.read32(FRAME), flush=True)
subprocess.run([str(ROOT / 'kit'), 'pad', 'send', 'release'], check=False)
