#!/usr/bin/env python3
"""S143: a shell --play tape that flies the biplane off the runway and banks it into the ground.

    python3 -B crash_tape.py OUT [--gas 300] [--bank 200] [--rest 220]

One line a 30 Hz tick: idle, then Cross (gas) for --gas ticks, then gas with the left stick hard left
and d-pad Up (nose down) for --bank ticks, then idle. Research, not product.
"""

import argparse

DT = '0x3d088889'  # 1/30


def report(cross=False, left=False, dive=False):
    b = bytearray.fromhex('0079ffff7f7f7f7f') + bytearray(24)
    if cross:
        b[3] &= 0xbf
        b[14] = 0xff
    if dive:
        b[2] &= 0xef
        b[10] = 0xff
    if left:
        b[6] = 0x00
    return '0x' + b.hex()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--idle', type=int, default=30)
    ap.add_argument('--gas', type=int, default=300)
    ap.add_argument('--bank', type=int, default=200)
    ap.add_argument('--rest', type=int, default=220)
    a = ap.parse_args()
    rows = ([report()] * a.idle + [report(cross=True)] * a.gas
            + [report(cross=True, left=True, dive=True)] * a.bank + [report()] * a.rest)
    with open(a.out, 'w') as f:
        f.write(f'# game-sim pad tape: {len(rows)} frames\nformat 1\ntape s143-crash\n')
        for r in rows:
            f.write(f'frame {DT} {r}\n')


if __name__ == '__main__':
    main()
