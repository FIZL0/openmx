#!/usr/bin/env python3
"""S143: a pilot_watch.py log as the csv crates/game-sim/tests/plane_crash_pilot.rs reads.

    python3 -B to_csv.py context/s143/nose.txt context/capture/s143/nose.csv

One row a tick. The plane's rigid body as float bit patterns (hex): last position, position, velocity,
last rotation, rotation, rates; the wreck word, latch and clock; grounded, airspeed; the plane's pad
(+0x338 forward, +0x33c back, +0x340 elevator, +0x344 aileron, +0x348 rudder) and throttle (+0x34c);
then the pilot's 22 joint nodes' world positions in the plane model's root frame (*(+0x18c)'s world
rows), decimals. Research, not product.
"""

import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pilot_read import ticks  # noqa: E402


def words(h):
    return [h[i:i + 8] for i in range(0, len(h), 8)]


def bits(w):
    return struct.unpack('<I', bytes.fromhex(w))[0]


def f(w):
    return struct.unpack('<f', bytes.fromhex(w))[0]


def main():
    src, out = sys.argv[1], sys.argv[2]
    os.makedirs(os.path.dirname(out), exist_ok=True)
    cols = ['frame', 'dt', 'wreck', 'latch', 'clock', 'grounded', 'airspeed']
    for name, n in [('last_pos', 3), ('pos', 3), ('vel', 3), ('last_quat', 4), ('quat', 4), ('rates', 3)]:
        cols += [f'{name}.{"xyzw"[k]}' for k in range(n)]
    cols += ['forward', 'back', 'elevator', 'aileron', 'rudder', 'throttle']
    cols += [f'bone{j}.{a}' for j in range(22) for a in 'xyz']
    with open(out, 'w') as o:
        o.write(','.join(cols) + '\n')
        for t in ticks(src):
            w = words(t['plane.3b0'])
            g = words(t['plane.590'])
            r50 = words(t['rigid.50'])
            r150 = words(t['rigid.150'])
            pad = words(t['plane.330'])
            row = [str(t['f']), f'{bits(t["dt"]):08x}', str(bits(w[0])), str(bits(w[1])), f'{bits(w[3]):08x}',
                   str(bits(g[0])), f'{bits(g[2]):08x}']
            row += [f'{bits(x):08x}' for x in r50[0:3] + r50[4:7] + r50[8:11]]
            row += [f'{bits(x):08x}' for x in r150[0:4] + r150[8:12] + r150[24:27]]
            row += [repr(f(x)) for x in pad[2:8]]
            m = [f(x) for x in words(t['model.node'])]
            ax, o0 = [m[0:3], m[4:7], m[8:11]], m[12:15]
            for j in range(22):
                q = [f(x) for x in words(t[f'node{j}'])][44:47]
                d = [q[k] - o0[k] for k in range(3)]
                row += ['%.4f' % sum(d[k] * ax[r][k] for k in range(3)) for r in range(3)]
            o.write(','.join(row) + '\n')


if __name__ == '__main__':
    main()
