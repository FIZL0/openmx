#!/usr/bin/env python3
"""S143: summarise a pilot_watch.py log, one line a tick (or every --every ticks).

    python3 -B pilot_read.py context/s143/nose.txt [--every 5] [--from F] [--to F] [--regions a,b]

Per tick: frame, the plane's wreck word and clock, grounded, airspeed, the plane node's position, the
pilot's root node's world position less the plane's, in the plane's axes, and the body's mode words
(+0x47c..). With --regions, the raw hex of those regions too. Research, not product.
"""

import argparse
import struct


def f32s(h):
    return struct.unpack(f'<{len(h) // 8}f', bytes.fromhex(h))


def u32s(h):
    return struct.unpack(f'<{len(h) // 8}I', bytes.fromhex(h))


def ticks(path):
    tick = None
    for line in open(path):
        if line.startswith('#'):
            continue
        name, _, hx = line.rstrip('\n').partition(' ')
        if name == 'f':
            if tick:
                yield tick
            tick = {'f': int(hx)}
        elif tick is not None:
            tick[name] = hx
    if tick:
        yield tick


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('log')
    ap.add_argument('--every', type=int, default=1)
    ap.add_argument('--from', dest='lo', type=int, default=0)
    ap.add_argument('--to', dest='hi', type=int, default=1 << 31)
    ap.add_argument('--regions', default='')
    ap.add_argument('--nodes', action='store_true', help='every joint node world position, plane-local')
    ap.add_argument('--req', action='store_true', help="the pilot request's phase and mix targets, the pad")
    a = ap.parse_args()
    regs = [r for r in a.regions.split(',') if r]
    for i, t in enumerate(ticks(a.log)):
        if not (a.lo <= t['f'] <= a.hi) or i % a.every:
            continue
        w = u32s(t['plane.3b0'])
        clock = f32s(t['plane.3b0'])[3]
        g = u32s(t['plane.590'])[0]
        air = f32s(t['plane.590'])[2]
        pn = f32s(t['plane.node'])
        ax = [pn[16:19], pn[20:23], pn[24:27]]
        o = pn[28:31]

        def local(q):
            d = [q[k] - o[k] for k in range(3)]
            return tuple(sum(d[k] * ax[r][k] for k in range(3)) for r in range(3))
        n0 = f32s(t['node0'])
        root = local(n0[44:47])
        mode = u32s(t['body.47c'])
        line = (f"{t['f']} w {w[0]} {w[1]} c {clock:.3f} g {g} air {air:7.2f} pos {o[0]:8.2f} {o[1]:7.2f} {o[2]:8.2f}"
                f" root {root[0]:6.2f} {root[1]:6.2f} {root[2]:6.2f} mode {' '.join(f'{m:x}' for m in mode)}")
        if a.req:
            r = bytes.fromhex(t['req'])
            ph, mx = struct.unpack_from('<2f', r, 0x170)
            line += f" req phase {ph:.3f} mix {mx:.3f} held {r[0xc]} air {r[0xf8]} pad {t['pad'][:48]}"
        if a.nodes:
            line += ' |' + ' '.join('%.2f,%.2f,%.2f' % local(f32s(t[f'node{j}'])[44:47])
                                     for j in range(22) if f'node{j}' in t)
        print(line)
        for r in regs:
            print(f'  {r} {t[r]}')


if __name__ == '__main__':
    main()
