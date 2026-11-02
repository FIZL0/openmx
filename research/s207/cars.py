"""S207: list a RAM image's path cars: address, name, clock, time scale, lag, where the model stands, and
the path's key span. With `at X Z` also, for each car, the key time its path passes nearest that ground
point, how near, and the speed there.

usage: python3 -B research/s207/cars.py IMAGE [at X Z]

IMAGE is a whole 32 MiB EE RAM image (`./kit state ram`, or `python3 -B research/s162/slots.py`). Path
cars are found by their vtable at +8 (research/oracle.txt `path_car_vtable`); fields as S81's
(crates/game-sim/tests/traffic.rs `mod at`).
"""
import math
import struct
import sys

VTABLE = 0x0044d370
ram = open(sys.argv[1], 'rb').read()
AT = (float(sys.argv[3]), float(sys.argv[4])) if len(sys.argv) > 4 and sys.argv[2] == 'at' else None


def w(a):
    return struct.unpack_from('<I', ram, a)[0]


def f(a):
    return struct.unpack_from('<f', ram, a)[0]


def ok(a):
    return 0x100000 <= a < 0x2000000


def name(a):
    return ram[a:a + 64].split(b'\0')[0].decode('latin1')


def key(keys, i):
    return f(keys + i * 0x30), struct.unpack_from('<3f', ram, keys + i * 0x30 + 0x10)


cars = [a for a in range(0x100000, 0x2000000 - 0x600, 16)
        if w(a + 8) == VTABLE and ok(w(a + 0x430)) and ok(w(a + 0x428))]
for k, car in enumerate(cars):
    path = w(car + 0x430)
    n, keys, clock = w(path), w(path + 8), f(path + 0xc)
    scale, lag = f(car + 0x534), f(car + 0x558)
    pos = struct.unpack_from('<3f', ram, w(car + 0x254) + 0x80 + 0x30)
    line = '%2d %08x %-22s clock %8.3f scale %.3f lag %7.2f at %8.1f %6.1f %8.1f keys %d %.2f..%.2f' % (
        k, car, name(car + 0x320), clock, scale, lag, *pos, n, key(keys, 0)[0], key(keys, n - 1)[0])
    if AT:
        best = None
        for i in range(n - 1):
            (ta, pa), (tb, pb) = key(keys, i), key(keys, i + 1)
            dx, dz = pb[0] - pa[0], pb[2] - pa[2]
            u = max(0.0, min(1.0, ((AT[0] - pa[0]) * dx + (AT[1] - pa[2]) * dz) / (dx * dx + dz * dz or 1.0)))
            d = math.hypot(pa[0] + u * dx - AT[0], pa[2] + u * dz - AT[1])
            if best is None or d < best[0]:
                best = (d, ta + u * (tb - ta), pa[1] + u * (pb[1] - pa[1]), math.hypot(dx, dz) / max(tb - ta, 1e-6),
                        math.degrees(math.atan2(dx, dz)))
        d, t, y, v, h = best
        line += ' | nearest %.1f ft at key time %.2f y %.1f, %.1f ft/s heading %.0f' % (d, t, y, v * scale, h)
    print(line)
