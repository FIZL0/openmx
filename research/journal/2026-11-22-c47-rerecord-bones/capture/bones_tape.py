#!/usr/bin/env python3
"""C47: S74's three `bones_watch.py` pad scripts as PCSX2 input recordings, so they play by the tick
whatever speed the emulator copy runs at.

    bones_tape.py TEMPLATE.p2m2 STATE.p2s ride|down|downchord OUT.p2m2

S201's `make_tape.py` writes the tape (its layout, two fields a tick); only the scripts are S74's here.
S74 waited 2.5 s after the load before it logged and started the pad, so every step sits LEAD ticks
after the load and `play_bones.sh` logs from the load's frame + LEAD. The wall-clock sleeps are ticks
at 30 a second, rounded as S201 rounded them (120 ms = 4 ticks, 200 ms = 6). Research, not product.
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', '2026-11-02-s201-riding-entry', 'capture'))

import make_tape  # noqa: E402

LEAD = 75
STEPS = {
    'ride': [(45, 'down cross'), (195, 'down l1'), (199, 'down r1'), (205, 'up l1'), (205, 'up r1'),
             (325, 'down l1'), (329, 'down r1'), (335, 'up l1'), (335, 'up r1'), (425, 'up cross')],
    'down': [(45, 'down cross'), (45, 'stick 0 1'), (150, 'stick 1 1'), (225, 'stick 0 0'), (225, 'up cross'),
             (261, 'down l1'), (265, 'down r1'), (271, 'up l1'), (271, 'up r1')],
    'downchord': [(45, 'down cross'), (45, 'stick 0 1'), (165, 'up cross'), (165, 'stick 0 0'),
                  (165, 'down l1'), (169, 'down r1'), (175, 'up l1'), (175, 'up r1')],
}
make_tape.SCRIPTS = {k: [(LEAD + t, w) for t, w in v] for k, v in STEPS.items()}
make_tape.LENGTH = {k: LEAD + 18 * 30 for k in STEPS}

if __name__ == '__main__':
    make_tape.main()
