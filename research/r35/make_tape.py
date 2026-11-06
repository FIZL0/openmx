"""R35: recording2 with its first backflip stopped short.

Usage: make_tape.py recording2.p2m2 OUT.p2m2 --from FIELD [--to FIELD] [--end FIELD]

recording2 is two fields a game tick; the human's flip 1 fires at tick ~2506 (field ~248) and the
human centres the stick from field ~270, so the flip torque holds (C8). From --from to --to (default
390, the human's next press) this tape holds the left stick full forward with no trick button and
Circle lifted: with the trick group at 0 the forward stick bleeds the flip torque off at its
rate_decrease (C8's try 1), so the earlier --from is, the less of the flip the bike turns. Fields
335..349's Taunt (Circle) is lifted with the rest, as in C8. The tape ends at --end (default 660,
~tick 2707), and the header's field count is rewritten to match.
"""
import argparse

ap = argparse.ArgumentParser()
ap.add_argument('src')
ap.add_argument('out')
ap.add_argument('--from', dest='start', type=int, required=True)
ap.add_argument('--to', type=int, default=390)
ap.add_argument('--end', type=int, default=660)
a = ap.parse_args()

HEAD = 570          # header; the field count is the u32 at 561
FIELD = 36          # two pads of 18 bytes: buttons lo, hi (active low), RX RY LX LY, 12 pressures
TRICKS = 0x10 | 0x20 | 0x80                  # Triangle, Circle, Square in byte 1
P_TRICKS = (6 + 4, 6 + 5, 6 + 7)

raw = open(a.src, 'rb').read()
fields = [bytearray(raw[HEAD + i * FIELD:HEAD + (i + 1) * FIELD]) for i in range((len(raw) - HEAD) // FIELD)]

for i in range(a.start, a.to + 1):
    f = fields[i]
    f[1] |= TRICKS
    for p in P_TRICKS:
        f[p] = 0
    f[4], f[5] = 0x7f, 0x00     # left stick centred across, full forward (up)

fields = fields[:a.end + 1]
head = bytearray(raw[:HEAD])
head[561:565] = (len(fields) - 1).to_bytes(4, 'little')   # field 0 is the blank one before the first
open(a.out, 'wb').write(bytes(head) + b''.join(fields))
print(a.out, len(fields) - 1, 'fields')
