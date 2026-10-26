"""S150: a crooked landing made by hand — research/s138_nose.py with the rate about the bike's own up
axis (node row 1, body y) instead of its right axis: the bike yaws in the air and lands turned.

usage: python3 -B research/s150_spin.py PID STATE UP RATE SAVE [FWD] [WATCH_S]

Run it under `./kit pcsx2 with --` on a copy started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
QT_ACCESSIBILITY=1 (the pause is pressed over AT-SPI). Loads STATE (the bike standing), pauses, gives
the chassis an upward velocity UP and FWD along its own forward axis in every copy, runs until a tick
reads airborne, pauses, gives it a yaw rate RATE about its own up axis (node row 1, body y),
saves into SAVE, resumes and watches. Then `research/s138_hits.py NAME --state SAVE ...` records the
landing from that state."""
import math, struct, sys, time
sys.path.insert(0, '.')
from ps2kit.config import cfg
sys.path.insert(0, 'research/journal/2026-10-20-c6-c6-recording2-capture/playback')
from ps2kit.pine import Pine
import atspi_lib as A
from gi.repository import Atspi

PID, STATE, UP, RATE, SAVE = int(sys.argv[1]), int(sys.argv[2]), float(sys.argv[3]), float(sys.argv[4]), int(sys.argv[5])
FWD = float(sys.argv[6]) if len(sys.argv) > 6 else 0.0
WATCH = float(sys.argv[7]) if len(sys.argv) > 7 else 4.0
app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])
p = Pine()
FC = cfg().symbols['frame_counter']


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


def pause(on):
    if paused() != on:
        A.press(item)
    t = time.time()
    while time.time() - t < 0.4:
        pass


def v3(a): return struct.unpack('<3f', p.read_block(a, 12))


def put(a, v):
    for k in range(3):
        p.write_f32(a + 4 * k, v[k])


p.load_state(STATE)
t = time.time()
while time.time() - t < 3.0:
    pass
pause(True)
rider = p.read32(cfg().symbols['game_manager'])
for off in (0x34, 0x34, 0x8, 0x8, 0x0):
    rider = p.read32(rider + off)
ch = p.read32(rider + 0x7a4)
node = p.read32(ch + 0x254)
fwd = v3(node + 0xa0)
print('frame', p.read32(FC), 'rider', hex(rider), 'chassis', hex(ch), 'fwd', fwd, flush=True)
v = [c + f * FWD for c, f in zip(v3(ch + 0x260), fwd)]; v[1] += UP
for o in (0x260, 0x2d0, 0x2dc):
    put(ch + o, v)
pause(False)
t0 = time.time()
while p.read32(rider + 0x57c) == 0 and time.time() - t0 < 3:
    pass
pause(True)
f1 = p.read32(FC)
up = v3(node + 0x90)
print('airborne at', f1, 'vel', v3(ch + 0x260), 'w', v3(ch + 0x280), flush=True)
w = [x + r * RATE for x, r in zip(v3(ch + 0x280), up)]
wb = list(v3(ch + 0x290)); wb[1] += RATE
put(ch + 0x280, w); put(ch + 0x290, wb); put(ch + 0x310, w); put(ch + 0x31c, w)
p.save_state(SAVE)
t = time.time()
while time.time() - t < 2.0:
    pass
print('saved', SAVE, flush=True)
pause(False)
t0 = time.time()
while time.time() - t0 < WATCH:
    f = v3(node + 0xa0)
    print('%5d heading %7.1f fwd %6.3f %6.3f %6.3f  w %6.2f %6.2f %6.2f  vy %6.2f  air %d  reason %d' % (
        (p.read32(FC), math.degrees(math.atan2(f[0], f[2]))) + f + v3(ch + 0x280) + (v3(ch + 0x260)[1], p.read32(rider + 0x57c),
                                                             p.read32(rider + 0x748))), flush=True)
    t = time.time()
    while time.time() - t < 0.1:
        pass
