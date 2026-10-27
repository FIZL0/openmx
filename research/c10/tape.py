"""The 2p tape's two pads, tick by tick, against what a playback did (C10's divergence check).

    python3 -B research/c10/tape.py [CAPTURE]       # context/recordings/split/2p.p2m2, capture/2p/<CAPTURE>.csv

The movie is a 570-byte header and 36 bytes a field (port 0, port 1; 18 bytes each: buttons in bytes 0-1,
active low, bytes 2-5 the sticks rx ry lx ly, then twelve pressures). A tick is two fields. Prints, per port, the
stretches of a second or more where the human let go of everything (no button, sticks centred), and the
stretches where Start or Select is down; with a capture, each stretch's body modes and wrecks, so a run that
rode the user's ride lines up with them (a human waits out a put-back) and one that parted does not.
Research, not product.
"""
import csv
import os
import sys

FIRST = 127882
tape = open(os.path.join('context', 'recordings', 'split', '2p.p2m2'), 'rb').read()
HEAD = 570
n = (len(tape) - HEAD) // 36


def pad(field, port):
    o = HEAD + 36 * field + 18 * port
    return tape[o:o + 18]


BUTTONS = ['select', 'l3', 'r3', 'start', 'up', 'right', 'down', 'left',
           'l2', 'r2', 'l1', 'r1', 'tri', 'circle', 'cross', 'square']


def held(p):
    w = p[0] | p[1] << 8
    return [BUTTONS[i] for i in range(16) if not w >> i & 1]


def idle(p):
    return not held(p) and all(abs(b - 0x80) < 0x30 for b in p[2:6])


rows = {}
if len(sys.argv) > 1:
    with open(os.path.join('context', 'capture', '2p', sys.argv[1] + '.csv')) as f:
        for r in csv.DictReader(f):
            try:
                rows[int(r['frame_counter'])] = (int(r['p1_mode']), int(r['p2_mode']), int(r['p1_wreck_reason']),
                                                 int(r['p2_wreck_reason']))
            except (ValueError, KeyError):
                pass


def what(a, b, port):
    seen = [rows.get(FIRST + t) for t in range(a, b + 1)]
    seen = [s for s in seen if s]
    if not seen:
        return ''
    modes = sorted(set(s[port] for s in seen))
    wrecks = sorted(set(s[2 + port] for s in seen) - {0})
    return '  modes %s wrecks %s' % (modes, wrecks)


for port in (0, 1):
    print('port %d (P%d): first %s last %s' % (port, port + 1, pad(0, port).hex(), pad(n - 1, port).hex()))
    for label, test in (('idle', idle), ('start/select', lambda p: {'start', 'select'} & set(held(p)))):
        t = 0
        while t < n // 2:
            if test(pad(2 * t, port)):
                s = t
                while t < n // 2 and test(pad(2 * t, port)):
                    t += 1
                if t - s >= (30 if label == 'idle' else 1):
                    print('  %s ticks %d..%d (frames %d..%d, %.1f s)%s' % (
                        label, s, t - 1, FIRST + s, FIRST + t - 1, (t - s) / 30, what(s, t - 1, port)))
            t += 1
