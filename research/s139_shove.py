"""S139: a bike too fast for its body rays, made by hand — research/s150_spin.py's throw without the
spin: the chassis sweep back's case.

usage: python3 -B research/s139_shove.py PID STATE VX VY VZ SAVE

Run it under `./kit pcsx2 with --` on a copy started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
QT_ACCESSIBILITY=1 (the pause is pressed over AT-SPI). Loads STATE (the bike standing, research/s76_tele.py's),
pauses, sets the chassis' velocity to (VX, VY, VZ) in world space in every copy (+0x260, +0x2d0,
+0x2dc), and saves into SAVE, still paused. The chassis is clamped to 135 ft/s (the riding bike's max
speed) on its next integrate, and its tires drag it along the ground: VY lifts it off. Then `research/s138_hits.py NAME --watch
research/s139/sweep.toml --state SAVE ...` records the run from that state."""
import struct, sys, time
sys.path.insert(0, '.')
from ps2kit.config import cfg
sys.path.insert(0, 'research/journal/2026-10-20-c6-c6-recording2-capture/playback')
from ps2kit.pine import Pine
import atspi_lib as A
from gi.repository import Atspi

PID, STATE = int(sys.argv[1]), int(sys.argv[2])
V = [float(a) for a in sys.argv[3:6]]
SAVE = int(sys.argv[6])
app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])
p = Pine()
FC = cfg().symbols['frame_counter']


def pause(on):
    if item.get_state_set().contains(Atspi.StateType.CHECKED) != on:
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
v = V
for o in (0x260, 0x2d0, 0x2dc):
    put(ch + o, v)
print('frame', p.read32(FC), 'cofg', v3(ch + 0x60), 'vel', v3(ch + 0x260), flush=True)
p.save_state(SAVE)
t = time.time()
while time.time() - t < 2.0:
    pass
print('saved', SAVE, flush=True)
