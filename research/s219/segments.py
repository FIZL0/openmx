"""S219: where a path car drives straight across a heading, so a bike facing that heading can meet its flank.

usage: python3 -B research/s219/segments.py IMAGE CAR_HEX BIKE_HEADING_DEG [MIN_SECONDS]

For the car at CAR_HEX (`research/s207/cars.py` lists them) in the 32 MiB EE RAM IMAGE, prints each run
of its path's keys at least MIN_SECONDS of game time long (default 3) whose flat heading stays within 20°
of square to BIKE_HEADING_DEG and whose ground rises less than 0.1 ft a foot: the key times (clock), the
game seconds (clock / time scale), the start and end points, the car's speed and heading.
"""
import math
import struct
import sys

ram = open(sys.argv[1], 'rb').read()
CAR = int(sys.argv[2], 16)
BIKE = math.radians(float(sys.argv[3]))
LONG = float(sys.argv[4]) if len(sys.argv) > 4 else 3.0


def w(a):
    return struct.unpack_from('<I', ram, a)[0]


def f(a):
    return struct.unpack_from('<f', ram, a)[0]


path = w(CAR + 0x430)
n, keys, scale = w(path), w(path + 8), f(CAR + 0x534)
k = [(f(keys + i * 0x30), struct.unpack_from('<3f', ram, keys + i * 0x30 + 0x10)) for i in range(n)]


def good(i):
    (ta, a), (tb, b) = k[i], k[i + 1]
    dx, dy, dz = (b[j] - a[j] for j in range(3))
    run = math.hypot(dx, dz)
    if run < 1e-3:
        return False
    cross = abs(math.sin(math.atan2(dx, dz) - BIKE))
    return cross > math.cos(math.radians(20)) and abs(dy) / run < 0.1


i = 0
while i < n - 1:
    if not good(i):
        i += 1
        continue
    j = i
    while j < n - 1 and good(j):
        j += 1
    (ta, a), (tb, b) = k[i], k[j]
    if (tb - ta) / scale >= LONG:
        d = math.hypot(b[0] - a[0], b[2] - a[2])
        print('keys %4d..%4d clock %7.2f..%7.2f (%5.1f s) from %7.1f %6.1f %7.1f to %7.1f %6.1f %7.1f  %5.1f ft/s heading %4.0f' % (
            i, j, ta, tb, (tb - ta) / scale, *a, *b, d / ((tb - ta) / scale),
            math.degrees(math.atan2(b[0] - a[0], b[2] - a[2]))))
    i = j
