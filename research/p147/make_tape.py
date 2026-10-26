"""P147: recording2 with flip 1 under-rotated, so the bike comes down on its roof.

Usage: make_tape.py recording2.p2m2 OUT.p2m2 --push FROM:TO [--end FIELD]

recording2 is two fields a game tick (a press lands at about field 2 x (tick - 2377); C8). Flip 1
fires at tick 2506 and turns from 2512 (field ~270). Over fields FROM..TO the left stick is pushed
full forward (y 0) with every trick button lifted: against the flip, which bleeds its torque off at
the row's rate_decrease (`air::backflip`; C8's try 1 did the same from field 335 and still came
nearly round). From TO+1 to the end the stick is centred and no button is held, so nothing else
steers the bike while it is still turning. The tape ends at --end (default 500, about tick 2627).
"""
import argparse

ap = argparse.ArgumentParser()
ap.add_argument('src')
ap.add_argument('out')
ap.add_argument('--push', required=True)
ap.add_argument('--end', type=int, default=500)
a = ap.parse_args()
push_from, push_to = (int(x) for x in a.push.split(':'))

HEAD = 570          # header; the field count is the u32 at 561
FIELD = 36          # two pads of 18 bytes: buttons lo, hi (active low), RX RY LX LY, 12 pressures
TRICKS = 0x10 | 0x20 | 0x40 | 0x80          # Triangle, Circle, Cross, Square: bits of byte 1
GAS = 0x40                                  # Cross is the gas: kept as the human held it

raw = open(a.src, 'rb').read()
fields = [bytearray(raw[HEAD + i * FIELD:HEAD + (i + 1) * FIELD]) for i in range((len(raw) - HEAD) // FIELD)]

for i in range(push_from, a.end + 1):
    f = fields[i]
    # Lift Triangle, Circle and Square (and their pressures); the gas (Cross) stays the human's.
    for bit, p in ((0x10, 10), (0x20, 11), (0x80, 13)):
        f[1] |= bit
        f[p] = 0
    f[4] = 0x7f
    f[5] = 0x00 if i <= push_to else 0x7f

fields = fields[:a.end + 1]
head = bytearray(raw[:HEAD])
head[561:565] = (len(fields) - 1).to_bytes(4, 'little')   # field 0 is the blank one before the first
open(a.out, 'wb').write(bytes(head) + b''.join(fields))
print(a.out, len(fields) - 1, 'fields, pushed', push_from, '..', push_to)
