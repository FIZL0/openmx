"""C20: a free09 tape with a full whip, made from the human's recording2 (C8's make_tape.py, changed).

Usage: make_tape.py recording2.p2m2 OUT.p2m2 [--from FIELD] [--to FIELD] [--x X] [--end FIELD]

recording2 is two fields a game tick (a press lands at about field 2 x (tick - 2377)). Two changes:

1. **Flip 1's Taunt goes**, as in C8 (fields 335..349: Circle lifted, the stick centred), so the
   backflip off jump 1 lands clean and the ride reaches jump 2 as C8's run did.
2. **Jump 2's cancan becomes a full whip.** The bike leaves the ground at tick ~2651 (field ~548) and
   lands at ~2718 (field ~682). Over fields 549..600 the human held Circle with the stick full left
   (the cancan); from 602 the human held the stick hard right anyway, to past the landing. Circle is
   lifted, and the left stick's x is X (default 255, hard right) from --from (549) to --to (700), so
   the stick is hard over from just after the take-off to past the landing, with no trick running.
   The stick's y is the human's.

The tape ends at field --end (default 860, ~tick 2807).
"""
import argparse

ap = argparse.ArgumentParser()
ap.add_argument('src')
ap.add_argument('out')
ap.add_argument('--from', dest='start', type=int, default=549)
ap.add_argument('--to', type=int, default=700)
ap.add_argument('--x', type=int, default=255)
ap.add_argument('--end', type=int, default=860)
a = ap.parse_args()

HEAD = 570          # header; the field count is the u32 at 561
FIELD = 36          # two pads of 18 bytes: buttons lo, hi (active low), RX RY LX LY, 12 pressures
CIRCLE = 0x20       # bit of byte 1
P_CIRCLE = 6 + 5    # pressure order: R L U D Tri Cir X Sq L1 R1 L2 R2

raw = open(a.src, 'rb').read()
fields = [bytearray(raw[HEAD + i * FIELD:HEAD + (i + 1) * FIELD]) for i in range((len(raw) - HEAD) // FIELD)]


def lift(f, bit, p):
    f[1] |= bit
    f[p] = 0


for i in range(335, 350):
    assert not fields[i][1] & CIRCLE, f'field {i}: the Taunt is not where it was'
    lift(fields[i], CIRCLE, P_CIRCLE)
    fields[i][4], fields[i][5] = 0x7f, 0x7f
assert fields[334][1] & CIRCLE and fields[350][1] & CIRCLE

assert fields[548][1] & CIRCLE and not fields[549][1] & CIRCLE and fields[601][1] & CIRCLE
for i in range(549, 601):
    lift(fields[i], CIRCLE, P_CIRCLE)
for i in range(a.start, a.to + 1):
    fields[i][4] = a.x
for i in range(a.start, a.end + 1):
    assert fields[i][1] & 0xb0 == 0xb0, f'field {i}: a trick button is held'

fields = fields[:a.end + 1]
head = bytearray(raw[:HEAD])
head[561:565] = (len(fields) - 1).to_bytes(4, 'little')   # field 0 is the blank one before the first
open(a.out, 'wb').write(bytes(head) + b''.join(fields))
print(a.out, len(fields) - 1, 'fields')
