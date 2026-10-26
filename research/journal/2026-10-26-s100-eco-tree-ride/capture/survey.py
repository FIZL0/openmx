"""S100 survey: put recording2's state in scratch slot 8, load it, pause, and print the bike's place,
its chassis' orthonormal 3x3 blocks (to find the heading) and free09's trees nearest it.
Usage: survey.py PID"""
import math, shutil, struct, sys, time
sys.path.insert(0, '.')
sys.path.insert(0, 'research/journal/2026-10-20-c6-c6-recording2-capture/playback')
from ps2kit.pine import Pine
import atspi_lib as A
from gi.repository import Atspi

PID = int(sys.argv[1])
SAVES = '/home/rycs/repos/openmx-slots/s2-slots/pcsx2/s2/PCSX2/savestates/'
shutil.copyfile('context/recordings/recording2.p2m2_SaveState.p2s', SAVES + 'SLUS-20767 (9866FE54).08.p2s')
app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])
def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)

p = Pine()
if paused():
    A.press(item)
p.load_state(8)
time.sleep(3)
if not paused():
    A.press(item)
time.sleep(0.5)
f0 = p.read32(0x479d04); time.sleep(0.3)
assert p.read32(0x479d04) == f0, 'not paused'
GM = 0x004e1e98
def chain(hops):
    a = p.read32(GM)
    for h in hops:
        a = p.read32(a + h)
    return a
bike = chain([0x34, 0x34, 0x8, 0x8, 0x0])
ch = p.read32(bike + 0x7a4)
cw = struct.unpack('<3f', p.read_block(ch + 0x60, 12))
print('frame', f0, 'bike', hex(bike), 'chassis', hex(ch), 'cofg', cw)
blk = p.read_block(ch, 0x400)
for off in range(0, 0x400 - 44, 16):
    m = [struct.unpack_from('<3f', blk, off + 16 * r) for r in range(3)]
    ok = all(abs(sum(x * x for x in r) - 1) < 1e-3 for r in m) and abs(sum(a * b for a, b in zip(m[0], m[1]))) < 1e-3
    if ok:
        print('rows at chassis+%#x' % off, ['(%.4f %.4f %.4f)' % r for r in m])
world = p.read32(0x0046a5f8)
cell = p.read_f32(world + 0xd4)
species = p.read32(world + 0x94)
n = p.read32(world + 0x64)
plants = p.read_block(p.read32(world + 0x68), 0x1c * n)
trees = []
for i in range(n):
    c = struct.unpack_from('<3H', plants, 0x1c * i + 8)
    s = plants[0x1c * i + 0xe]
    sp = p.read32(species + 4 * s)
    if not sp or p.read32(sp + 0x194) == 0:
        continue
    r = p.read32(sp + 0x198)
    rad = p.read_f32(r + 0x24)
    name = p.read_block(sp, 32).split(b'\0')[0].decode()
    x, y, z = (v * cell for v in c)
    d = math.hypot(x - cw[0], z - cw[2])
    trees.append((d, i, name, rad, x, y, z, plants[0x1c * i + 0x10], plants[0x1c * i + 0x11]))
trees.sort()
print(len(trees), 'trees of', n, 'plants; cell', cell)
for t in trees[:40]:
    print('%7.1f ft  plant %4d %-12s r %.2f at (%.2f, %.2f, %.2f) h %d w %d' % t)
