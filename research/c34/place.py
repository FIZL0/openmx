"""C34: put the car's body at a place and turn, still, and let it settle.

usage: place.py CAR X Y Z QX QY QZ QW

Over PINE while the game runs (no pad here to pause it): the body's position (+0x60) and last position
(+0x50) become (X, Y, Z), its turn (+0x170) and last turn (+0x150) the quaternion, and every speed,
push and spin (+0x70, +0x90, +0x1b0, +0x190, +0x1e0) is cleared, written again over four frames so
the integrator does not undo them (flip.py's way). The caller saves once the car stands still.
"""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

car = int(sys.argv[1], 0)
to = [float(v) for v in sys.argv[2:5]] + [1.0]
q = [float(v) for v in sys.argv[5:9]]
p = Pine(check=False, lock=False, timeout=5.0)
body = p.read32(car + 0x160)
f0 = p.frame()
while p.frame() < f0 + 4:
    for off, v in ((0x170, q), (0x150, q), (0x60, to), (0x50, to)):
        for i in range(4):
            p.write_f32(body + off + 4 * i, v[i])
    for off in (0x70, 0x90, 0x1b0, 0x190, 0x1e0):
        for i in range(3):
            p.write_f32(body + off + 4 * i, 0.0)
    time.sleep(0.01)
print('body', hex(body), 'placed over frames', f0, p.frame(), flush=True)
