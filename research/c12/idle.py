"""C12: the tape's idle stretches per pad — where the human let go of everything (no button, sticks
centred within 20) for at least MIN fields, after riding. A human lets go after a crash until the
put-back, so these mark the user's own wreck ticks, whatever the playback rides.

    python3 -B research/c12/idle.py [MIN]          # MIN fields, default 45

Prints each stretch as tape fields and as frame_counter (field + 127882). Research, not product.
"""
import os
import sys

FIRST = 127882
HEAD = 570
MIN = int(sys.argv[1]) if len(sys.argv) > 1 else 45
raw = open(os.path.join('context', 'recordings', 'split', '2p.p2m2'), 'rb').read()
n = (len(raw) - HEAD) // 36
for port in (0, 1):
    idle = []
    for i in range(1, n):
        p = raw[HEAD + 36 * i + 18 * port:HEAD + 36 * i + 18 * port + 6]
        idle.append(p[0] == 0xff and p[1] == 0xff and all(abs(x - 127) <= 20 for x in p[2:6]))
    runs, start = [], None
    for i, v in enumerate(idle + [False], 1):
        if v and start is None:
            start = i
        elif not v and start is not None:
            if i - start >= MIN:
                runs.append((start, i - 1))
            start = None
    print('P%d:' % (port + 1), ' '.join('%d-%d(%d)' % (a + FIRST, b + FIRST, b - a + 1) for a, b in runs))
