"""S229a: a generated drive that puts a car's tires on a mesh, as a PCSX2 input recording.

usage: make_tape.py TEMPLATE.p2m2 OUT.p2m2 SCHEDULE

TEMPLATE is S149a's tape for the same car (context/s149a/<car>.p2m2): its header and save state. SCHEDULE
is one of SCHEDULES below. The fields are S149a's make_tape.py's (two a game tick; Cross gas, Square
brake, both together reverse, Circle hand brake, L1 clutch, the left stick steers).

The target is free07's garage, the one prop near S149a's start line: a slab 0.7 ft above the ground,
31 x 43 ft, turned 32 degrees, walled on three sides and open to the west-south-west, its opening about
46 ft behind the parked baja. Backing in puts the rear tires, then all four, on the slab.
"""
import sys

HEAD = 570
CROSS, SQUARE, CIRCLE, L1 = 0x40, 0x80, 0x20, 0x04
P_CIRCLE, P_CROSS, P_SQUARE, P_L1 = 6 + 5, 6 + 6, 6 + 7, 6 + 8
PRESSURE = {CROSS: P_CROSS, SQUARE: P_SQUARE, CIRCLE: P_CIRCLE, L1: P_L1}

# (first tick, last tick + 1, buttons, left stick x, left stick y): 0x7f centred.
SCHEDULES = {
    'probe': [
        (0, 15, 0, 0x7f, 0x7f),
        (15, 215, CROSS | SQUARE, 0xff, 0x7f),
        (215, 300, 0, 0x7f, 0x7f),
    ],
    'garage': [
        (0, 15, 0, 0x7f, 0x7f),                         # stand
        (15, 55, CROSS | SQUARE, 0x00, 0x7f),           # reverse, steer full left: the rear swings east
        (55, 93, CROSS | SQUARE, 0x7f, 0x7f),           # reverse straight, into the garage's opening
        (93, 140, SQUARE, 0x7f, 0x7f),                  # brake: all four tires on the slab
        (140, 160, 0, 0x7f, 0x7f),                      # stand
        (160, 200, CROSS, 0x7f, 0x7f),                  # gas: out of the garage and off the slab
        (200, 240, SQUARE, 0x7f, 0x7f),                 # brake
        (240, 260, 0, 0x7f, 0x7f),                      # stand
    ],
}


def field(buttons, x, y):
    f = bytearray([0xff, 0xff, 0x7f, 0x7f, x, y] + [0] * 12)
    for bit, p in PRESSURE.items():
        if buttons & bit:
            f[1] &= ~bit & 0xff
            f[p] = 0xff
    return bytes(f)


if __name__ == '__main__':
    tpl = open(sys.argv[1], 'rb').read()
    schedule = SCHEDULES[sys.argv[3]]
    fields = []
    for a, b, k, x, y in schedule:
        fields += [field(k, x, y)] * (2 * (b - a))
    head = bytearray(tpl[:561]) + len(fields).to_bytes(4, 'little') + (0).to_bytes(4, 'little') + bytes([1])
    assert len(head) == HEAD
    out = bytes(head) + bytes(36) + b''.join(f + bytes(18) for f in fields)
    open(sys.argv[2], 'wb').write(out)
    print(sys.argv[2], len(fields), 'fields,', schedule[-1][1], 'ticks')
