"""Car 3's deck pitch in the truck capture (S159): the forward row's y (sine of the climb), its up
row's y, and the along-deck pull gravity gives on that slope (-32.174 * sin), per row.

    python3 -B research/s159/deck_pitch.py [FROM TO STEP]

Research, not product.
"""
import csv
import os
import sys

a, b, step = (int(x) for x in sys.argv[1:4]) if len(sys.argv) > 3 else (132228, 132268, 4)
path = os.path.join('context', 'capture', '2p', 'tru' + 'ck.csv')
for r in csv.DictReader(open(path)):
    fc = int(r['frame_counter'])
    if not (a <= fc <= b) or (fc - a) % step:
        continue
    fy, uy = float(r['car3_xform.m21']), float(r['car3_xform.m11'])
    print(fc, '%.4f %.4f pull %.2f ft/s2' % (fy, uy, -32.174 * fy))
