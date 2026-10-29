"""S149a: a generated drive for a car machine, as a PCSX2 input recording.

usage: make_tape.py TEMPLATE.p2m2 OUT.p2m2 [STEER]

STEER (1..0x7f, default 0x7f) is how far the stick goes from centre on the steering legs: full lock
rolled the monster truck over at speed, so its tape steers 0x40.

The header (version, emulator, author, game, from_savestate) comes from TEMPLATE, as P21's
make_p2m2.py does it; the fields are this file's schedule, two fields a game tick. The car's card
(S149a's switch.py) gives the controls: Cross accelerates, Square brakes, Circle is the hand brake,
L1 the clutch, the left stick steers.

The schedule, in ticks from the state's own:
"""
import sys

HEAD = 570          # the field count is the u32 at 561
CROSS, SQUARE, CIRCLE, L1 = 0x40, 0x80, 0x20, 0x04          # bits of byte 1, active low
P_CIRCLE, P_CROSS, P_SQUARE, P_L1 = 6 + 5, 6 + 6, 6 + 7, 6 + 8  # pressures: R L U D Tri Cir X Sq L1 R1 L2 R2
PRESSURE = {CROSS: P_CROSS, SQUARE: P_SQUARE, CIRCLE: P_CIRCLE, L1: P_L1}

# (first tick, last tick + 1, buttons, left stick x, left stick y): 0x7f centred, 0 full left/up.
SCHEDULE = [
    (0, 15, 0, 0x7f, 0x7f),                     # stand
    (15, 135, CROSS, 0x7f, 0x7f),               # gas from rest: the gearbox up through its gears
    (135, 185, CROSS, 0x00, 0x7f),              # gas, steer full left
    (185, 235, CROSS, 0xff, 0x7f),              # gas, steer full right
    (235, 275, CROSS, 0x7f, 0x7f),              # gas, straight
    (275, 320, SQUARE, 0x7f, 0x7f),             # brake, to a stop and on into reverse
    (320, 360, CROSS, 0x7f, 0x7f),              # gas again
    (360, 405, CROSS | CIRCLE, 0x00, 0x7f),     # hand brake into a left turn, on the gas
    (405, 435, CROSS, 0x7f, 0x7f),              # straighten out
    (435, 475, CROSS | L1, 0x7f, 0x7f),         # clutch in, revving
    (475, 520, CROSS, 0x7f, 0x7f),              # clutch dropped
    (520, 545, CROSS, 0x7f, 0xff),              # stick back, then forward: the preload's hop if it has one
    (545, 560, CROSS, 0x7f, 0x00),
    (560, 660, 0, 0x7f, 0x7f),                  # coast
]
__doc__ += ''.join(f'    {a:4}..{b:4}  buttons {k:#04x}  stick ({x:#04x}, {y:#04x})\n' for a, b, k, x, y in SCHEDULE)
TICKS = SCHEDULE[-1][1]


def field(buttons, x, y):
    f = bytearray([0xff, 0xff, 0x7f, 0x7f, x, y] + [0] * 12)
    for bit, p in PRESSURE.items():
        if buttons & bit:
            f[1] &= ~bit & 0xff
            f[p] = 0xff
    return bytes(f)


if __name__ == '__main__':
    tpl = open(sys.argv[1], 'rb').read()
    steer = int(sys.argv[3], 0) if len(sys.argv) > 3 else 0x7f
    fields = []
    for a, b, k, x, y in SCHEDULE:
        x = {0x00: 0x7f - steer, 0xff: 0x80 + steer}.get(x, x)
        fields += [field(k, x, y)] * (2 * (b - a))
    head = bytearray(tpl[:561]) + len(fields).to_bytes(4, 'little') + (0).to_bytes(4, 'little') + bytes([1])
    assert len(head) == HEAD
    # Field 0 is the blank one before the first (make_p2m2.py), then pad 0 and an empty pad 1 a field.
    out = bytes(head) + bytes(36) + b''.join(f + bytes(18) for f in fields)
    open(sys.argv[2], 'wb').write(out)
    print(sys.argv[2], len(fields), 'fields,', TICKS, 'ticks')
