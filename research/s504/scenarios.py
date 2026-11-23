"""S504: the pilot's policies (research/s504/pilot.py): each takes the tick and both riders and gives
both pads, (gas, brake, stick x, stick y). Heading 0 is +z, and a right turn grows it. Research."""
import math


def wrap(a):
    return (a + math.pi) % (2 * math.pi) - math.pi


def steer(r, x, z, gain=2.0):
    return round(max(-1, min(1, gain * wrap(math.atan2(x - r.pos[0], z - r.pos[2]) - r.heading))), 2)


def pedal(r, want):
    if r.speed > want + 12:
        return 0, 1
    return (1.0, 0) if r.speed < want - 4 else (0.35, 0) if r.speed < want + 4 else (0, 0)


def ride(r, x, z, want, gain=2.0):
    return (*pedal(r, want), steer(r, x, z, gain), 0)


def lead(a, b):
    """Where `b` will be when the two meet, by their closing speed."""
    gap = math.dist(a.pos, b.pos)
    closing = -sum((q - p) * (w - v) for p, q, v, w in zip(a.pos, b.pos, a.vel, b.vel)) / max(gap, 0.1)
    t = min(gap / max(closing, 5), 1)
    return b.pos[0] + b.vel[0] * t, b.pos[2] + b.vel[2] * t


def head_on(t, a, b):
    """P1 turns round on the spot while P2 comes over the hill; then each at the other."""
    gap = math.dist(a.pos, b.pos)
    if t > 285:
        return ride(a, *lead(a, b), 35, 5), ride(b, *lead(b, a), 45, 5)
    return ride(a, b.pos[0], b.pos[2], 22 if gap > 200 else 35), ride(b, a.pos[0], a.pos[2], 45)


def landing(t, a, b):
    """P1 holds its place in the dip beyond the rise; P2 takes the rise at a speed that comes down there."""
    hold = (0, 0, 0, 0) if a.pos[2] + a.vel[2] > 3654 else (0.5, 0, 0, 0)
    if b.pos[2] > 3545:
        return hold, (0, 0, 0, 0)
    return hold, ride(b, 4308, b.pos[2] + 40, 66, 3)


def swipe(t, a, b):
    """P2 comes up on P1's left at a little more speed, then both lean together."""
    dz = a.pos[2] - b.pos[2]
    if dz > 25:
        want = 8 if dz > 90 else 30
        return ride(a, 4316, a.pos[2] + 60, want, 3), ride(b, a.pos[0] - 7, a.pos[2] + 20, 50, 4)
    if dz > 1.5:
        side = 6 if dz > 10 else 5
        return ride(a, 4316, a.pos[2] + 60, 38, 3), ride(b, a.pos[0] - side, b.pos[2] + 40, 47, 4)
    return ride(a, b.pos[0], a.pos[2] + 25, 42, 3), ride(b, a.pos[0], b.pos[2] + 25, 42, 3)
