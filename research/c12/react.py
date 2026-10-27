"""C12: how the tape's pad reacts around each wreck a playback rode — a human lets go (or changes what
they hold) a few tenths of a second after their own crash, so a wreck the user really had shows a
reaction and one they never had does not.

    python3 -B research/c12/react.py CAPTURE         # capture/2p/<CAPTURE>.csv, check.toml names

Per wreck onset: the rider's pad from 30 ticks before to 120 after in 15-tick bins, each bin the share of
fields with anything held (buttons or a stick off centre by more than 20), 0-9. Research, not product.
"""
import csv
import os
import sys

FIRST = 127882
HEAD = 570
raw = open(os.path.join('context', 'recordings', 'split', '2p.p2m2'), 'rb').read()
n = (len(raw) - HEAD) // 36


def busy(port, f):
    if not 0 < f < n:
        return 0
    p = raw[HEAD + 36 * f + 18 * port:HEAD + 36 * f + 18 * port + 6]
    return int(not (p[0] == 0xff and p[1] == 0xff and all(abs(x - 127) <= 20 for x in p[2:6])))


last = {}
with open(os.path.join('context', 'capture', '2p', sys.argv[1] + '.csv')) as f:
    for r in csv.DictReader(f):
        try:
            fc = int(r['frame_counter'])
        except ValueError:
            continue
        for k, p in enumerate(('p1', 'p2')):
            w = r[p + '_wreck_reason']
            if w not in ('0', '') and last.get(p, '0') in ('0', ''):
                bins = []
                for b in range(-30, 120, 15):
                    s = sum(busy(k, fc - FIRST + b + i) for i in range(15))
                    bins.append(str(min(9, s * 10 // 15)))
                print('%s %6d reason %-2s  %s|%s' % (p, fc, w, ''.join(bins[:2]), ''.join(bins[2:])))
            last[p] = w
