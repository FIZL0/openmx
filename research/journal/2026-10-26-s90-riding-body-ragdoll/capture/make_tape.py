#!/usr/bin/env python3
"""S90: the shell tape of S66's 'ride' script in 30 Hz ticks (the same schedule the PCSX2 run plays)."""
import sys

N = 450
DT = '0x3d08850a'


def report(cross, l1, r1):
    b = bytearray.fromhex('0079ffff7f7f7f7f') + bytearray(24)
    if cross:
        b[3] &= ~0x40 & 0xff
        b[14] = 0xff
    if l1:
        b[3] &= ~0x04 & 0xff
        b[16] = 0xff
    if r1:
        b[3] &= ~0x08 & 0xff
        b[17] = 0xff
    return b.hex()


lines = [f'# game-sim pad tape: {N} frames', 'format 1', 'tape s90-ride']
for t in range(N):
    cross = 45 <= t < 430
    l1 = 195 <= t < 205 or 330 <= t < 340
    r1 = 199 <= t < 205 or 334 <= t < 340
    lines.append(f'frame {DT} 0x{report(cross, l1, r1)} -')
open(sys.argv[1], 'w').write('\n'.join(lines) + '\n')
