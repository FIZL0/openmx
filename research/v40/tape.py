"""V40: a shell tape for a by-eye look at the tracks: gas with the bars over for GAS ticks, then the
brake (Square) to a stop, then idle. One line a 30 Hz ride tick.

usage: python3 -B research/v40/tape.py OUT [GAS] [STEER]   (STEER 0..255 on the left stick's x, 128 centred)
"""
import sys

out, gas, steer = sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 75, int(sys.argv[3]) if len(sys.argv) > 3 else 200
total = gas + 120


def report(cross, square, lx):
    b = bytearray.fromhex('0079ffff7f7f7f7f') + bytearray(24)
    face = 0xff
    if cross:
        face &= 0xbf
        b[14] = 0xff
    if square:
        face &= 0x7f
        b[15] = 0xff
    b[3] = face
    b[6] = lx
    return b.hex()


with open(out, 'w') as f:
    f.write(f'# game-sim pad tape: {total} frames\nformat 1\ntape v40\n')
    for i in range(total):
        f.write(f'frame 0x3c888889 0x{report(i < gas, gas <= i < gas + 60, steer if i < gas else 128)} -\n')
