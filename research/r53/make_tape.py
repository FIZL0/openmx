"""R53: research/c16/make_tape.py with P2's pad driven too.

usage: make_tape.py TEMPLATE.p2m2 OUT.p2m2 P1_SEGMENT... [-- P2_SEGMENT...]

The segments are c16's (FIELDS[:BUTTONS[:LX,LY[:RX,RY]]]). The shorter pad's list is padded idle to the
longer's. OUT needs an OUT_SaveState.p2s beside it. R53's boot ticks at 30 Hz: two fields a tick.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'c16'))
from make_tape import IDLE, fields     # noqa: E402

args = sys.argv[3:]
cut = args.index('--') if '--' in args else len(args)
p1, p2 = fields(args[:cut]), fields(args[cut + 1:])
n = max(len(p1), len(p2))
p1 += [IDLE] * (n - len(p1))
p2 += [IDLE] * (n - len(p2))
tpl = open(sys.argv[1], 'rb').read()
head = bytearray(tpl[:561]) + n.to_bytes(4, 'little') + (0).to_bytes(4, 'little') + bytes([1])
assert len(head) == 570
open(sys.argv[2], 'wb').write(bytes(head) + bytes(36) + b''.join(a + b for a, b in zip(p1, p2)))
print(sys.argv[2], n, 'fields')
