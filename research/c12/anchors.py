"""C12: score a 2p playback against the human's own reactions on the tape.

    python3 -B research/c12/anchors.py CAPTURE [CAPTURE ...]   # capture/2p/<CAPTURE>.csv, check.toml names

The tape's let-go moments (research/c12/idle.py: nothing held for 45+ fields, after the first second)
are where the user reacted to their own crash. Per capture: which of them follow a wreck onset of the
same rider within the 90 ticks before (hit), and which of the capture's wrecks get no let-go in the 150
ticks after. A playback riding the user's ride hits them all. Research, not product.
"""
import csv
import os
import subprocess
import sys

FIRST = 127882
out = subprocess.run([sys.executable, '-B', os.path.join(os.path.dirname(__file__), 'idle.py'), '45'],
                     capture_output=True, text=True).stdout.splitlines()
lets = {}
for line in out:
    p, rest = line.split(':')
    lets['p' + p[1]] = [int(x.split('-')[0]) for x in rest.split()]
for name in sys.argv[1:]:
    wrecks = {'p1': [], 'p2': []}
    last = {}
    with open(os.path.join('context', 'capture', '2p', name + '.csv')) as f:
        for r in csv.DictReader(f):
            try:
                fc = int(r['frame_counter'])
            except ValueError:
                continue
            for p in ('p1', 'p2'):
                w = r[p + '_wreck_reason']
                if w not in ('0', '') and last.get(p, '0') in ('0', ''):
                    wrecks[p].append(fc)
                last[p] = w
    print('==', name)
    for p in ('p1', 'p2'):
        marks = []
        for t in lets[p]:
            if t < FIRST + 60:
                continue
            hit = [w for w in wrecks[p] if t - 90 <= w <= t]
            marks.append('%d:%s' % (t, ('hit %+d' % (hit[-1] - t)) if hit else 'miss'))
        unreacted = [w for w in wrecks[p] if not any(w <= t <= w + 150 for t in lets[p])]
        print('  %s let-go: %s' % (p, ' '.join(marks)))
        print('  %s wrecks with no let-go after: %d of %d' % (p, len(unreacted), len(wrecks[p])))
