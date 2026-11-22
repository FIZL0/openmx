"""R53: print research/r53/r53-wreck-truck.toml's capture a row a line: P2's wreck words, the wreck body,
the truck (awake, contacts, place, velocity), the wreck-to-truck and plane-to-truck distances.

usage: python3 -B research/r53/show.py context/capture/r53/wreck-truck.csv
"""
import csv
import math
import sys

for r in csv.DictReader(open(sys.argv[1])):
    v = lambda n: tuple(float(r[f'{n}.{c}']) for c in 'xyz')
    wp, wv, tp, tv, pp = v('wreck_pos'), v('wreck_vel'), v('truck_pos'), v('truck_vel'), v('plane_pos')
    print('%4s reason %2s wrecked %s clock %5.2f wreck %8.2f %7.2f %8.2f v %6.2f %6.2f %6.2f | truck %s c %s %8.2f %7.2f %8.2f v %6.2f %6.2f %6.2f | wreck-truck %5.2f plane-truck %6.1f' % (
        (r['frame_counter'], r['p2_reason'], r['p2_wrecked'], float(r['p2_wreck_clock'])) + wp + wv
        + (r['truck_awake'], r['truck_contacts']) + tp + tv + (math.dist(wp, tp), math.dist(pp, tp))))
