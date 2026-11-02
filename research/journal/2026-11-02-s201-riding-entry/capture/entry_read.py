#!/usr/bin/env python3
"""S201: print the rider body's parts round one tick of an entry_watch.py capture.

    entry_read.py CAPTURE FRAME [PARTS]     e.g. entry_read.py context/s201/free09-ride.txt 3137 0,8

For each tick FRAME-2..FRAME+6: the body's mode, the ragdoll's riding/repose/placed/active/stiff words,
the request's bike velocity, and for each part its body's position, its move since the last position,
its velocity and its rates. Research, not product.
"""

import struct
import sys


def rows(path):
    out, cur = [], None
    for line in open(path):
        if line.startswith('#'):
            continue
        if line.startswith('f '):
            cur = {'frame': int(line[2:])}
            out.append(cur)
            continue
        k, v = line.split(' ', 1)
        cur[k] = bytes.fromhex(v.strip())
    return out


def word(b, o):
    return struct.unpack_from('<I', b, o)[0]


def quad(b, o):
    return struct.unpack_from('<4f', b, o)


def fmt(v):
    return ' '.join(f'{x:7.3f}' for x in v[:3])


def main():
    rs = rows(sys.argv[1])
    t = int(sys.argv[2])
    parts = [int(x) for x in (sys.argv[3] if len(sys.argv) > 3 else '0').split(',')]
    for r in rs:
        if not t - 2 <= r['frame'] <= t + 6:
            continue
        flags = [word(r['body.2a4'], 4 * i) for i in range(5)]
        print(r['frame'], 'mode', word(r['body.47c'], 0), 'riding/repose/placed/active/stiff', flags,
              'bike vel', fmt(quad(r['req'], 0x90)))
        for i in parts:
            b = r[f'part{i}']
            last, at, vel, rates = quad(b, 0), quad(b, 0x10), quad(b, 0x20), quad(b, 0x160)
            print(f'   part {i:2} pos {fmt(at)} moved {fmt([at[k] - last[k] for k in range(3)])} vel {fmt(vel)} '
                  f'rates {fmt(rates)}')


if __name__ == '__main__':
    main()
