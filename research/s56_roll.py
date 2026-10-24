"""S56: the slowest wreck the original has, made by hand. A ridden bike cannot tip over on the ground (its
roll freedom is off while a wheel is down, research/oracle.txt S56), so this throws the standing bike up
and rolls it in the air past the airborne roll limit (reason 8).

usage: python3 -B research/s56_roll.py PID STATE UP RATE SAVE [WATCH_S]

Run it under `./kit pcsx2 with --` on a copy started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
QT_ACCESSIBILITY=1 (the pause is pressed over AT-SPI). The s62 "roll" capture came from
`... 7 20 18 5` (recording2's state in slot 7), then `research/s62_wreck.py roll --pad
research/s56/roll.pad --state 5 --frames 700`.

PID STATE UP RATE SAVE [WATCH_S] — load STATE (the bike standing), pause, give the chassis an
upward velocity UP in every copy, run until a tick reads airborne, pause, give it a roll rate RATE about
its own forward axis (free in the air), save into SAVE, resume and watch."""
import struct, sys, time
sys.path.insert(0, '.')
from ps2kit.config import cfg
sys.path.insert(0, 'research/journal/2026-10-20-c6-c6-recording2-capture/playback')
from ps2kit.pine import Pine
import atspi_lib as A
from gi.repository import Atspi

PID, STATE, UP, RATE, SAVE = int(sys.argv[1]), int(sys.argv[2]), float(sys.argv[3]), float(sys.argv[4]), int(sys.argv[5])
WATCH = float(sys.argv[6]) if len(sys.argv) > 6 else 6.0
app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])
p = Pine()
FC = cfg().symbols['frame_counter']


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


def pause(on):
    if paused() != on:
        A.press(item)
    time.sleep(0.4)


def v3(a): return struct.unpack('<3f', p.read_block(a, 12))


def put(a, v):
    for k in range(3):
        p.write_f32(a + 4 * k, v[k])


p.load_state(STATE)
time.sleep(3.0)
pause(True)
rider = p.read32(cfg().symbols['game_manager'])
for off in (0x34, 0x34, 0x8, 0x8, 0x0):
    rider = p.read32(rider + off)
ch = p.read32(rider + 0x7a4)
node = p.read32(ch + 0x254)
print('frame', p.read32(FC), 'rider', hex(rider), 'chassis', hex(ch), 'roll limit', p.read_f32(rider + 0x778),
      'flags', struct.unpack('<3I', p.read_block(ch + 0x2ac, 12)))
v = list(v3(ch + 0x260)); v[1] += UP
for o in (0x260, 0x2d0, 0x2dc):
    put(ch + o, v)
pause(False)
t0 = time.time()
while p.read32(rider + 0x57c) == 0 and time.time() - t0 < 3:
    pass
pause(True)
f1 = p.read32(FC)
fwd = v3(node + 0xa0)
print('airborne at', f1, 'flags', struct.unpack('<3I', p.read_block(ch + 0x2ac, 12)), 'vel', v3(ch + 0x260), 'w', v3(ch + 0x280))
w = [x + f * RATE for x, f in zip(v3(ch + 0x280), fwd)]
wb = list(v3(ch + 0x290)); wb[2] += RATE
put(ch + 0x280, w); put(ch + 0x290, wb); put(ch + 0x310, w); put(ch + 0x31c, w)
p.save_state(SAVE)
time.sleep(2.0)
print('saved', SAVE)
pause(False)
t0 = time.time()
while time.time() - t0 < WATCH:
    print('%5d up %6.3f %6.3f %6.3f  w %6.2f %6.2f %6.2f  speed %6.2f  air %d  euler %s  reason %d' % (
        (p.read32(FC),) + v3(node + 0x90) + v3(ch + 0x280) + (sum(c * c for c in v3(ch + 0x260)) ** 0.5,)
        + (p.read32(rider + 0x57c), '%.2f %.2f %.2f' % v3(ch + 0x90), p.read32(rider + 0x748))), flush=True)
    time.sleep(0.2)
