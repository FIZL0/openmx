#!/usr/bin/env python3
"""S74: read a bones_watch.py capture and print the ticks around each put-back.

    python3 -B research/journal/2026-10-21-s74-putback-bones/capture/bones_read.py context/s74/free06-ride.txt

Research, not product.
"""

import os
import struct
import sys


ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))


def ticks(path):
    head = {}
    cur = None
    for line in open(path):
        line = line.rstrip('\n')
        if line.startswith('#'):
            if line.startswith('# bike'):
                w = line[2:].split()
                head['raw'] = w
            continue
        k, v = line.split(' ', 1)
        if k == 'f':
            cur = {'f': int(v)}
            yield cur
        else:
            cur[k] = bytes.fromhex(v)


def f32(b, o, n=1):
    v = struct.unpack_from(f'<{n}f', b, o)
    return v[0] if n == 1 else v


def u32(b, o):
    return struct.unpack_from('<I', b, o)[0]


def summary(t):
    reason = u32(t['bike.748'], 0)
    ghost = f32(t['bike.7dc'], 0)
    mode, prevm = u32(t['body.47c'], 0), u32(t['body.47c'], 4)
    state = u32(t['body.8dc'], 0x14)
    ramp = f32(t['body.680'], 0xc)
    bike = f32(t['bike.node'], 0x70, 3)
    n0 = f32(t['node0'], 0xb0, 3)
    l0 = f32(t['node0'], 0x70, 3)
    jt0 = f32(t['joints'], 0x40, 3)
    hand = f32(t['node11'], 0xb0, 3)
    return (f'f {t["f"]} reason {reason} ghost {ghost:.3f} mode {mode}/{prevm} state {state} ramp {ramp:.3f} '
            f'bike {fmt(bike)} n0.w {fmt(n0)} n0.l {fmt(l0)} j0.t {fmt(jt0)} flag0 {u32(t["node0"], 0):08x} '
            f'hand.w {fmt(hand)}')


def fmt(v):
    return '(' + ', '.join(f'{x:.3f}' for x in v) + ')'


def main():
    name = sys.argv[1]
    ts = list(ticks(name if "/" in name else os.path.join(ROOT, "context", "s74", name + ".txt")))
    lo = int(sys.argv[2]) if len(sys.argv) > 2 else None
    hi = int(sys.argv[3]) if len(sys.argv) > 3 else None
    prev = None
    for t in ts:
        if lo is not None:
            if lo <= t['f'] <= hi:
                print(summary(t))
            continue
        r = u32(t['bike.748'], 0)
        g = f32(t['bike.7dc'], 0)
        if prev and (u32(prev['bike.748'], 0) != r or (g > 0) != (f32(prev['bike.7dc'], 0) > 0)):
            print(summary(prev))
            print(summary(t))
            print()
        prev = t


if __name__ == '__main__':
    main()
