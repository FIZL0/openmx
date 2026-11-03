#!/usr/bin/env python3
"""MD81a: pair SLUS-21104's functions in an address range with SLUS-21701's by layout order.

The two builds lay MXVehicleMotocross/MXVehicleMotoShared out in the same order (twins.py's exact
matches sit at a near-constant shift), so the functions are aligned as a sequence: equal (code equal
with field offsets masked) runs pair one to one, and a replaced run of the same length pairs in order.

    research/md81a/run_align.sh A0 A1 U0 U1
"""
import difflib
import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import twins  # noqa: E402

a_elf, a_fn, u_elf, u_fn, a0, a1, u0, u1 = sys.argv[1:9]
a0, a1, u0, u1 = (int(x, 16) for x in (a0, a1, u0, u1))
A, U = twins.load(a_elf), twins.load(u_elf)
af = [f for f in twins.funcs(a_fn) if a0 <= f[0] < a1 and f[2] >= 4]
uf = [f for f in twins.funcs(u_fn) if u0 <= f[0] < u1 and f[2] >= 4]
ah = [tuple(twins.norm(twins.words(A, a, s), True)[0]) for a, n, s in af]
uh = [tuple(twins.norm(twins.words(U, a, s), True)[0]) for a, n, s in uf]
sm = difflib.SequenceMatcher(None, ah, uh, autojunk=False)
for tag, i1, i2, j1, j2 in sm.get_opcodes():
    if tag == 'equal':
        for i, j in zip(range(i1, i2), range(j1, j2)):
            same = twins.norm(twins.words(A, af[i][0], af[i][2]))[1] == twins.norm(twins.words(U, uf[j][0], uf[j][2]))[1]
            print(f'{af[i][0]:08x} {uf[j][0]:08x} same{"" if same else "-literals-differ"} {af[i][2]}')
    elif tag == 'replace' and i2 - i1 == j2 - j1:
        for i, j in zip(range(i1, i2), range(j1, j2)):
            r, moved, rest = twins.pair(A, U, af[i][0], af[i][2], uf[j][0], uf[j][2])
            print(f'{af[i][0]:08x} {uf[j][0]:08x} differs {af[i][2]}/{uf[j][2]} ratio {r:.3f}')
    else:
        for i in range(i1, i2):
            print(f'{af[i][0]:08x} -------- {tag} {af[i][2]}')
        for j in range(j1, j2):
            print(f'-------- {uf[j][0]:08x} {tag} {uf[j][2]}')
