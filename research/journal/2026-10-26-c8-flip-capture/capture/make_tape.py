"""C8: a free09 tape with an uncut backflip and a no-hander, made from the human's recording2.

Usage: make_tape.py recording2.p2m2 OUT.p2m2 [--hold-to FIELD] [--end FIELD]

recording2 is two fields a game tick (frame_counter ~ field / 2 + 2377 for a trick press; S31 put
the flip's stick crossing at field 248 = tick 2505). Up to the first changed field the ride is the
human's, so the backflip at tick ~2506 fires as it did in V35's capture. Two changes:

1. **Flip 1's Taunt goes.** Fields 335..349 hold Circle with the stick up, which asks the Taunt at
   tick 2544 and cuts the backflip clip (V35). Circle is lifted and the stick centred. While a trick
   button is held the flip torque neither grows nor decays (`air::backflip`: only with the group at
   0 does the stick move it), and a turning bike gives the stick no say of its own, so the human's
   forward push under the Taunt did nothing. Pushed with no button held it bleeds the torque off and
   the bike lands short and wrecks (run 1). A centred stick freezes the torque as the Taunt did.
2. **The cancan becomes a no-hander.** Fields 549..600 hold Circle with the stick full left (cancan,
   slot 2, tick 2652, in the air 2652..2715). They become Triangle (trick button 2) with the stick
   full up for the first eight fields, which is slot 4 `nohandedlander`, and Triangle is held to
   field --hold-to (default 700, ~10 ticks past the human's landing) so the rider lands it no-handed.

The tape ends at field --end (default 860, ~tick 2807): after the changes the ride is not the human's,
and nothing past it is wanted. The header's frame count is rewritten to match.
"""
import argparse

ap = argparse.ArgumentParser()
ap.add_argument('src')
ap.add_argument('out')
ap.add_argument('--hold-to', type=int, default=700)
ap.add_argument('--end', type=int, default=860)
a = ap.parse_args()

HEAD = 570          # header; the field count is the u32 at 561
FIELD = 36          # two pads of 18 bytes: buttons lo, hi (active low), RX RY LX LY, 12 pressures
TRIANGLE, CIRCLE = 0x10, 0x20               # bits of byte 1
P_TRIANGLE, P_CIRCLE = 6 + 4, 6 + 5         # pressure order: R L U D Tri Cir X Sq L1 R1 L2 R2

raw = open(a.src, 'rb').read()
fields = [bytearray(raw[HEAD + i * FIELD:HEAD + (i + 1) * FIELD]) for i in range((len(raw) - HEAD) // FIELD)]


def press(f, bit, p):
    f[1] &= ~bit & 0xff
    f[p] = 0xff


def lift(f, bit, p):
    f[1] |= bit
    f[p] = 0


for i in range(335, 350):
    assert not fields[i][1] & CIRCLE, f'field {i}: the Taunt is not where it was'
    lift(fields[i], CIRCLE, P_CIRCLE)
    fields[i][4], fields[i][5] = 0x7f, 0x7f
assert fields[334][1] & CIRCLE and fields[350][1] & CIRCLE

assert fields[548][1] & CIRCLE and not fields[549][1] & CIRCLE and fields[601][1] & CIRCLE
for i in range(549, a.hold_to + 1):
    f = fields[i]
    lift(f, CIRCLE, P_CIRCLE)
    press(f, TRIANGLE, P_TRIANGLE)
    if i < 549 + 8:
        f[4], f[5] = 0x7f, 0x00     # left stick straight up: the gesture's direction
for i in range(a.hold_to + 1, a.end + 1):
    assert fields[i][1] & (CIRCLE | TRIANGLE) == CIRCLE | TRIANGLE, f'field {i}: a trick button is held'

fields = fields[:a.end + 1]
head = bytearray(raw[:HEAD])
head[561:565] = (len(fields) - 1).to_bytes(4, 'little')   # field 0 is the blank one before the first
open(a.out, 'wb').write(bytes(head) + b''.join(fields))
print(a.out, len(fields) - 1, 'fields')
