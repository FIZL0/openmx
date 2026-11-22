"""R53: print P2's wreck words and wreck body, and the truck's body, from an EE RAM image of R53's boot.

usage: python3 -B research/r53/ram_peek.py STATE    (a context/states/r53 image name, without .ram)
"""
import struct
import sys
from pathlib import Path

r = (Path(__file__).resolve().parents[2] / 'context' / 'states' / 'r53' / (sys.argv[1] + '.ram')).read_bytes()
u = lambda a: struct.unpack_from('<I', r, a)[0]
f = lambda a, n=3: tuple(round(x, 3) for x in struct.unpack_from('<%df' % n, r, a))
RIDER, TRUCK = 0x911c90, 0x1339fd0
b = u(u(RIDER + 0x7a8) + 0x110)
print('frame', u(0x479d04), 'reason', u(RIDER + 0x748), 'fore limit', f(RIDER + 0x758, 1))
print('wreck body', hex(b), 'pos', f(b + 0x60), 'vel', f(b + 0x70))
print('chassis vel', f(u(RIDER + 0x7a4) + 0x260))
t = u(TRUCK + 0x160)
print('truck awake', u(t), 'pos', f(t + 0x60), 'vel', f(t + 0x70))
