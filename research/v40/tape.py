"""V40: a shell tape for a by-eye look at the tracks: gas with the bars over for GAS ticks, then the
brake (Square) to a stop, then idle. One line a 30 Hz ride tick.

The shell ticks on each line's frame length (`--play` rides `played_dt`), so the lines say the 30 Hz
cap, 0.03333 (`0x3d08850a`). Until S222 they said 1/60 s (`0x3c888889`), as V70's did (S184): the
shell rode the schedule in half the time, and at V40's shot frame 190 the bike had not stopped but was
still rolling downhill at 39 mph.

HOLD (V120, default 0) adds that many ticks of the brake after the idle, so the bike stands still for
a shot from one pose; research/v40/hold.pad is the same schedule for the original's .p2m2.

usage: python3 -B research/v40/tape.py OUT [GAS] [STEER] [HOLD]   (STEER 0..255 on the left stick's x, 128 centred)
"""
import sys

out, gas, steer = sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 75, int(sys.argv[3]) if len(sys.argv) > 3 else 200
hold = int(sys.argv[4]) if len(sys.argv) > 4 else 0
total = gas + 120 + hold


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
        f.write(f'frame 0x3d08850a 0x{report(i < gas, gas <= i < gas + 60 or i >= gas + 120, steer if i < gas else 128)} -\n')
