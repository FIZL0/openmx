#!/usr/bin/env python3
"""S201: a PCSX2 input recording of one of S74's pad scripts, for a machine with no virtual pad.

    make_tape.py TEMPLATE.p2m2 STATE.p2s ride OUT.p2m2 [--fields-per-tick 2]

Writes OUT.p2m2 (P21's make_p2m2.py layout: TEMPLATE's first 561 header bytes, the field count, from
savestate, then pad 1 and pad 2's 18 bytes a field) and copies STATE.p2s beside it as
OUT.p2m2_SaveState.p2s. The script is S74's `bones_watch.py` one, its wall-clock sleeps turned into
ticks at 30 a second; each tick is `--fields-per-tick` fields (S100 saw a generated tape play two fields
a tick; check where the pad words change in the capture). Research, not product.
"""

import argparse
import shutil

# S74's scripts as (tick, what) steps: a button down or up, or the left stick at (x, y) in -1..1.
SCRIPTS = {
    'ride': [(45, 'down cross'), (195, 'down l1'), (199, 'down r1'), (205, 'up l1'), (205, 'up r1'),
             (325, 'down l1'), (329, 'down r1'), (335, 'up l1'), (335, 'up r1'), (425, 'up cross')],
    'down': [(45, 'down cross'), (45, 'stick 0 1'), (150, 'stick 1 1'), (225, 'stick 0 0'), (225, 'up cross'),
             (261, 'down l1'), (265, 'down r1'), (271, 'up l1'), (271, 'up r1'), (391, 'stick 0 0')],
}
LENGTH = {'ride': 520, 'down': 520}

# The pad's two digital bytes are active low; byte 1: L2 0, R2 1, L1 2, R1 3, triangle 4, circle 5,
# cross 6, square 7. The pressure bytes after the four stick bytes: right, left, up, down, triangle,
# circle, cross, square, L1, R1, L2, R2.
BIT = {'l1': 2, 'r1': 3, 'cross': 6}
PRESSURE = {'cross': 6, 'l1': 8, 'r1': 9}


def field(held, stick):
    b1 = 0xff
    for b in held:
        b1 &= ~(1 << BIT[b]) & 0xff
    axis = lambda v: max(0, min(255, round(0x7f + v * 0x80)))
    pressures = [0] * 12
    for b in held:
        pressures[PRESSURE[b]] = 0xff
    return bytes([0xff, b1, 0x7f, 0x7f, axis(stick[0]), axis(stick[1])] + pressures)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('template')
    ap.add_argument('state')
    ap.add_argument('script', choices=sorted(SCRIPTS))
    ap.add_argument('out')
    ap.add_argument('--fields-per-tick', type=int, default=2)
    a = ap.parse_args()
    held, stick = set(), (0.0, 0.0)
    steps = sorted(SCRIPTS[a.script], key=lambda s: s[0])
    body = bytearray()
    n = 0
    for tick in range(LENGTH[a.script]):
        for t, what in steps:
            if t != tick:
                continue
            w = what.split()
            if w[0] == 'down':
                held.add(w[1])
            elif w[0] == 'up':
                held.discard(w[1])
            else:
                stick = (float(w[1]), float(w[2]))
        f = field(held, stick)
        assert len(f) == 18
        for _ in range(a.fields_per_tick):
            body += f + bytes(18)
            n += 1
    tpl = open(a.template, 'rb').read()
    head = bytearray(tpl[:561]) + n.to_bytes(4, 'little') + (0).to_bytes(4, 'little') + bytes([1])
    assert len(head) == 570
    open(a.out, 'wb').write(bytes(head) + bytes(36) + bytes(body))
    shutil.copyfile(a.state, a.out + '_SaveState.p2s')
    print(a.out, n, 'fields')


if __name__ == '__main__':
    main()
