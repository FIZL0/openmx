"""S217: a free09 tape with a queued trick pair, flatliner then the Taunt, made from C8's tape.

Usage: make_tape.py c8.p2m2 OUT.p2m2 [--second FIELD] [--centre-to FIELD]

C8's tape (research/journal/2026-10-26-c8-flip-capture/capture/make_tape.py) is the human's
recording2 with flip 1's Taunt lifted and the cancan made a no-hander: Triangle with the stick up at
field 549 (tick ~2651) on the jump the bike leaves at ~2650 and lands at ~2718. This tape changes
only that jump, so everything up to field 549 is still the human's and C8's.

1. **Flatliner** (slot 7: group 2, right): Triangle with the stick full right over fields 549..556.
2. **The Taunt** (slot 0: group 1, up), queued while flatliner runs: Circle with the stick full up
   over fields --second .. --second + 7 (default 579, ~15 ticks into flatliner).

Both buttons are let go well before flatliner's HoldTime (0.93 s, ~28 ticks), so neither trick is
held: its clock is plain dt, with no hold wobble (the noise wave) and no easing into a hold. Between
and after the presses the stick is centred to --centre-to (default 680, about the landing): with no
trick button down a pushed stick has its full say, and the human's full-left lean that C8's held
no-hander halved would roll the bike; centred, the bike flies square (P29). From there on the tape is
C8's (the human's) again, and it ends where C8's does.
"""
import argparse

ap = argparse.ArgumentParser()
ap.add_argument('src')
ap.add_argument('out')
ap.add_argument('--second', type=int, default=579)
ap.add_argument('--centre-to', type=int, default=680)
a = ap.parse_args()

HEAD = 570          # header; the field count is the u32 at 561
FIELD = 36          # two pads of 18 bytes: buttons lo, hi (active low), RX RY LX LY, 12 pressures
TRIANGLE, CIRCLE = 0x10, 0x20               # bits of byte 1
P_TRIANGLE, P_CIRCLE = 6 + 4, 6 + 5         # pressure order: R L U D Tri Cir X Sq L1 R1 L2 R2
FIRST, PRESS = 549, 8

raw = open(a.src, 'rb').read()
n = int.from_bytes(raw[561:565], 'little')
fields = [bytearray(raw[HEAD + i * FIELD:HEAD + (i + 1) * FIELD]) for i in range(n + 1)]
assert len(raw) == HEAD + (n + 1) * FIELD, 'not C8 tape: trailing bytes'


def press(f, bit, p):
    f[1] &= ~bit & 0xff
    f[p] = 0xff


def lift(f, bit, p):
    f[1] |= bit
    f[p] = 0


# C8's no-hander: Triangle from 549 to 600, the stick up for the first eight fields.
assert all(not fields[i][1] & TRIANGLE for i in range(549, 601)), 'not C8 tape: no no-hander at 549..600'
assert fields[548][1] & TRIANGLE and fields[601][1] & TRIANGLE
assert FIRST + PRESS <= a.second and a.second + PRESS <= a.centre_to
for i in range(549, 601):
    lift(fields[i], TRIANGLE, P_TRIANGLE)
for i in range(FIRST + PRESS, a.centre_to + 1):
    fields[i][4], fields[i][5] = 0x7f, 0x7f
for i in range(FIRST, FIRST + PRESS):
    press(fields[i], TRIANGLE, P_TRIANGLE)
    fields[i][4], fields[i][5] = 0xff, 0x7f     # full right
for i in range(a.second, a.second + PRESS):
    press(fields[i], CIRCLE, P_CIRCLE)
    fields[i][4], fields[i][5] = 0x7f, 0x00     # full up
for i in range(FIRST - 4, a.centre_to + 1):
    held = fields[i][1] & (CIRCLE | TRIANGLE) != CIRCLE | TRIANGLE
    assert held == (FIRST <= i < FIRST + PRESS or a.second <= i < a.second + PRESS), f'field {i}'

open(a.out, 'wb').write(raw[:HEAD] + b''.join(fields))
print(a.out, n, 'fields')
