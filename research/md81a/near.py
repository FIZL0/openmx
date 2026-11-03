#!/usr/bin/env python3
"""MD81a: for each SLUS-21104 address, the SLUS-21701 function near the same place in the layout
(atv address + 0x17000 .. + 0x19000) whose code, field offsets masked, is closest.

    research/md81a/run_align.sh near ADDR...
"""
import difflib
import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import twins  # noqa: E402

a_elf, a_fn, u_elf, u_fn = sys.argv[1:5]
A, U = twins.load(a_elf), twins.load(u_elf)
af = {a: s for a, n, s in twins.funcs(a_fn)}
uf = [f for f in twins.funcs(u_fn) if f[2] >= 4]
lo, hi = 0x17000, 0x19000
for x in sys.argv[5:]:
    a = int(x, 16)
    an = twins.norm(twins.words(A, a, af[a]), True)[0]
    best = []
    for u, n, s in uf:
        if a + lo <= u < a + hi:
            un = twins.norm(twins.words(U, u, s), True)[0]
            best.append((difflib.SequenceMatcher(None, an, un, autojunk=False).ratio(), u, s))
    best.sort(reverse=True)
    print(f'{a:08x} {af[a]:5d}  ' + '  '.join(f'{u:08x} {s} {r:.3f}' for r, u, s in best[:3]))
