#!/usr/bin/env python3
"""MD81a: where a function sits in the vtables of each build, and each vtable slot side by side.

    research/md81a/run_align.sh vtable ATV_FN UNT_FN [ATV_VT UNT_VT N]

Prints every word equal to ATV_FN in SLUS-21104 and to UNT_FN in SLUS-21701 (vtable slots), and with
ATV_VT UNT_VT N the first N slots of both vtables, each with its function's twin verdict.
"""
import os
import struct
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import twins  # noqa: E402

a_elf, a_fn, u_elf, u_fn = sys.argv[1:5]
A, U = twins.load(a_elf), twins.load(u_elf)
rest = [int(x, 16) for x in sys.argv[5:]]


def find(segs, w):
    out = []
    for va, b in segs:
        for i in range(0, len(b) - 3, 4):
            if struct.unpack_from('<I', b, i)[0] == w:
                out.append(va + i)
    return out


print('atv', ' '.join(f'{x:08x}' for x in find(A, rest[0])))
print('unt', ' '.join(f'{x:08x}' for x in find(U, rest[1])))
if len(rest) >= 5:
    av, uv, n = rest[2], rest[3], rest[4]
    af = {a: s for a, _, s in twins.funcs(a_fn)}
    uf = {a: s for a, _, s in twins.funcs(u_fn)}
    for k in range(n):
        x = twins.words(A, av + 4 * k, 4)[0]
        y = twins.words(U, uv + 4 * k, 4)[0]
        v = ''
        if x in af and y in uf and af[x] >= 4 and uf[y] >= 4:
            r, moved, diff = twins.pair(A, U, x, af[x], y, uf[y])
            v = 'same' if not diff else f'ratio {r:.3f}'
        print(f'+{4 * k:#05x} {x:08x} {y:08x} {v}')
