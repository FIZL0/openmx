"""S219: a capture's wreck reason changes, with the bike's velocity, its g loads and each truck's distance.

usage: python3 -B research/s219/look.py CAPTURE.csv
"""
import csv
import math
import sys

rows = list(csv.DictReader(open(sys.argv[1])))
cars = [c[:-6] for c in rows[0] if c.endswith('_pos.x')]
prev = None
for i, r in enumerate(rows):
    if r['wreck_reason'] != prev or i == 0 or i == len(rows) - 1:
        b = (float(r['ch_cofg.x']), float(r['ch_cofg.z']))
        near = ' '.join('%s %.1f ft' % (c, math.hypot(float(r[c + '_pos.x']) - b[0], float(r[c + '_pos.z']) - b[1]))
                        for c in cars)
        print('row %3d frame %s reason %s vel %s g %s | %s' % (
            i, r['frame_counter'], r['wreck_reason'],
            ' '.join('%.2f' % float(r['ch_vel.' + k]) for k in 'xyz'),
            ' '.join('%.2f' % float(r['g_peak.' + k]) for k in 'xyz'), near))
    prev = r['wreck_reason']
