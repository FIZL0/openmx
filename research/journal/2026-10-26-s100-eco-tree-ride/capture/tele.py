"""S100: P21's tele2.py with a braked settle (S89's tele.py). Pause, move the bike's chassis (and every copy of its position the frame reads) by one delta, then
resume and watch. Usage: tele2.py PID X Y Z SECONDS [save-slot]"""
import struct, subprocess, sys, time, os
sys.path.insert(0, '.')
sys.path.insert(0, 'research/journal/2026-10-20-c6-c6-recording2-capture/playback')
from ps2kit.pine import Pine
import atspi_lib as A
from gi.repository import Atspi

PID = int(sys.argv[1])
X, Y, Z = (float(v) for v in sys.argv[2:5])
SECS = float(sys.argv[5])
app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])
def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)

p = Pine()
GM = 0x004e1e98
def chain(hops):
    a = p.read32(GM)
    for h in hops:
        a = p.read32(a + h)
    return a

if not paused():
    A.press(item)
time.sleep(0.5)
f0 = p.read32(0x479d04); time.sleep(0.3)
assert p.read32(0x479d04) == f0, 'not paused'
veh = chain([0x34, 0x34] + [0x8] * 8 + [0x0])
bike = chain([0x34, 0x34, 0x8, 0x8, 0x0])
ch = p.read32(bike + 0x7a4)
cw = struct.unpack('<3f', p.read_block(ch + 0x60, 12))
d = (X - cw[0], Y - cw[1], Z - cw[2])
print('frame', f0, 'cofg_world', cw, 'delta', d)
# Every float triple within 12 ft of the cofg in the bike, chassis and actor objects, and in the
# transform object, gets the same delta.
regions = [(bike, 0x2000), (ch, 0x400), (veh, 0x800), (p.read32(bike + 0x18) - 0x1600, 0x1800)]
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
print('cofg now', struct.unpack('<3f', p.read_block(ch + 0x60, 12)))
for k in range(3):
    p.write_f32(veh + 0x130 + 4 * k, 0.0)
subprocess.run(['./kit', 'pad', 'send', 'down square'])
A.press(item)
t0 = time.time()
while time.time() - t0 < SECS:
    pos = struct.unpack('<3f', p.read_block(veh + 0x1b0, 12))
    vel = struct.unpack('<3f', p.read_block(ch + 0x260, 12))
    print('%5d pos %9.2f %8.2f %9.2f vel %7.2f %7.2f %7.2f' % ((p.read32(0x479d04),) + pos + vel), flush=True)
    time.sleep(0.25)
subprocess.run(['./kit', 'pad', 'send', 'release'])
time.sleep(1.0)
if len(sys.argv) > 6:
    if not paused():
        A.press(item)
    time.sleep(0.5)
    p.save_state(int(sys.argv[6]))
    time.sleep(2.0)
    print('saved', sys.argv[6], 'at frame', p.read32(0x479d04))
    for r in range(4):
        print('xform', struct.unpack('<4f', p.read_block(veh + 0x180 + 16 * r, 16)))
