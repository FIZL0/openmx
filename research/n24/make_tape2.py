"""N24: write a two-pad .p2m2 from held-input segments, one list per pad.
C16's research/c16/make_tape.py with port 1 (P2) written from its own segments instead of idle.

usage: make_tape2.py TEMPLATE.p2m2 OUT.p2m2 P1_SEGMENT... / P2_SEGMENT...

Segments as in make_tape.py: FIELDS[:BUTTONS[:LX,LY[:RX,RY]]]. The shorter list is padded with idle
fields. OUT needs an OUT_SaveState.p2s beside it.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'c16'))
from make_tape import IDLE, fields  # noqa: E402


def main():
    tpl = open(sys.argv[1], 'rb').read()
    args = sys.argv[3:]
    cut = args.index('/') if '/' in args else len(args)
    p1, p2 = fields(args[:cut]), fields(args[cut + 1:])
    n = max(len(p1), len(p2))
    p1 += [IDLE] * (n - len(p1))
    p2 += [IDLE] * (n - len(p2))
    head = bytearray(tpl[:561]) + n.to_bytes(4, 'little') + (0).to_bytes(4, 'little') + bytes([1])
    assert len(head) == 570
    out = bytes(head) + bytes(36) + b''.join(a + b for a, b in zip(p1, p2))
    open(sys.argv[2], 'wb').write(out)
    print(sys.argv[2], n, 'fields')


if __name__ == '__main__':
    main()
