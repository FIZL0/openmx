"""C12: the 2p tape's inputs, one line per STEP fields: held buttons and stick bytes per pad.

    python3 -B research/c12/tape_head.py [FROM] [TO] [STEP]

Buttons are libpad's active-low bits, named. Research, not product.
"""
import os
import sys

HEAD = 570
NAMES = ['sel', 'L3', 'R3', 'st', 'up', 'rt', 'dn', 'lt', 'L2', 'R2', 'L1', 'R1', 'tri', 'O', 'X', 'sq']
raw = open(os.path.join('context', 'recordings', 'split', '2p.p2m2'), 'rb').read()
n = (len(raw) - HEAD) // 36
a, b, step = (int(x) for x in (sys.argv[1:] + ['0', '600', '15'][len(sys.argv) - 1:])[:3])
for i in range(a, min(b, n), step):
    out = ['%5d' % i]
    for port in (0, 1):
        p = raw[HEAD + 36 * i + 18 * port:HEAD + 36 * i + 18 * port + 18]
        bits = (~(p[0] | p[1] << 8)) & 0xffff
        held = '+'.join(NAMES[k] for k in range(16) if bits >> k & 1) or '-'
        out.append('P%d %-14s R(%3d,%3d) L(%3d,%3d)' % (port + 1, held, p[2], p[3], p[4], p[5]))
    print('  '.join(out))
