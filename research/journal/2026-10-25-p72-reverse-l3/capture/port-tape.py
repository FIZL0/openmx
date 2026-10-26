#!/usr/bin/env python3
"""P72: the port backing up, for a by-eye look beside the original's shot.

A tape holds the pad already past the layout, so gas and brake are the game's Cross and Square:
rest, a blip of the gas to open the gate, roll to a stop, then both held.

    python3 -B port-tape.py back.tape
    openmx --track free09 --trace-at 4620,2320,0 --play back.tape --headless \
        --shot context/shots/p72/port-back.png --shot-frame 230 --frames 240
"""
import sys

def rep(b3=0xff, cross=0, square=0):
    r = bytearray.fromhex("0079ffff7f7f7f7f") + bytearray(24)
    r[3], r[14], r[15] = b3, cross, square
    return r.hex()

lines = [rep()] * 30 + [rep(0xbf, 0xff)] * 5 + [rep()] * 85 + [rep(0x3f, 0xff, 0xff)] * 150
with open(sys.argv[1], "w") as f:
    f.write(f"# game-sim pad tape: {len(lines)} frames\nformat 1\ntape p72-back\n")
    f.writelines(f"frame 0x3c888889 0x{l} -\n" for l in lines)
