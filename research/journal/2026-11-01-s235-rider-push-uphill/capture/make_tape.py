"""S235: a generated PCSX2 input recording from a save state, sticks centred, Cross held per segment.
P21's make_p2m2.py with a pressure and a pattern.

    python3 make_tape.py TEMPLATE.p2m2 OUT.p2m2 FIELDS SEG [SEG...]

Each SEG is `fields:pressure` (pressure 0 = Cross up). Segments play in order; the last one that is a
`/`-joined cycle (`6:255/6:0`) repeats to FIELDS. A generated tape plays two fields a game tick (S100),
so FIELDS = 2 x the ticks wanted."""
import sys
tpl = open(sys.argv[1], 'rb').read()
n = int(sys.argv[3])
head = bytearray(tpl[:561])
head += n.to_bytes(4, 'little') + (0).to_bytes(4, 'little') + bytes([1])
assert len(head) == 570


def field(pressure):
    pad = bytes([0xff, 0xbf if pressure else 0xff, 0x7f, 0x7f, 0x7f, 0x7f] + [0] * 6 + [pressure] + [0] * 5)
    assert len(pad) == 18
    return pad + bytes(18)


fields = []
for seg in sys.argv[4:]:
    parts = [tuple(int(v) for v in p.split(':')) for p in seg.split('/')]
    if len(parts) == 1:
        fields += [field(parts[0][1])] * parts[0][0]
    else:
        while len(fields) < n:
            for count, pressure in parts:
                fields += [field(pressure)] * count
fields = (fields + [field(0)] * n)[:n]
out = bytes(head) + bytes(36) + b''.join(fields)
open(sys.argv[2], 'wb').write(out)
print(sys.argv[2], len(out), 'bytes,', n, 'fields:', ' '.join(sys.argv[4:]))
