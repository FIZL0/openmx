"""C34: turn the parked car on its roof, a few feet up, and let it fall and settle.

usage: flip.py CAR [UP]

Over PINE while the game runs (no pad here to pause it): the body's turn (+0x170) and its last turn
(+0x150) become the turn rolled half round about the car's own forward axis (local z), the position
(+0x60) and last position (+0x50) go UP feet higher (default 4), and every speed, push and spin is
cleared. Writes are repeated over a few frames so the integrator does not undo them, as tele2.py
found for the bike. The caller saves the state once the car lies still.
"""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

car = int(sys.argv[1], 0)
up = float(sys.argv[2]) if len(sys.argv) > 2 else 4.0
p = Pine(check=False, lock=False, timeout=5.0)
body = p.read32(car + 0x160)


def quat(a):
    return [p.read_f32(a + 4 * i) for i in range(4)]


def mul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return [aw * bx + ax * bw + ay * bz - az * by,
            aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw,
            aw * bw - ax * bx - ay * by - az * bz]


q = quat(body + 0x170)
flipped = mul(q, [0.0, 0.0, 1.0, 0.0])
pos = quat(body + 0x60)
to = [pos[0], pos[1] + up, pos[2], pos[3]]
print('body', hex(body), 'quat', q, '->', flipped, 'pos', pos, '->', to, flush=True)
f0 = p.frame()
while p.frame() < f0 + 4:
    for off, v in ((0x170, flipped), (0x150, flipped), (0x60, to), (0x50, to)):
        for i in range(4):
            p.write_f32(body + off + 4 * i, v[i])
    for off in (0x70, 0x90, 0x1b0, 0x190, 0x1e0):
        for i in range(3):
            p.write_f32(body + off + 4 * i, 0.0)
    time.sleep(0.01)
print('written over frames', f0, p.frame(), flush=True)
