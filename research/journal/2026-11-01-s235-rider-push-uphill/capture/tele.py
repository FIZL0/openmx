"""S235: move the paused human bike's centre of gravity to (X, Y, Z), let it settle, save a state.

    ./kit pcsx2 with -- python3 -B research/journal/2026-11-01-s235-rider-push-uphill/capture/tele.py PID load8
    ./kit pcsx2 with -- python3 -B .../tele.py PID X Y Z FRAMES SLOT

`load8` loads slot 8 (first_recording, put there by S106's setup.py), pauses over AT-SPI and prints
the cofg. The other form is P21's tele2.py: every float triple within 12 ft of the cofg in the bike,
chassis, actor and transform blocks moves by the same delta, the velocities are zeroed, the VM runs
FRAMES game frames with no input, pauses again and saves to SLOT. No pad: /dev/uinput is root-only."""
import struct, sys, time
sys.path.insert(0, '.')
sys.path.insert(0, 'research/journal/2026-10-20-c6-c6-recording2-capture/playback')
from ps2kit.pine import Pine
import atspi_lib as A
from gi.repository import Atspi

PID = int(sys.argv[1])
app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])
FC = 0x479d04
GM = 0x004e1e98
p = Pine()


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


def pause(on):
    if paused() != on:
        A.press(item)
    time.sleep(0.5)


def chain(hops):
    a = p.read32(GM)
    for h in hops:
        a = p.read32(a + h)
    return a


def objs():
    veh = chain([0x34, 0x34] + [0x8] * 8 + [0x0])
    bike = chain([0x34, 0x34, 0x8, 0x8, 0x0])
    return veh, bike, p.read32(bike + 0x7a4)


if sys.argv[2] == 'load8':
    pause(False)
    p.load_state(8)
    time.sleep(6.0)
    pause(True)
    f0 = p.read32(FC)
    time.sleep(0.3)
    assert p.read32(FC) == f0, 'not paused'
    veh, bike, ch = objs()
    print('frame', f0, 'cofg', struct.unpack('<3f', p.read_block(ch + 0x60, 12)),
          'quat', struct.unpack('<4f', p.read_block(ch + 0x70, 16)))
    sys.exit()

X, Y, Z = (float(v) for v in sys.argv[2:5])
FRAMES, SLOT = int(sys.argv[5]), int(sys.argv[6])
pause(True)
veh, bike, ch = objs()
cw = struct.unpack('<3f', p.read_block(ch + 0x60, 12))
d = (X - cw[0], Y - cw[1], Z - cw[2])
print('frame', p.read32(FC), 'cofg', cw, 'delta', d)
regions = [(bike, 0x2000), (ch, 0x400), (veh, 0x800), (p.read32(bike + 0x18) - 0x1600, 0x1800)]
for o in (0x7b0, 0x7b4):
    regions.append((p.read32(bike + o), 0x900))
moved = 0
for base, n in regions:
    b = p.read_block(base, n)
    off = 0
    while off + 12 <= n:
        v = struct.unpack_from('<3f', b, off)
        if all(abs(v[k] - cw[k]) < 12 for k in range(3)):
            for k in range(3):
                p.write_f32(base + off + 4 * k, v[k] + d[k])
            moved += 1
            off += 12
        else:
            off += 4
print('moved', moved, 'triples')
for o in (0x260, 0x280, 0x2b8, 0x2d0, 0x310):
    for k in range(3):
        p.write_f32(ch + o + 4 * k, 0.0)
for k in range(3):
    p.write_f32(veh + 0x130 + 4 * k, 0.0)
f0 = p.read32(FC)
pause(False)
deadline = time.time() + 30
while p.read32(FC) < f0 + FRAMES and time.time() < deadline:
    time.sleep(0.02)
pause(True)
print('paused at', p.read32(FC), 'cofg %.2f %.2f %.2f' % struct.unpack('<3f', p.read_block(ch + 0x60, 12)),
      'vel %.3f %.3f %.3f' % struct.unpack('<3f', p.read_block(ch + 0x260, 12)))
p.save_state(SLOT)
time.sleep(2.0)
print('saved', SLOT)
