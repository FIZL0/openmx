"""V70: research/v70/launch.pad as a shell tape (one line a 30 Hz ride tick), for the port's side of
the side-by-side: 1 s idle, 0.9 s of full gas, 3 s of brake, 2.5 s of gas and brake with the bars full
over (backing up, P72), then the brake alone to the end.

usage: python3 -B research/v70/tape.py OUT [TOTAL_TICKS]
"""
import sys

out = sys.argv[1]
LX = int(sys.argv[3]) if len(sys.argv) > 3 else 255
total = int(sys.argv[2]) if len(sys.argv) > 2 else 450


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


def at(t):
    s = t / 30.0
    if s < 1.0:
        return report(False, False, 128)
    if s < 1.9:
        return report(True, False, 128)
    if s < 4.9:
        return report(False, True, 128)
    if s < 7.4:
        return report(True, True, LX)
    return report(False, True, 128)


with open(out, 'w') as f:
    f.write(f'# game-sim pad tape: {total} frames\nformat 1\ntape v70\n')
    for i in range(total):
        f.write(f'frame 0x3c888889 0x{at(i)} -\n')
