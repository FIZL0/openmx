"""P264: S149a's car drive with the look-back held over stretches of it, as a PCSX2 input recording.

usage: make_tape.py TEMPLATE.p2m2 OUT.p2m2 [STEER]

S149a's make_tape.py schedule (gas, steer both ways, brake, hand brake, clutch, stick hop, coast),
with L2 (the original layout's look-back) held down besides on the ticks of LOOK_BACK: on the
straight from rest, through the full-left turn, under the brake into reverse and on the coast.
"""
import importlib.util
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('s149a', os.path.join(here, '../2026-10-29-s149a-car-captures/make_tape.py'))
s149a = importlib.util.module_from_spec(spec)
spec.loader.exec_module(s149a)

L2, P_L2 = 0x01, 6 + 10       # byte 1's bit, active low; its pressure

# (first tick, last tick + 1), in ticks from the state's own.
LOOK_BACK = [(60, 110), (150, 175), (290, 330), (580, 630)]


def field(buttons, x, y, back):
    f = bytearray(s149a.field(buttons, x, y))
    if back:
        f[1] &= ~L2 & 0xff
        f[P_L2] = 0xff
    return bytes(f)


if __name__ == '__main__':
    tpl = open(sys.argv[1], 'rb').read()
    steer = int(sys.argv[3], 0) if len(sys.argv) > 3 else 0x7f
    fields = []
    for a, b, k, x, y in s149a.SCHEDULE:
        x = {0x00: 0x7f - steer, 0xff: 0x80 + steer}.get(x, x)
        for t in range(a, b):
            back = any(lo <= t < hi for lo, hi in LOOK_BACK)
            fields += [field(k, x, y, back)] * 2
    head = bytearray(tpl[:561]) + len(fields).to_bytes(4, 'little') + (0).to_bytes(4, 'little') + bytes([1])
    assert len(head) == s149a.HEAD
    out = bytes(head) + bytes(36) + b''.join(f + bytes(18) for f in fields)
    open(sys.argv[2], 'wb').write(out)
    print(sys.argv[2], len(fields), 'fields,', s149a.TICKS, 'ticks, look-back', LOOK_BACK)
